#!/usr/bin/env python3
"""Simulated coding swarms under herdr, for testing the village.

Builds real git repositories with worktrees and an origin, then plays out an
autocoder swarm ("shop", 5 workers) and a modernize swarm ("billing", 2
workers) against them:

  * GitHub issues arrive, get claimed (`working`), blocked (`needs-design`,
    `proposal`) and approved, and close when their work lands;
  * workers check out `feature/issue-N` in their worktree, work, sometimes
    wait on a person, and push to origin master with an autocoder tip ref
    (refs/autocoder/tips/master/<sha>-<N>);
  * a sentinel moves master-green (and now and then falls behind), and
    promotions move env/staging and env/prod;
  * Claude Code transcripts and sub-agents are written where Claude Code
    keeps them, so the village shows sub-agents and what everyone is doing.

The state agents-tui can't read from git goes to <dir>/state, where the
`herdr` and `gh` shims in tools/mock/bin serve it.

Linked to the platform (temporal_sim.py): every prod promotion of "shop"
writes the released commit to <dir>/state/release, and every line the
platform appends to <dir>/state/incidents.jsonl becomes an `incident` issue
in "shop" naming the workstream that failed.

    swarm_sim.py --dir DEMO setup     # create repos (idempotent)
    swarm_sim.py --dir DEMO run       # play it out, forever
"""

import argparse
import json
import os
import random
import subprocess
import time
from pathlib import Path

SWARMS = [("shop", 5, "autocoder"), ("billing", 2, "modernize")]
LABELS = [["bug", "P1"], ["bug", "P0"], ["enhancement", "P2"], ["test-failure", "P1"], ["security", "P0"],
          ["documentation", "P3"], ["performance", "P2"], ["refactor", "P3"], ["enhancement", "P1"],
          ["bug", "P2"], ["dependencies", "P3"]]
BLOCKED = [["needs-design", "enhancement", "P2"], ["proposal"], ["needs-clarification", "bug", "P2"]]
TITLES = {
    "bug": ["Checkout total rounds the wrong way", "Search drops the last result", "Crash when the cart is empty",
            "Timezone shifts order dates", "Retry storm on payment timeout"],
    "enhancement": ["Saved carts", "Bulk discount codes", "Order export to CSV", "Dark mode for the dashboard"],
    "test-failure": ["test_refund_flow flakes on CI", "Integration suite red on Postgres 17"],
    "security": ["Session cookie missing SameSite", "Upgrade vulnerable YAML parser"],
    "documentation": ["Document the webhook retries", "Refresh the API quickstart"],
    "performance": ["Product page takes 2s to render", "N+1 queries in order history"],
    "refactor": ["Split the pricing module", "Remove the legacy cart adapter"],
    "dependencies": ["Bump axum to 0.8", "Update the Node toolchain"],
    "proposal": ["Proposal: loyalty points", "Proposal: move to event sourcing"],
}
AUTOCODER_SUBAGENTS = ["Explore", "general-purpose", "Plan"]
MODERNIZE_SUBAGENTS = ["modernize:architect", "modernize:coder", "modernize:tester", "modernize:security", "modernize:documentation"]
TOOLS = ["Read", "Grep", "Edit", "Bash", "Edit", "WebSearch", "Read", "Bash", "Task"]
GIT_ENV = dict(os.environ, GIT_AUTHOR_NAME="Swarm Sim", GIT_AUTHOR_EMAIL="sim@example.invalid",
               GIT_COMMITTER_NAME="Swarm Sim", GIT_COMMITTER_EMAIL="sim@example.invalid", GIT_CONFIG_NOSYSTEM="1")


def git(cwd, *args):
    return subprocess.run(["git", "-c", "commit.gpgsign=false", "-c", "init.defaultBranch=master", *args],
                          cwd=cwd, env=GIT_ENV, check=True, capture_output=True, text=True).stdout.strip()


def claude_dir(home, cwd):
    """Claude Code's project directory for a working directory."""
    return home / ".claude" / "projects" / "".join(c if c.isascii() and c.isalnum() else "-" for c in str(cwd))


def tool_line(tool):
    if tool is None:
        return {"type": "user", "message": {"role": "user", "content": [{"type": "tool_result", "content": "ok"}]}}
    return {"type": "assistant", "message": {"role": "assistant", "content": [{"type": "tool_use", "name": tool, "input": {}}]}}


class Swarm:
    def __init__(self, root, home, name, workers, workflow):
        self.name, self.workflow, self.home = name, workflow, home
        self.origin = root / "origin" / f"{name}.git"
        self.repo = root / "src" / name
        self.state = root / "state"
        self.workers = [{"i": i, "path": root / "src" / f"{name}-wt-{i}", "issue": None, "status": "idle", "until": 0,
                         "block_at": 0, "subagents": [], "next_tool": 0} for i in range(1, workers + 1)]
        self.issues = []
        self.next_issue_no = 400 if name == "shop" else 900
        self.next_arrival = 0
        self.manager_subagents = []
        self.next_manager_change = 0

    # -- setup ------------------------------------------------------------
    def setup(self):
        if (self.repo / ".git").exists():
            return
        self.origin.parent.mkdir(parents=True, exist_ok=True)
        git(self.origin.parent, "init", "-q", "--bare", self.origin.name)
        self.repo.mkdir(parents=True, exist_ok=True)
        git(self.repo, "init", "-q")
        (self.repo / "README.md").write_text(f"# {self.name}\n")
        if self.name == "shop":
            (self.repo / ".autocoder.json").write_text(json.dumps({"environments": [
                {"name": "staging", "ref": "env/staging", "requires": "green", "auto": True},
                {"name": "prod", "ref": "env/prod", "requires": "staging"}]}, indent=2))
        git(self.repo, "add", "-A")
        git(self.repo, "commit", "-q", "-m", "Initial commit")
        git(self.repo, "remote", "add", "origin", str(self.origin))
        git(self.repo, "push", "-q", "origin", "master", "master:master-green", "master:env/staging", "master:env/prod")
        git(self.repo, "fetch", "-q", "origin")
        git(self.repo, "remote", "set-head", "origin", "master")
        for w in self.workers:
            git(self.repo, "worktree", "add", "-q", "-b", f"idle/wt-{w['i']}", str(w["path"]))

    # -- issues -----------------------------------------------------------
    def new_issue(self, labels=None):
        labels = labels or (random.choice(BLOCKED) if random.random() < 0.15 else random.choice(LABELS))
        kind = next((l for l in labels if l in TITLES), "bug")
        self.next_issue_no += 1
        self.issues.append({"number": self.next_issue_no, "title": random.choice(TITLES[kind]), "labels": list(labels),
                            "opened": time.time()})

    def claimable(self):
        blocking = {"needs-design", "needs-clarification", "needs-approval", "too-complex", "proposal", "working"}
        rank = lambda i: next((int(l[1]) for l in i["labels"] if len(l) == 2 and l[0] == "P"), 9)
        return sorted((i for i in self.issues if not blocking & set(i["labels"])), key=lambda i: (rank(i), i["number"]))

    # -- transcripts ------------------------------------------------------
    def append(self, path, entry):
        path.parent.mkdir(parents=True, exist_ok=True)
        with open(path, "a") as f:
            f.write(json.dumps(entry) + "\n")

    def subagent(self, cwd, sid, agent_type, finished, tool=None):
        base = claude_dir(self.home, cwd) / "sim-session" / "subagents"
        base.mkdir(parents=True, exist_ok=True)
        (base / f"agent-{sid}.meta.json").write_text(json.dumps(
            {"agentType": agent_type, "description": f"{agent_type} for {cwd.name}", "finished": finished}))
        if tool is not None or not (base / f"agent-{sid}.jsonl").exists():
            self.append(base / f"agent-{sid}.jsonl", tool_line(tool))

    # -- the swarm at work -------------------------------------------------
    def tick(self, t):
        if t >= self.next_arrival:
            self.new_issue()
            self.next_arrival = t + random.uniform(10, 22)
        for issue in self.issues:
            # Blocked issues get approved after a while.
            if t - issue["opened"] > 45 and {"needs-design", "needs-clarification", "proposal"} & set(issue["labels"]):
                issue["labels"] = [l for l in issue["labels"] if l not in ("needs-design", "needs-clarification", "proposal")]
                if not any(l in TITLES for l in issue["labels"]):
                    issue["labels"] += ["enhancement", "P2"]
        for w in self.workers:
            self.work(w, t)
        if self.workflow == "modernize" and t >= self.next_manager_change:
            for sid, kind in self.manager_subagents:
                self.subagent(self.repo, sid, kind, True)
            self.manager_subagents = [(f"m{int(t)}{k}", kind) for k, kind in enumerate(random.sample(MODERNIZE_SUBAGENTS, 3))]
            self.next_manager_change = t + 30
        for sid, kind in self.manager_subagents:
            if random.random() < 0.3:
                self.subagent(self.repo, sid, kind, False, random.choice(TOOLS))

    def work(self, w, t):
        if w["status"] == "idle":
            todo = self.claimable()
            if todo and random.random() < 0.5:
                issue = todo[0]
                issue["labels"].append("working")
                w.update(issue=issue["number"], status="working", until=t + random.uniform(25, 45), next_tool=0)
                w["block_at"] = t + random.uniform(8, 15) if random.random() < 0.3 else 0
                git(w["path"], "checkout", "-q", "-B", f"feature/issue-{issue['number']}")
                pool = MODERNIZE_SUBAGENTS if self.workflow == "modernize" else AUTOCODER_SUBAGENTS
                w["subagents"] = [(f"w{w['i']}x{issue['number']}{k}", kind) for k, kind in enumerate(random.sample(pool, random.randint(0, 2)))]
        elif w["status"] == "working":
            if w["block_at"] and t >= w["block_at"]:
                w.update(status="blocked", block_at=0, resume=t + random.uniform(8, 14))
            elif t >= w["until"]:
                self.land(w)
                w.update(status="done", until=t + 5)
            elif t >= w["next_tool"]:
                tool = random.choice(TOOLS + [None])
                self.append(claude_dir(self.home, w["path"]) / "sim-session.jsonl", tool_line(tool))
                for sid, kind in w["subagents"]:
                    self.subagent(w["path"], sid, kind, False, random.choice(TOOLS))
                w["next_tool"] = t + random.uniform(3, 6)
        elif w["status"] == "blocked" and t >= w["resume"]:
            w["status"] = "working"
        elif w["status"] == "done" and t >= w["until"]:
            for sid, kind in w["subagents"]:
                self.subagent(w["path"], sid, kind, True)
            w.update(status="idle", issue=None, subagents=[])
            git(w["path"], "checkout", "-q", f"idle/wt-{w['i']}")

    def land(self, w):
        """Merge the worker's issue to master the way merge-to-integration does."""
        n = w["issue"]
        git(self.repo, "commit", "-q", "--allow-empty", "-m", f"Fix #{n}")
        sha = git(self.repo, "rev-parse", "HEAD")
        git(self.repo, "push", "-q", "origin", "master", f"{sha}:refs/autocoder/tips/master/{sha}-{n}")
        git(self.repo, "fetch", "-q", "origin")
        self.issues = [i for i in self.issues if i["number"] != n]

    def sentinel(self, red):
        if not red:
            git(self.repo, "push", "-q", "-f", "origin", "refs/remotes/origin/master:refs/heads/master-green")
            git(self.repo, "fetch", "-q", "origin")

    def promote(self, env, source):
        git(self.repo, "push", "-q", "-f", "origin", f"refs/remotes/origin/{source}:refs/heads/{env}")
        git(self.repo, "fetch", "-q", "origin")
        if env == "env/prod":
            self.write_release()

    def write_release(self):
        sha = git(self.repo, "rev-parse", "refs/remotes/origin/env/prod")
        self.state.mkdir(parents=True, exist_ok=True)
        (self.state / "release").write_text(sha + "\n")

    def file_incidents(self, offset):
        """New lines from the platform's incident file become incident issues."""
        path = self.state / "incidents.jsonl"
        try:
            with open(path) as f:
                f.seek(offset)
                lines = f.read()
                offset = f.tell()
        except OSError:
            return offset
        for line in lines.splitlines():
            try:
                inc = json.loads(line)
            except ValueError:
                continue
            self.next_issue_no += 1
            self.issues.append({"number": self.next_issue_no, "title": f"{inc['type']} {inc['id']} failed: {inc['reason']}",
                                "labels": ["incident", "bug", "P1"], "opened": time.time()})
        return offset

    # -- what the shims serve -----------------------------------------------
    def herdr_agents(self):
        manager_status = "working" if self.manager_subagents or any(w["status"] != "idle" for w in self.workers) else "idle"
        agents = [{"pane_id": f"{self.name}:p0", "name": f"manager-{self.name}", "agent": "claude",
                   "agent_status": manager_status, "cwd": str(self.repo)}]
        for w in self.workers:
            agents.append({"pane_id": f"{self.name}:p{w['i']}", "name": f"wt{w['i']}-{self.name}", "agent": "claude",
                           "agent_status": w["status"], "cwd": str(w["path"])})
        return agents

    def write_issues(self):
        out = [{"number": i["number"], "title": i["title"], "state": "OPEN", "labels": [{"name": l} for l in i["labels"]]}
               for i in self.issues]
        path = self.state / "issues" / f"{self.name}.json"
        path.parent.mkdir(parents=True, exist_ok=True)
        tmp = path.with_suffix(".tmp")
        tmp.write_text(json.dumps(out))
        tmp.replace(path)


def main():
    ap = argparse.ArgumentParser(description="Simulated coding swarms under herdr.")
    ap.add_argument("--dir", required=True, type=Path)
    ap.add_argument("--home", type=Path, help="HOME whose ~/.claude gets transcripts (default <dir>/home)")
    ap.add_argument("--seed", type=int)
    ap.add_argument("command", choices=["setup", "run"])
    args = ap.parse_args()
    if args.seed is not None:
        random.seed(args.seed)
    root = args.dir.resolve()
    home = (args.home or root / "home").resolve()
    swarms = [Swarm(root, home, name, n, wf) for name, n, wf in SWARMS]
    for s in swarms:
        s.setup()
    if args.command == "setup":
        return
    for s in swarms:
        for _ in range(8):
            s.new_issue()
    swarms[0].write_release()
    incident_offset = 0
    print(f"simulating {', '.join(s.name for s in swarms)} in {root}", flush=True)
    next_green, next_staging, next_prod, red_until, next_red = 0, 0, 0, 0, time.time() + 150
    while True:
        t = time.time()
        for s in swarms:
            s.tick(t)
        incident_offset = swarms[0].file_incidents(incident_offset)
        if t >= next_red:
            red_until, next_red = t + 25, t + random.uniform(150, 240)
        if t >= next_green:
            for s in swarms:
                s.sentinel(red=t < red_until)
            next_green = t + 12
        if t >= next_staging:
            swarms[0].promote("env/staging", "master-green")
            next_staging = t + 20
        if t >= next_prod:
            swarms[0].promote("env/prod", "env/staging")
            next_prod = t + 35
        state = root / "state"
        state.mkdir(parents=True, exist_ok=True)
        tmp = state / "herdr.tmp"
        tmp.write_text(json.dumps({"result": {"agents": [a for s in swarms for a in s.herdr_agents()]}}))
        tmp.replace(state / "herdr.json")
        for s in swarms:
            s.write_issues()
        time.sleep(1)


if __name__ == "__main__":
    main()
