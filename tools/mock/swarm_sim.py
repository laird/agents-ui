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

Every prod release of "shop" is recorded the way tools/release.sh records
one: a successful GitHub Deployment of the commit to prod, described as
"Release v0.N.0" and linking its GitHub Release, so the village's ship
sails with a tag. To cut a release now instead of waiting for the next one:

    touch DEMO/state/release-now

With --lifecycle, "shop" plays every part of an issue's life on a loop, so
the village's animations (#368) can be watched without waiting on chance:
every 90 s three issues arrive together (the supply boat), one is blocked
and then unblocked (to the board and back), one is closed as not planned
(carried off), workers claim, work on and land the rest, and a release
ships what has landed.

With --voyages, "shop" uses this repo's own environments, nuc-1 then release,
both read from GitHub Deployments, and voyages.py scripts them on a 120 s
loop: a deploy to nuc-1 that goes pending, in_progress, success; a
promotion of what nuc-1 runs to release; and a deploy to nuc-1 that fails.
The village's environment row shows each voyage (loading, sailing, docked,
failed). `touch DEMO/state/release-now` promotes nuc-1 to release at once.

With --gate, "shop"'s finished work goes through a simulated merge gate
(#375) instead of landing at once. Each hand-off is a merge job that writes
the files src/web/gate.rs reads (<AUTOCODER_TMP>/autocoder-merge-<repo-id>-<N>
.pid/.log/.exit/.run.sh, and a flock on autocoder-gate-slot-<k>.lock while it
gates): it queues for one of two slots, merges, gates with a test count that
climbs, then passes, lands unproven, fails, conflicts or meets a red base, in
turn. Every third hand-off waits for a partner and goes as a merge train.
Failed work is handed back and re-queued a little later.

Linked to the platform (temporal_sim.py): every prod promotion of "shop"
writes the released commit to <dir>/state/release, and every line the
platform appends to <dir>/state/incidents.jsonl becomes an `incident` issue
in "shop" naming the workstream that failed.

    swarm_sim.py --dir DEMO setup     # create repos (idempotent)
    swarm_sim.py --dir DEMO run       # play it out, forever
    swarm_sim.py --dir DEMO --story run   # the incident story: one bug, start to finish
    swarm_sim.py --dir DEMO --feature run                       # two features, spec to prod
    swarm_sim.py --dir DEMO --feature --promotion feature run   # ... promoted as whole features
    swarm_sim.py --dir DEMO --lifecycle run                     # every issue transition, on a loop
    swarm_sim.py --dir DEMO --voyages run                       # nuc-1 and release deployments, on a loop
    swarm_sim.py --dir DEMO --gate run                          # finished work goes through a merge gate

With --story the demo tells one story cleanly: an incident from the platform
becomes a P0 bug, a worker fixes it (asking once for permission on the way),
it lands and stays open as awaiting-integration (as autocoder parks it) until
a prod release contains the fix; then it closes, and state/fixed names that
release so the platform can stop failing on it. No red builds, a quieter
backlog.

With --feature, "shop" follows two features from spec to prod (#346). Its
environments become dev (a branch), uat (GitHub Deployments only) and prod
(Deployments and a branch). The story:
  * the specs arrive: "Saved carts" (#700, subtasks linked as GitHub
    sub-issues) and "Order export" (#710, subtasks whose bodies say
    `Sub-task of #710`, labelled `decomposed` the way autocoder does);
  * workers claim the subtasks, land them, and they move through dev, uat and
    prod one by one (so a feature reads "UAT 2/4" for a while), or with
    --promotion feature together, held at the quay until the last one lands;
  * one subtask starts as `needs-design` (the feature is stuck), and one is
    closed as not planned (its feature counts 3, not 4);
  * once a subtask reaches uat, uat's deployment goes missing for a while (the
    village can't tell what uat runs);
  * every counted subtask reaches prod (live), and the parent closes later.
The issues, sub-issues and deployments go to <dir>/state/features and
<dir>/state/deployments, where the `gh` shim serves GitHub's GraphQL and
Deployments answers from them.
"""

import argparse
import atexit
import datetime
import hashlib
import json
import os
import random
import signal
import subprocess
import sys
import time
from pathlib import Path

import voyages  # next to this file: scripted Deployments for the environment row

SWARMS = [("shop", 5, "autocoder"), ("billing", 2, "modernize")]
STORY = False
FEATURE = False
LIFECYCLE = False
VOYAGES = False
GATE = False
PROMOTION = "subtask"
REPO_URL = "https://github.com/sim/shop"
# The feature story's two features (see the module docstring). Numbers sit
# far above the random issues so the two never collide.
FEATURES = [
    {"number": 700, "title": "Saved carts", "linked_by": "sub-issues", "arrive": 5,
     "body": "Customers can save a cart and come back to it later.\n\nSpec: [Saved carts](docs/specs/saved-carts.md)",
     "children": [(701, "Cart table"), (702, "Save and restore API"), (703, "Saved carts page"), (704, "Expire old carts")],
     "stuck": 703},
    {"number": 710, "title": "Order export", "linked_by": "body", "arrive": 25,
     "body": "Accounting wants every order as CSV, once a day.",
     "children": [(711, "Export query"), (712, "CSV writer"), (713, "Download button"), (714, "Excel export")],
     "dropped": 714},
]
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
        self.story = None  # FeatureStory, for "shop" with --feature
        self.gate = None  # GateSim, for "shop" with --gate
        self.releases = []  # shop's prod deployments, newest first, without --feature
        self.next_tag = 1

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
        blocking = {"needs-design", "needs-clarification", "needs-approval", "too-complex", "proposal", "working",
                    "awaiting-integration", "decomposed", "merging"}
        rank = lambda i: next((int(l[1]) for l in i["labels"] if len(l) == 2 and l[0] == "P"), 9)
        # `hold`: the feature story keeps its doomed subtask in the backlog until it drops it.
        return sorted((i for i in self.issues if not blocking & set(i["labels"]) and not i.get("hold")), key=lambda i: (rank(i), i["number"]))

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
            self.next_arrival = t + (random.uniform(40, 60) if STORY or FEATURE else random.uniform(10, 22))
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
                if "incident" in issue["labels"]:
                    # The story's fix: half a minute, with one stop to ask for permission.
                    w.update(until=t + 32, block_at=t + 9)
                git(w["path"], "checkout", "-q", "-B", f"feature/issue-{issue['number']}")
                pool = MODERNIZE_SUBAGENTS if self.workflow == "modernize" else AUTOCODER_SUBAGENTS
                # Worker 1 always runs a crowd (4-7) so the village's sub-agent rows and "+N" badge show.
                kinds = [random.choice(pool) for _ in range(random.randint(4, 7))] if w["i"] == 1 else random.sample(pool, random.randint(0, 2))
                w["subagents"] = [(f"w{w['i']}x{issue['number']}{k}", kind) for k, kind in enumerate(kinds)]
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
        if self.gate:
            self.gate.submit(w["issue"], time.time())
            return
        self.land_issue(w["issue"])

    def land_issue(self, n, close=True):
        git(self.repo, "commit", "-q", "--allow-empty", "-m", f"Fix #{n}")
        sha = git(self.repo, "rev-parse", "HEAD")
        git(self.repo, "push", "-q", "origin", "master", f"{sha}:refs/autocoder/tips/master/{sha}-{n}")
        git(self.repo, "fetch", "-q", "origin")
        issue = next((i for i in self.issues if i["number"] == n), None)
        if self.story:
            self.story.landed(n, sha)
        if issue and ("incident" in issue["labels"] or not close):
            # Landed, not yet shipped: parked until a prod release carries it.
            issue["labels"] = [l for l in issue["labels"] if l not in ("working", "merging")] + ["awaiting-integration"]
            issue["fix"] = sha
        else:
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
            if self.name == "shop" and not FEATURE:
                self.record_release(git(self.repo, "rev-parse", "refs/remotes/origin/env/prod"), time.time())

    def release_deployment(self, deploys, sha, t):
        """Record a release of sha as tools/release.sh does: a new successful
        deployment described "Release <tag>", the previous one made inactive."""
        if deploys and deploys[0]["sha"] == sha:
            return False
        tag = f"v0.{self.next_tag}.0"
        self.next_tag += 1
        if deploys:
            deploys[0]["statuses"].insert(0, {"state": "inactive", "created_at": now_iso(t)})
        deploys.insert(0, {"id": 5000 + self.next_tag, "sha": sha, "environment": "prod", "created_at": now_iso(t),
                           "description": f"Release {tag}",
                           "statuses": [{"state": "success", "created_at": now_iso(t), "log_url": f"{REPO_URL}/releases/tag/{tag}"},
                                        {"state": "in_progress", "created_at": now_iso(t - 1)}]})
        return True

    def record_release(self, sha, t):
        if self.release_deployment(self.releases, sha, t):
            path = self.state / "deployments" / f"{self.name}.json"
            path.parent.mkdir(parents=True, exist_ok=True)
            tmp = path.with_suffix(".tmp")
            tmp.write_text(json.dumps({"prod": self.releases[:10]}))
            tmp.replace(path)

    def write_release(self, sha=None):
        sha = sha or git(self.repo, "rev-parse", "refs/remotes/origin/env/prod")
        self.state.mkdir(parents=True, exist_ok=True)
        (self.state / "release").write_text(sha + "\n")
        # Parked incidents whose fix this release carries are shipped: close them.
        for issue in [i for i in self.issues if i.get("fix")]:
            shipped = subprocess.run(["git", "merge-base", "--is-ancestor", issue["fix"], sha], cwd=self.repo, env=GIT_ENV).returncode == 0
            if shipped:
                self.issues.remove(issue)
                (self.state / "fixed").write_text(sha + "\n")

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
                                "labels": ["incident", "bug", "P0" if STORY else "P1"], "opened": time.time()})
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


def now_iso(t):
    return datetime.datetime.fromtimestamp(t, datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def shop_config(feature):
    if not feature:
        return {"environments": [{"name": "staging", "ref": "env/staging", "requires": "green", "auto": True},
                                 {"name": "prod", "ref": "env/prod", "requires": "staging"}]}
    # uat has no branch: only its deployments say what it runs.
    return {"environments": [{"name": "dev", "ref": "env/dev", "requires": "green", "auto": True},
                             {"name": "uat", "requires": "dev"},
                             {"name": "prod", "ref": "env/prod", "requires": "uat"}],
            "promotion": PROMOTION}


class FeatureStory:
    """The feature story for "shop" (see the module docstring)."""

    def __init__(self, swarm, t0):
        self.s, self.t0 = swarm, t0
        self.started = []          # features whose spec has arrived
        self.closed = {}           # subtask -> stateReason, once closed
        self.landed_sha = {}       # subtask -> the commit it landed as
        self.live_at, self.parent_closed = {}, set()
        self.deploys = {"uat": [], "prod": []}  # newest first, each with its statuses
        self.next_id = 1
        self.gap, self.gap_done = None, False
        self.next_dev = self.next_uat = self.next_prod = t0 + 10
        swarm.promote("env/dev", "master-green")

    def counted(self, f):
        return [n for n, _ in f["children"] if n != f.get("dropped")]

    def ancestor(self, sha, of):
        return subprocess.run(["git", "merge-base", "--is-ancestor", sha, of], cwd=self.s.repo, env=GIT_ENV).returncode == 0

    def landed(self, n, sha):
        if any(n == c for f in self.started for c, _ in f["children"]):
            self.landed_sha[n] = sha
            self.closed[n] = "COMPLETED"

    def hold(self):
        """Whole-feature promotion: green waits while a feature is part-landed."""
        if PROMOTION != "feature":
            return False
        for f in self.started:
            done = sum(1 for n in self.counted(f) if n in self.landed_sha)
            if 0 < done < len(self.counted(f)):
                return True
        return False

    def in_gap(self, t):
        return self.gap is not None and self.gap[0] <= t < self.gap[1]

    def current(self, env, t):
        if env == "uat" and self.in_gap(t):
            return None
        return self.deploys[env][0]["sha"] if self.deploys[env] else None

    def deploy(self, env, sha, t):
        old = self.deploys[env]
        if old:
            old[0]["statuses"].insert(0, {"state": "inactive", "created_at": now_iso(t)})
        self.deploys[env].insert(0, {"id": self.next_id, "sha": sha, "environment": env, "created_at": now_iso(t),
                                     "statuses": [{"state": "success", "created_at": now_iso(t)},
                                                  {"state": "in_progress", "created_at": now_iso(t - 1)}]})
        self.next_id += 1

    def tick(self, t):
        s = self.s
        for f in FEATURES:
            if f not in self.started and t >= self.t0 + f["arrive"]:
                self.started.append(f)
                s.issues.append({"number": f["number"], "title": f["title"], "labels": ["enhancement", "decomposed"], "opened": t})
                for n, title in f["children"]:
                    labels = ["enhancement", "P1", "subtask"] + (["needs-design"] if n == f.get("stuck") else [])
                    s.issues.append({"number": n, "title": title, "labels": labels, "opened": t, "hold": n == f.get("dropped")})
            # Dropped a little after the spec arrives: the feature counts one fewer.
            drop = f.get("dropped")
            if f in self.started and drop and drop not in self.closed and t >= self.t0 + f["arrive"] + 40:
                s.issues = [i for i in s.issues if i["number"] != drop]
                self.closed[drop] = "NOT_PLANNED"

        # Promotions: dev follows green, uat deploys dev, prod deploys uat.
        if t >= self.next_dev:
            s.promote("env/dev", "master-green")
            self.next_dev = t + 15
        if t >= self.next_uat:
            if not self.in_gap(t):
                dev = git(s.repo, "rev-parse", "refs/remotes/origin/env/dev")
                if dev != self.current("uat", t):
                    self.deploy("uat", dev, t)
            self.next_uat = t + 22
        if t >= self.next_prod:
            uat = self.current("uat", t)
            if uat and uat != self.current("prod", t):
                s.release_deployment(self.deploys["prod"], uat, t)
                git(s.repo, "push", "-q", "-f", "origin", f"{uat}:refs/heads/env/prod")
                git(s.repo, "fetch", "-q", "origin")
                s.write_release()
            self.next_prod = t + 35

        # Once a subtask reaches uat, uat's deployment goes missing for a while.
        uat = self.current("uat", t)
        if not self.gap_done and uat and any(self.ancestor(sha, uat) for sha in self.landed_sha.values()):
            self.gap, self.gap_done = (t + 25, t + 65), True

        # Live once every counted subtask runs in prod; the parent closes later.
        prod = self.current("prod", t)
        for f in self.started:
            n = f["number"]
            if n not in self.live_at and prod and all(c in self.landed_sha and self.ancestor(self.landed_sha[c], prod) for c in self.counted(f)):
                self.live_at[n] = t
            if n in self.live_at and n not in self.parent_closed and t >= self.live_at[n] + 45:
                self.parent_closed.add(n)
                s.issues = [i for i in s.issues if i["number"] != n]
        self.write_state(t)

    def node(self, n, title, body):
        open_ = any(i["number"] == n for i in self.s.issues)
        return {"number": n, "title": title, "url": f"{REPO_URL}/issues/{n}", "body": body,
                "state": "OPEN" if open_ else "CLOSED", "stateReason": None if open_ else self.closed.get(n, "COMPLETED")}

    def write_state(self, t):
        parents = []
        for f in self.started:
            body = "" if f["linked_by"] == "sub-issues" else f"Part of the order export.\n\nSub-task of #{f['number']}\n"
            kids = [self.node(n, title, body) for n, title in f["children"]]
            parents.append({"number": f["number"], "title": f["title"], "url": f"{REPO_URL}/issues/{f['number']}", "body": f["body"],
                            "state": "CLOSED" if f["number"] in self.parent_closed else "OPEN",
                            "labels": ["enhancement", "decomposed"],
                            "subIssues": kids if f["linked_by"] == "sub-issues" else [],
                            "search": kids if f["linked_by"] == "body" else []})
        deploys = {env: ([] if env == "uat" and self.in_gap(t) else d) for env, d in self.deploys.items()}
        for name, data in (("features", {"parents": parents}), ("deployments", deploys)):
            path = self.s.state / name / f"{self.s.name}.json"
            path.parent.mkdir(parents=True, exist_ok=True)
            tmp = path.with_suffix(".tmp")
            tmp.write_text(json.dumps(data))
            tmp.replace(path)


class Lifecycle:
    """--lifecycle: every transition of an issue's life, on a 90 s loop."""
    PERIOD = 90
    SCRIPT = [(0, "arrive"), (12, "block"), (30, "unblock"), (40, "close"), (75, "release")]

    def __init__(self, swarm, t0):
        self.s, self.t0, self.cycle, self.done, self.blocked = swarm, t0, -1, set(), None
        swarm.next_arrival = float("inf")  # arrivals come only from the script

    def tick(self, t):
        cycle, at = divmod(t - self.t0, self.PERIOD)
        if cycle != self.cycle:
            self.cycle, self.done = cycle, set()
        for when, what in self.SCRIPT:
            if at >= when and what not in self.done:
                self.done.add(what)
                getattr(self, what)()

    def waiting(self):
        return [i for i in self.s.claimable() if not i.get("hold")]

    def arrive(self):
        for _ in range(3):
            self.s.new_issue(random.choice(LABELS))

    def block(self):
        todo = self.waiting()
        if todo:
            self.blocked = todo[-1]
            self.blocked["labels"].append("needs-design")

    def unblock(self):
        if self.blocked:
            self.blocked["labels"] = [l for l in self.blocked["labels"] if l != "needs-design"]
            self.blocked = None

    def close(self):
        todo = self.waiting()
        if todo:
            self.s.issues.remove(todo[-1])  # closed as not planned

    def release(self):
        (self.s.state / "release-now").touch()


class GateSim:
    """--gate: a merge gate for "shop", written as autocoder's merge jobs write it."""
    SLOTS = 2
    OUTCOMES = ["pass", "pass", "fail", "unproven", "conflict", "pass", "red"]
    TESTS = 745

    def __init__(self, swarm, tmp):
        self.s, self.tmp = swarm, tmp
        tmp.mkdir(parents=True, exist_ok=True)
        common = os.path.realpath(swarm.repo / ".git")
        self.prefix = f"autocoder-merge-{swarm.repo.name}-{hashlib.sha1(common.encode()).hexdigest()[:8]}-"
        for f in tmp.glob(self.prefix + "*"):
            f.unlink()
        self.jobs, self.pending, self.handed, self.parked = [], [], [], []
        self.count = 0
        atexit.register(self.stop)
        signal.signal(signal.SIGTERM, lambda *_: sys.exit(0))

    def stop(self):
        for j in self.jobs:
            self.kill(j)

    def kill(self, j):
        # The whole group: flock's child inherits the lock, so killing flock alone keeps the slot busy.
        if j.get("proc"):
            try:
                os.killpg(j["proc"].pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            j["proc"].wait()
            j["proc"] = None

    def issue(self, n):
        return next((i for i in self.s.issues if i["number"] == n), None)

    def label(self, n, drop=(), add=()):
        i = self.issue(n)
        if i:
            i["labels"] = [l for l in i["labels"] if l not in drop] + [l for l in add if l not in i["labels"]]

    def path(self, j, ext):
        return self.tmp / f"{self.prefix}{j['issue']}.{ext}"

    def log(self, j, text):
        with open(self.path(j, "log"), "a") as f:
            f.write(text)

    def spawn(self, j, cmd):
        self.kill(j)
        j["proc"] = subprocess.Popen(cmd, stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, start_new_session=True)
        self.path(j, "pid").write_text(f"{j['proc'].pid}\n")

    def submit(self, n, t):
        """A worker hands its finished branch to the gate (merge-launch.sh)."""
        self.label(n, drop=("working",), add=("merging",))
        self.pending.append(n)

    def start(self, members, t):
        self.count += 1
        lead = members[0]
        feature = f"train/{datetime.datetime.fromtimestamp(t, datetime.timezone.utc).strftime('%Y%m%dT%H%M%S')}Z" if len(members) > 1 else f"feature/issue-{lead}"
        if len(members) > 1:
            # A train branch: one merge commit per member, as merge-to-integration's train builder writes them.
            tip = git(self.s.repo, "rev-parse", "origin/master")
            tree = git(self.s.repo, "rev-parse", "origin/master^{tree}")
            for m in members:
                side = git(self.s.repo, "commit-tree", tree, "-p", tip, "-m", f"Fix #{m}")
                tip = git(self.s.repo, "commit-tree", tree, "-p", tip, "-p", side, "-m", f"Merge branch 'feature/issue-{m}' into {feature}")
            git(self.s.repo, "update-ref", f"refs/heads/{feature}", tip)
        j = {"issue": lead, "members": members, "phase": "queued", "since": t, "outcome": self.OUTCOMES[(self.count - 1) % len(self.OUTCOMES)], "slot": None, "ran": 0}
        if len(members) > 1 and j["outcome"] in ("conflict", "red"):
            j["outcome"] = "pass"
        for ext in ("log", "exit"):
            self.path(j, ext).unlink(missing_ok=True)
        self.path(j, "log").write_text("")
        self.path(j, "run.sh").write_text(f"merge-to-integration.sh --feature {feature} --issue {lead} --integration master\n")
        self.spawn(j, ["sleep", "900"])
        self.jobs.append(j)

    def finish(self, j, code, text):
        self.log(j, text)
        self.path(j, "exit").write_text(f"{code}\n")
        self.kill(j)
        j["phase"] = "done"

    def tick(self, t):
        # Every third hand-off waits for a partner and rides a train with it.
        while self.pending:
            if self.count % 3 == 2 and len(self.pending) < 2:
                break
            take = 2 if self.count % 3 == 2 else 1
            self.start(self.pending[:take], t)
            self.pending = self.pending[take:]
        gating = [j for j in self.jobs if j["phase"] == "gating"]
        for j in list(self.jobs):
            age = t - j["since"]
            if j["phase"] == "queued" and age >= 4:
                j["phase"], j["since"] = "merging", t
                self.log(j, f"🔀 Merging {j['members']} onto master in a throwaway worktree\n")
            elif j["phase"] == "merging" and age >= 3:
                if j["outcome"] == "conflict":
                    self.finish(j, 1, f"Auto-merging src/web/village/lifecycle.js\nCONFLICT (content): Merge conflict in src/web/village/lifecycle.js\n❌ Merge conflict integrating 'master' into feature/issue-{j['issue']}.\n")
                    self.hand_back(j, t)
                    continue
                free = [k for k in range(1, self.SLOTS + 1) if k not in {g["slot"] for g in gating}]
                if not free:
                    if not j.get("waiting"):
                        self.log(j, f"⏳ All {self.SLOTS} gate slot(s) on this host are busy; waiting for slot {self.SLOTS} ({time.strftime('%H:%M:%S')})\n")
                        j["waiting"] = True
                    continue
                j["slot"], j["phase"], j["since"] = free[0], "gating", t
                gating.append(j)
                lock = self.tmp / f"autocoder-gate-slot-{free[0]}.lock"
                lock.touch()
                self.spawn(j, ["flock", str(lock), "sleep", "900"])
                self.log(j, f"▶️  Gate slot {free[0]} acquired ({time.strftime('%H:%M:%S')})\n🧪 Gating the integrated tree {random.getrandbits(32):08x}: cargo test\nrunning {self.TESTS} tests\n")
            elif j["phase"] == "gating":
                if age < 14:
                    n = min(self.TESTS - 5, int(self.TESTS * age / 14)) - j["ran"]
                    if n > 0:
                        self.log(j, "".join(f"test t{j['ran'] + k} ... ok\n" for k in range(n)))
                        j["ran"] += n
                    continue
                self.outcome(j, t)
        # Handed back work is re-queued after a while; unproven work is proven green.
        for n, when in list(self.handed):
            if t >= when:
                self.handed.remove((n, when))
                self.label(n, drop=("needs-clarification",))
        for n, when in list(self.parked):
            if t >= when:
                self.parked.remove((n, when))
                self.s.issues = [i for i in self.s.issues if i["number"] != n]
        self.jobs = [j for j in self.jobs if j["phase"] != "done" or t - j["since"] < 2]

    def outcome(self, j, t):
        o, members = j["outcome"], j["members"]
        if o in ("pass", "unproven"):
            done = f"test result: ok. {self.TESTS} passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 14.00s\n"
            if o == "pass":
                self.finish(j, 0, done + f"✅ Issue #{j['issue']} merged into 'master' and verified: tests passed on the published tree.\n✅ Issue #{j['issue']} closed.\n")
            else:
                self.finish(j, 0, done + f"✅ Issue #{j['issue']} merged into 'master' (disjoint re-sync; the published tree was not the gated tree)\n")
            for m in members:
                self.s.land_issue(m, close=(o == "pass"))
                if o == "unproven":
                    self.parked.append((m, t + 20))
        elif o == "fail":
            self.finish(j, 2, f"test result: FAILED. {self.TESTS - 2} passed; 2 failed; 0 ignored\n❌ Tests fail on the integrated tree.\n❌ origin/master passes alone, so this change broke it. NOT pushing.\n")
            self.hand_back(j, t)
        else:  # red base
            self.finish(j, 2, "❌ Tests fail on the integrated tree.\n❌ origin/master fails alone too (tree 9c9c9c9c), and no green ref yet. NOT pushing.\n")
            self.hand_back(j, t)

    def hand_back(self, j, t):
        for m in j["members"]:
            self.label(m, drop=("merging",), add=("needs-clarification",))
            self.handed.append((m, t + 25))


def main():
    ap = argparse.ArgumentParser(description="Simulated coding swarms under herdr.")
    ap.add_argument("--dir", required=True, type=Path)
    ap.add_argument("--home", type=Path, help="HOME whose ~/.claude gets transcripts (default <dir>/home)")
    ap.add_argument("--seed", type=int)
    ap.add_argument("--story", action="store_true", help="tell the incident story cleanly (see the module docstring)")
    ap.add_argument("--feature", action="store_true", help="follow two features from spec to prod (see the module docstring)")
    ap.add_argument("--promotion", choices=["subtask", "feature"], default="subtask",
                    help="with --feature: promote each subtask on its own, or each feature as a whole")
    ap.add_argument("--lifecycle", action="store_true", help="play every transition of an issue's life on a loop (see the module docstring)")
    ap.add_argument("--voyages", action="store_true", help="script nuc-1 and release deployments (see the module docstring)")
    ap.add_argument("--gate", action="store_true", help="send finished work through a simulated merge gate (see the module docstring)")
    ap.add_argument("command", choices=["setup", "run"])
    args = ap.parse_args()
    global STORY, FEATURE, PROMOTION, LIFECYCLE
    global VOYAGES, GATE
    STORY, FEATURE, PROMOTION, LIFECYCLE, VOYAGES = args.story, args.feature, args.promotion, args.lifecycle, args.voyages
    GATE = args.gate
    quiet = STORY or FEATURE or LIFECYCLE
    if args.seed is not None:
        random.seed(args.seed)
    root = args.dir.resolve()
    home = (args.home or root / "home").resolve()
    swarms = [Swarm(root, home, name, n, wf) for name, n, wf in SWARMS]
    for s in swarms:
        s.setup()
    if args.command == "setup":
        return
    # The environments follow the mode, whatever an earlier run left behind.
    shop = swarms[0]
    (shop.repo / ".autocoder.json").write_text(json.dumps(voyages.config() if VOYAGES else shop_config(FEATURE), indent=2))
    for s in swarms:
        for _ in range(3 if quiet else 8):
            s.new_issue()
        if quiet:
            s.next_arrival = time.time() + 60
    shop.write_release()
    if FEATURE:
        shop.story = FeatureStory(shop, time.time())
    lifecycle = Lifecycle(shop, time.time() + 5) if LIFECYCLE else None
    # --voyages: nuc-1 ships the integration tip; a release is what the platform runs.
    script = voyages.VoyageScript(lambda: git(shop.repo, "rev-parse", "refs/remotes/origin/master"), time.time() + 5,
                                  released=shop.write_release) if VOYAGES else None
    if GATE:
        shop.gate = GateSim(shop, Path(os.environ.get("AUTOCODER_TMP") or root / "tmp"))
    incident_offset = 0
    print(f"simulating {', '.join(s.name for s in swarms)} in {root}", flush=True)
    next_green, next_staging, next_prod, red_until, next_red = 0, 0, 0, 0, time.time() + 150
    # Room for the ship to load up between releases; the story keeps its pace.
    prod_every = 35 if STORY else 75
    release_now = root / "state" / "release-now"
    while True:
        t = time.time()
        if release_now.exists():
            # Cut a release now: staging catches up with green, and prod with staging.
            release_now.unlink()
            next_staging = next_prod = 0
            if script:
                script.promote_now(t)
            if shop.story:
                shop.story.next_dev = shop.story.next_uat = shop.story.next_prod = 0
        for s in swarms:
            s.tick(t)
        incident_offset = swarms[0].file_incidents(incident_offset)
        if t >= next_red and not quiet:
            red_until, next_red = t + 25, t + random.uniform(150, 240)
        if t >= next_green:
            for s in swarms:
                s.sentinel(red=t < red_until or (s.story is not None and s.story.hold()))
            next_green = t + 12
        if lifecycle:
            lifecycle.tick(t)
        if shop.gate:
            shop.gate.tick(t)
        if script:
            # Deployments, not branches, say what nuc-1 and release run.
            script.tick(t)
            path = shop.state / "deployments" / f"{shop.name}.json"
            path.parent.mkdir(parents=True, exist_ok=True)
            tmp = path.with_suffix(".tmp")
            tmp.write_text(json.dumps(script.state()))
            tmp.replace(path)
        elif shop.story:
            shop.story.tick(t)
        else:
            if t >= next_staging:
                shop.promote("env/staging", "master-green")
                next_staging = t + 20
            if t >= next_prod:
                shop.promote("env/prod", "env/staging")
                next_prod = t + prod_every
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
