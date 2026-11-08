#!/usr/bin/env python3
"""Replay a repository's development in the village from its git and GitHub history.

Writes a recording in the format tools/mock/record.py produces, so
tools/mock/replay-build.py renders it unchanged, without a live swarm or a
prior recording:

    python3 tools/mock/history.py --repo . --since 2026-11-08T14:50Z --until 2026-11-08T17:00Z \\
        --step 60 tools/mock/recordings/history.json.gz
    python3 tools/mock/replay-build.py tools/mock/recordings/history.json.gz replay.html

Sources, all from history:
  * git: where each issue landed on the integration branch. autocoder's tip
    refs (refs/autocoder/tips/<branch>/<sha>-<N>) name the exact commit; without
    them, the `Merge origin/<branch> into feature/issue-N` or `Fix #N` /
    `Implement #N` commit does. The commit's time is when it landed.
  * GitHub (unless --no-github): every issue's timeline: when it opened, each
    label added and removed (`working` is a claim), when it closed and why;
    sub-issues and `Sub-task of #N` bodies for features; GitHub Deployments
    per environment in .autocoder.json.
  * --no-github: commits and merges only. Each landed issue is claimed from
    its branch's first commit to its landing.

Workers aren't in git, so claims are dealt to houses by start time, each to
the lowest-numbered free house (greedy interval colouring): never more houses
than the most claims at once, and never two issues in one house. Stretches
with no events for longer than --idle-gap are skipped.

What the history can't say: the green ref and environment branches only
have their current position, so an issue counts as having reached them once
it has landed and their current tip contains it. Deployments do have a
history and are replayed as they happened.

Tests: python3 -m unittest discover -s tools/mock -p 'test_history.py'
"""

import argparse
import bisect
import gzip
import json
import os
import re
import subprocess
import sys
from datetime import datetime, timezone

# The village's own rules (src/model/issue.rs, src/web/village.rs, framework.rs, feature.rs).
BLOCKING_LABELS = ["needs-design", "needs-clarification", "needs-approval", "needs-feedback", "blocked",
                   "too-complex", "future", "proposal"]
HANDED_OFF_LABELS = ["merging", "awaiting-integration"]
KIND_LABELS = [("security", "security"), ("test-failure", "tests"), ("tests", "tests"), ("test", "tests"),
               ("bug", "bug"), ("performance", "perf"), ("perf", "perf"), ("enhancement", "feature"),
               ("feature", "feature"), ("refactor", "refactor"), ("documentation", "docs"), ("docs", "docs"),
               ("chore", "chores"), ("dependencies", "chores"), ("ci", "chores"), ("proposal", "research"),
               ("research", "research"), ("question", "research")]
DELIVERED_KEEP = 12
NOT_PLANNED = {"NOT_PLANNED", "DUPLICATE"}
INF = float("inf")


def parse_time(s):
    """An ISO-8601 time (`2026-11-08T14:50Z`, `…:00Z`, `…+00:00`) as epoch seconds."""
    dt = datetime.fromisoformat(s.strip().replace("Z", "+00:00"))
    if dt.tzinfo is None:
        dt = dt.replace(tzinfo=timezone.utc)
    return dt.timestamp()


# ---------------------------------------------------------------------------
# The model: issues as they were at any moment
# ---------------------------------------------------------------------------

class Issue:
    def __init__(self, number, title="", url="", body="", created=0.0, closed=None, reason=None, labels=None):
        self.number, self.title, self.url, self.body = number, title, url, body or ""
        self.created, self.closed, self.reason = created, closed, (reason or "").upper() or None
        # (time, "add" | "remove", label), in time order.
        self.label_events = sorted(labels or [], key=lambda e: e[0])
        self.releases = []       # times of `🔓 Releasing claim` comments
        self.parent = None       # feature parent, when this is a subtask
        self.linked_by = None    # "sub-issues" | "body"
        self.linked_at = None    # when it became the parent's subtask

    def is_open(self, t):
        return self.created <= t and (self.closed is None or t < self.closed)

    def labels_at(self, t):
        """Labels in the order they were added, as of `t`."""
        out = []
        for et, op, name in self.label_events:
            if et > t:
                break
            if op == "add" and name not in out:
                out.append(name)
            elif op == "remove" and name in out:
                out.remove(name)
        return out

    def claims(self, rejoin=120):
        """Claim intervals: `working` added until removed or closed (end None:
        still claimed). A gap of `rejoin` seconds or less with no release
        comment in it is one claim: the merge job or a relaunch drops and
        restores the label while the same worker holds the issue. A release
        (a worker giving the issue back, e.g. for a dependency) always ends one."""
        out, start = [], None
        for et, op, name in self.label_events:
            if name != "working":
                continue
            if self.closed is not None and et >= self.closed:
                break
            if op == "add" and start is None:
                gap_from = out[-1][1] if out and out[-1][1] is not None else None
                released = gap_from is not None and any(gap_from - 30 <= r <= et for r in self.releases)
                if gap_from is not None and et - gap_from <= rejoin and not released:
                    start = out.pop()[0]
                else:
                    start = et
            elif op == "remove" and start is not None:
                out.append((start, et))
                start = None
        if start is not None:
            out.append((start, self.closed))
        return out


def assign_houses(intervals):
    """Deal (start, end, key) claims to houses: a key claimed again goes back
    to its last house if that is free, else the lowest-numbered free house.
    Returns ([(start, end, key, house)], houses): a house is only added when
    every other is busy, so `houses` is the most claims at once, and no house
    holds two at a time. An end of None means the claim never ended."""
    ends, out, last = [], [], {}
    for start, end, key in sorted(intervals, key=lambda c: (c[0], INF if c[1] is None else c[1], str(c[2]))):
        stop = INF if end is None else end
        free = [h for h, e in enumerate(ends) if e <= start]
        house = last.get(key) if last.get(key) in free else (free[0] if free else None)
        if house is None:
            house = len(ends)
            ends.append(stop)
        else:
            ends[house] = stop
        last[key] = house
        out.append((start, end, key, house))
    return out, len(ends)


def frame_times(events, since, until, step, idle_gap):
    """Frame times every `step` seconds from `since` to `until`, jumping over
    stretches with no events for longer than `idle_gap` (a frame lands just
    before the next event)."""
    evs = sorted(e for e in events if since <= e <= until)
    times, t = [], since
    while t <= until:
        times.append(t)
        i = bisect.bisect_right(evs, t)
        nxt = evs[i] if i < len(evs) else None
        if nxt is None:
            if until - t > idle_gap:
                break
            t += step
        elif nxt - t > idle_gap:
            t = max(t + step, nxt - step)
        else:
            t += step
    return times


def house_kind(labels):
    low = [l.lower() for l in labels]
    return next((kind for name, kind in KIND_LABELS if name in low), "workshop")


def priority(labels):
    return next((l for l in labels if re.fullmatch(r"P[0-3]", l)), None)


def rank(p):
    return int(p[1]) if p else 9


def blocking(labels):
    return next((l for l in labels if l in BLOCKING_LABELS), None)


def spec_link(body, url):
    """The first link in a parent's body to a document in the same repository
    (see spec_link in src/model/issue.rs), else None."""
    repo = url.split("/issues/")[0] if "/issues/" in url else None
    if not repo:
        return None
    found = []
    for m in re.finditer(re.escape(repo + "/blob/") + r"[^\s)>\"'`\]]+", body):
        link = m.group(0).rstrip(".,;:")
        if link.split("#")[0].split("?")[0].lower().endswith(".md"):
            found.append((m.start(), link))
            break
    m = re.search(r"\]\((?:\./|/)?(docs/[^\s)>\"'`\]]+)", body)
    if m:
        found.append((m.start(), f"{repo}/blob/HEAD/{m.group(1).rstrip('.,;:')}"))
    return min(found)[1] if found else None


class Stop:
    """One pipeline stop over time. `at(t)` is (source, sha, commit): what the
    stop ran at `t`: source "deployment" | "branch" | "unknown", the deployed
    sha (deployments only) and the commit to test ancestry against."""

    def __init__(self, name, deployments=None, branch_head=None):
        self.name = name
        # (success time, sha), oldest first.
        self.deployments = sorted(deployments or [])
        self.branch_head = branch_head

    def at(self, t):
        live = [sha for when, sha in self.deployments if when <= t]
        if live:
            return "deployment", live[-1], live[-1]
        if self.branch_head:
            return "branch", None, self.branch_head
        return "unknown", None, None


class History:
    """Everything a frame is folded from."""

    def __init__(self, project, repo_path, integration, issues, landed, stops, promotion="subtask",
                 contains=lambda commit, sha: False):
        self.project, self.repo_path, self.integration = project, repo_path, integration
        self.issues = {i.number: i for i in issues}
        self.landed = landed              # issue -> (time, sha)
        self.stops = stops                # [Stop], the green ref first
        self.promotion = promotion
        self.contains = contains          # contains(commit, sha): is sha in commit's history?
        self.claims, self.houses = assign_houses([(s, e, i.number) for i in issues for s, e in i.claims()])

    def events(self):
        out = []
        for i in self.issues.values():
            out.append(i.created)
            out.extend(t for t, _, _ in i.label_events)
            if i.closed is not None:
                out.append(i.closed)
        out.extend(t for t, _ in self.landed.values())
        for s in self.stops:
            out.extend(t for t, _ in s.deployments)
        return out


def placement(h, t):
    """(quay, stops, reached) at `t`, as place_tips does in src/web/village.rs."""
    states = [s.at(t) for s in h.stops]
    stops = [{"name": s.name, "issues": [], "source": src} | ({"sha": sha} if sha else {})
             for s, (src, sha, _) in zip(h.stops, states)]
    quay, reached = [], {}
    for n, (when, sha) in sorted(h.landed.items(), key=lambda kv: -kv[1][0]):
        if when > t:
            continue
        r = 0
        for _, _, commit in states:
            if commit is None or not h.contains(commit, sha):
                break
            r += 1
        reached[n] = r
        (quay if r == 0 else stops[r - 1]["issues"]).append(n)
    if stops:
        stops[-1]["issues"] = stops[-1]["issues"][:DELIVERED_KEEP]
    release = None
    if states and states[-1][2]:
        release = states[-1][1] or states[-1][2]
    return quay, stops, reached, release


def house(role, number=None, issue=None, labels=(), status="idle"):
    return {"id": f"history/{role}", "role": role, "kind": house_kind(labels) if issue else "cottage",
            "status": status, "issue": number, "issue_title": issue.title if issue else None,
            "priority": priority(labels) if issue else None, "crew": [role], "subagents": [], "action": None,
            "links": []}


def features_at(h, t, village, reached):
    """Features as src/web/feature.rs builds them, from the issues as they were at `t`."""
    in_house = {x["issue"]: x["status"] for x in village["houses"] if x["issue"]}
    board = {x["number"]: x["label"] for x in village["board"]}
    stops = village["pipeline"]["stops"]
    kids = {}
    for i in h.issues.values():
        if i.parent and i.linked_at is not None and i.linked_at <= t and i.created <= t:
            kids.setdefault(i.parent, []).append(i)
    out = []
    for parent_no, children in sorted(kids.items()):
        p = h.issues.get(parent_no)
        if p is None or p.created > t:
            continue
        # Sub-issues win over body links, as in feature_link_from_json.
        by_sub = [c for c in children if c.linked_by == "sub-issues"]
        children = sorted(by_sub or children, key=lambda c: c.number)
        subtasks, counted = [], []
        for c in children:
            closed = c.closed is not None and c.closed <= t
            dropped = closed and c.reason in NOT_PLANNED
            at = reached.get(c.number)
            hs = None if closed else in_house.get(c.number)
            note = None if closed else board.get(c.number)
            where = ("quay" if at == 0 else "stop" if at is not None else "house" if hs else "board" if note
                     else "closed" if closed else "backlog")
            st = {"number": c.number, "title": c.title, "dropped": dropped, "where": where,
                  "blocked": note or ("waiting" if hs == "blocked" else None)}
            if at:
                st["stop"] = stops[at - 1]["name"]
            subtasks.append(st)
            if not dropped:
                counted.append(c.number)
        total = len(counted)
        stages = [{"name": s["name"], "reached": sum(1 for n in counted if reached.get(n, -1) > k), "total": total,
                   "unknown": s["source"] == "unknown"} for k, s in enumerate(stops)]
        live = total > 0 and bool(stages) and stages[-1]["reached"] == total and not stages[-1]["unknown"]
        # Open parents are features; a closed one stays while it is live.
        if not p.is_open(t) and not live:
            continue
        f = {"number": p.number, "title": p.title, "url": p.url, "linked_by": by_sub and "sub-issues" or "body",
             "promotion": h.promotion, "subtasks": subtasks,
             "build": {"landed": sum(1 for n in counted if n in reached), "total": total},
             "stages": stages, "live": live,
             "stuck": [s["number"] for s in subtasks if not s["dropped"] and s["blocked"]]}
        spec = spec_link(p.body, p.url)
        if spec:
            f["spec_url"] = spec
        out.append(f)
    return out


def house_claims(h, t, window=0):
    """house -> issue at `t`. A claim shorter than a frame still shows: any
    claim in (t - window, t] counts, the one still running at `t` first,
    else the latest to start."""
    best = {}
    for start, end, n, k in h.claims:
        stop = INF if end is None else end
        if start > t or stop <= t - window:
            continue
        key = (stop > t, start)
        if k not in best or key > best[k][0]:
            best[k] = (key, n)
    return {k: n for k, (_, n) in best.items()}


def frame_at(h, t, window=0):
    """The village at `t`, in /api/village's shape. `window` is the time a
    frame stands for, so claims shorter than it still appear."""
    quay, stops, reached, release = placement(h, t)
    houses = [house(f"worker-{k + 1}") for k in range(h.houses)]
    for k, n in house_claims(h, t, window).items():
        i = h.issues.get(n) or Issue(n, f"#{n}")
        houses[k] = house(f"worker-{k + 1}", n, i, i.labels_at(t), "working")
    in_house = {x["issue"] for x in houses if x["issue"]}
    placed = set(reached)
    backlog, board = [], []
    for i in h.issues.values():
        if not i.is_open(t) or i.number in in_house or i.number in placed:
            continue
        labels = i.labels_at(t)
        if any(l in HANDED_OFF_LABELS for l in labels):
            quay.append(i.number)
            continue
        if "working" in labels or "decomposed" in labels:
            continue
        b = blocking(labels)
        if b:
            board.append({"number": i.number, "title": i.title, "label": b, "url": i.url or None})
        else:
            backlog.append({"number": i.number, "title": i.title, "kind": house_kind(labels),
                            "priority": priority(labels), "url": i.url or None})
    backlog.sort(key=lambda c: (rank(c["priority"]), c["number"]))
    board.sort(key=lambda n: n["number"])
    sentinel = "unknown" if not stops or stops[0]["source"] == "unknown" else "green"
    village = {
        "project": h.project, "plane": "development", "framework": "autocoder", "workflow": None,
        "repo_path": h.repo_path,
        "manager": house("manager", status="working" if in_house else "idle") | {"id": "manager"},
        "houses": houses, "backlog": backlog, "board": board,
        "pipeline": {"integration": h.integration, "sentinel": sentinel, "quay": quay, "stops": stops,
                     "release": release, "promotion": h.promotion},
        "builds": [],
    }
    feats = features_at(h, t, village, reached)
    if feats:
        village["features"] = feats
    return {"villages": [village], "worlds": []}


def frames(h, since, until, step, idle_gap):
    # A frame just after every claim starts and ends, so a claim given back
    # within a step (a release for a dependency) shows as a claim and a gap.
    marks = {x + 1 for start, end, _, _ in h.claims for x in (start, end) if x is not None and since <= x + 1 <= until}
    times = sorted(set(frame_times(h.events(), since, until, step, idle_gap)) | marks)
    return [frame_at(h, t, t - prev if prev is not None else 0) for prev, t in zip([None] + times, times)]


# ---------------------------------------------------------------------------
# Reading the history: git and GitHub
# ---------------------------------------------------------------------------

def run(cmd, cwd, check=True):
    p = subprocess.run(cmd, cwd=cwd, capture_output=True, text=True)
    if check and p.returncode != 0:
        raise RuntimeError(f"{' '.join(cmd[:3])}…: {p.stderr.strip()}")
    return p.stdout


def git(repo, *args, check=True):
    return run(["git", *args], repo, check)


def integration_branch(repo):
    head = git(repo, "symbolic-ref", "--short", "refs/remotes/origin/HEAD", check=False).strip()
    if head.startswith("origin/"):
        return head[len("origin/"):]
    for b in ("main", "master"):
        if git(repo, "rev-parse", "--verify", "--quiet", f"refs/remotes/origin/{b}", check=False).strip():
            return b
    return "master"


def ref_for(repo, branch):
    """origin/<branch> when it exists, else the local branch, else None."""
    for r in (f"refs/remotes/origin/{branch}", f"refs/heads/{branch}"):
        if git(repo, "rev-parse", "--verify", "--quiet", r, check=False).strip():
            return r
    return None


ISSUE_SUBJECT = re.compile(r"^(?:Fix|Implement(?: enhancement)?)\s+#(\d+)\b")
MERGE_SUBJECT = re.compile(r"\binto feature/issue-(\d+)\b")


def landings_from_log(lines):
    """issue -> (time, sha, subject) from `%H %ct %at %s` lines of the
    integration branch, newest first: the `Merge … into feature/issue-N`
    commit when there is one (autocoder pushes it), else `Fix #N` / `Implement #N`."""
    out = {}
    for line in lines:
        parts = line.split(" ", 3)
        if len(parts) < 4:
            continue
        sha, ct, _, subject = parts
        m = MERGE_SUBJECT.search(subject) or ISSUE_SUBJECT.match(subject)
        if not m:
            continue
        n = int(m.group(1))
        prev = out.get(n)
        # Prefer the merge; between two of a kind, the latest (the one that landed).
        if prev is None or (MERGE_SUBJECT.search(subject) and not MERGE_SUBJECT.search(prev[2])):
            out[n] = (float(ct), sha, subject)
    return out


def read_landings(repo, integration, use_remote):
    ref = ref_for(repo, integration)
    if not ref:
        return {}, {}
    log = git(repo, "log", "--format=%H %ct %at %s", ref).splitlines()
    by_message = landings_from_log(log)
    landed = {n: (t, sha) for n, (t, sha, _) in by_message.items()}
    titles = {}
    for line in reversed(log):  # oldest first, so the latest subject wins
        subject = line.split(" ", 3)[-1]
        m = ISSUE_SUBJECT.match(subject)
        if m:
            titles[int(m.group(1))] = subject[m.end():].lstrip(": ").strip()
    tip_lines = []
    if use_remote:
        tip_lines = git(repo, "ls-remote", "origin", f"refs/autocoder/tips/{integration}/*", check=False).splitlines()
    tip_lines += [f"{l.split()[0]} {l.split()[1]}" for l in
                  git(repo, "for-each-ref", "--format=%(objectname) %(refname)", f"refs/autocoder/tips/{integration}/",
                      check=False).splitlines() if l.strip()]
    for line in tip_lines:
        try:
            oid, name = line.split()
            n = int(name.rsplit("-", 1)[1])
            ct = git(repo, "show", "-s", "--format=%ct", oid, check=False).strip()
        except (ValueError, IndexError):
            continue
        if ct:
            landed[n] = (float(ct), oid)
    return landed, titles


def branch_start(repo, sha):
    """When work on a landed commit began: the oldest author time among the
    commits it brought (its merge's second-parent side, or itself)."""
    parents = git(repo, "show", "-s", "--format=%P", sha, check=False).split()
    rng = [f"{parents[0]}..{sha}"] if len(parents) > 1 else ["-1", sha]
    times = [float(x) for x in git(repo, "log", "--format=%at", *rng, check=False).split()]
    return min(times) if times else None


def gh_json(repo, *args):
    out = run(["gh", *args], repo, check=False)
    try:
        return json.loads(out) if out.strip() else None
    except ValueError:
        return None


def paginated(repo, path):
    """Every page of a list endpoint (`--slurp` wraps the pages in one array)."""
    pages = gh_json(repo, "api", "--paginate", "--slurp", path)
    if not isinstance(pages, list):
        return []
    return [x for page in pages for x in (page if isinstance(page, list) else [])]


def read_issues(repo, since, until, limit):
    listing = gh_json(repo, "issue", "list", "--state", "all", "--limit", str(limit), "--json",
                      "number,title,url,body,createdAt,closedAt,stateReason") or []
    issues = []
    for d in listing:
        created = parse_time(d["createdAt"])
        closed = parse_time(d["closedAt"]) if d.get("closedAt") else None
        if created > until or (closed is not None and closed < since):
            continue
        timeline = paginated(repo, f"repos/{{owner}}/{{repo}}/issues/{d['number']}/timeline?per_page=100")
        labels, reason, linked, releases = [], d.get("stateReason"), None, []
        for e in timeline:
            kind, when = e.get("event"), e.get("created_at")
            if not when:
                continue
            if kind in ("labeled", "unlabeled"):
                labels.append((parse_time(when), "add" if kind == "labeled" else "remove", e["label"]["name"]))
            elif kind == "closed" and e.get("state_reason"):
                reason = e["state_reason"]
            elif kind == "parent_issue_added" and linked is None:
                linked = parse_time(when)
            elif kind == "commented" and (e.get("body") or "").lstrip().startswith("🔓"):
                releases.append(parse_time(when))
        i = Issue(d["number"], d["title"], d["url"], d.get("body"), created, closed, reason, labels)
        i.linked_at, i.releases = linked, releases
        issues.append(i)
    link_features(repo, issues)
    return issues


SUBTASK_OF = re.compile(r"sub-task of #(\d+)(?!\d)", re.I)


def link_features(repo, issues, sub_issues=None):
    """Mark each issue's feature parent: GitHub sub-issues, else the
    `Sub-task of #N` line in its body. `sub_issues` (parent -> children) is
    fetched when not given."""
    by_no = {i.number: i for i in issues}
    if sub_issues is None:
        sub_issues = {}
        for i in issues:
            kids = gh_json(repo, "api", f"repos/{{owner}}/{{repo}}/issues/{i.number}/sub_issues?per_page=100")
            if isinstance(kids, list) and kids:
                sub_issues[i.number] = [k["number"] for k in kids if isinstance(k, dict) and "number" in k]
    for parent, kids in sub_issues.items():
        for k in kids:
            c = by_no.get(k)
            if c:
                c.parent, c.linked_by = parent, "sub-issues"
                c.linked_at = c.linked_at if c.linked_at is not None else c.created
    for c in issues:
        if c.parent is None:
            m = next((m for m in SUBTASK_OF.finditer(c.body) if int(m.group(1)) != c.number), None)
            if m and int(m.group(1)) in by_no:
                c.parent, c.linked_by, c.linked_at = int(m.group(1)), "body", c.created


def read_stops(repo, integration, use_github):
    """The green ref, then each environment in .autocoder.json."""
    stops = [Stop(f"{integration}-green", branch_head=ref_for(repo, f"{integration}-green"))]
    try:
        config = json.loads(git(repo, "show", f"{ref_for(repo, integration) or 'HEAD'}:.autocoder.json", check=False) or "{}")
    except ValueError:
        config = {}
    for env in config.get("environments", []) or []:
        name = env.get("name")
        if not name:
            continue
        deployments = []
        if use_github:
            for d in paginated(repo, f"repos/{{owner}}/{{repo}}/deployments?environment={name}&per_page=100"):
                statuses = paginated(repo, f"repos/{{owner}}/{{repo}}/deployments/{d['id']}/statuses?per_page=100")
                ok = [parse_time(s["created_at"]) for s in statuses if s.get("state") == "success"]
                if ok:
                    deployments.append((min(ok), d["sha"]))
        stops.append(Stop(name, deployments, ref_for(repo, env.get("ref") or f"env/{name}")))
    promotion = config.get("promotion") if config.get("promotion") in ("subtask", "feature") else "subtask"
    return stops, promotion


def git_contains(repo):
    cache = {}

    def contains(commit, sha):
        key = (commit, sha)
        if key not in cache:
            cache[key] = subprocess.run(["git", "merge-base", "--is-ancestor", sha, commit], cwd=repo,
                                        capture_output=True).returncode == 0
        return cache[key]
    return contains


def project_name(repo):
    url = git(repo, "remote", "get-url", "origin", check=False).strip()
    name = re.sub(r"\.git$", "", url.rstrip("/").rsplit("/", 1)[-1].rsplit(":", 1)[-1]) if url else ""
    return name or os.path.basename(os.path.abspath(repo))


def read_history(repo, since, until, use_github=True, issue_limit=500):
    repo = os.path.abspath(repo)
    integration = integration_branch(repo)
    landed, titles = read_landings(repo, integration, use_github)
    # Only what landed in the window: older history would crowd the quay.
    landed = {n: v for n, v in landed.items() if since <= v[0] <= until}
    if use_github:
        issues = read_issues(repo, since, until, issue_limit)
    else:
        # Commits and merges only: each landed issue, claimed from its first commit.
        issues = []
        for n, (t, sha) in landed.items():
            # A landing of one commit has no span of its own: give it two minutes.
            start = min(branch_start(repo, sha) or t, t - 120)
            i = Issue(n, titles.get(n) or f"#{n}", "", "", start, t, "COMPLETED",
                      [(start, "add", "working"), (t, "remove", "working")])
            issues.append(i)
    stops, promotion = read_stops(repo, integration, use_github)
    return History(project_name(repo), repo, integration, issues, landed, stops, promotion, git_contains(repo))


def main(argv=None):
    ap = argparse.ArgumentParser(description="Replay a repository's development in the village from its history.")
    ap.add_argument("out", help="recording to write (.json.gz), for replay-build.py")
    ap.add_argument("--repo", default=".")
    ap.add_argument("--since", required=True, help="ISO-8601 start, e.g. 2026-11-08T14:50Z")
    ap.add_argument("--until", required=True, help="ISO-8601 end")
    ap.add_argument("--step", type=int, default=60, help="seconds of history per frame (default 60)")
    ap.add_argument("--idle-gap", type=int, default=600, help="skip stretches with no events longer than this (s)")
    ap.add_argument("--run", default="history", help="the recording's run name (default history)")
    ap.add_argument("--no-github", action="store_true", help="git only: commits and merges, no network")
    args = ap.parse_args(argv)
    since, until = parse_time(args.since), parse_time(args.until)
    h = read_history(args.repo, since, until, use_github=not args.no_github)
    fr = frames(h, since, until, args.step, args.idle_gap)
    raw = json.dumps({args.run: fr}, separators=(",", ":")).encode()
    with open(args.out, "wb") as f:
        f.write(gzip.compress(raw, 9))
    print(f"frames {len(fr)} houses {h.houses} issues {len(h.issues)} landed {len(h.landed)} raw {len(raw)}",
          file=sys.stderr)


if __name__ == "__main__":
    main()
