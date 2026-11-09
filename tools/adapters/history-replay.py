#!/usr/bin/env python3
"""An agents-village adapter that plays a repository's history back live.

    python3 tools/mock/history.py --repo ~/src/agents-ui --since 2026-03-20T00:00Z --until 2026-11-08T17:00Z \\
        --step 1800 --idle-gap 3600 --run agents-ui hist-agents-ui.json.gz
    agents-tui --headless --read-only --source swarms=none \\
        --framework "exec:python3 tools/adapters/history-replay.py hist-agents-ui.json.gz --every 6"

Reads recordings made by tools/mock/history.py and, each time agents-tui reruns
it, prints the village as it was at the replay clock's position: one recorded
frame every `--every` seconds of wall time, looping at the end. Unlike
replay-build.py's static page, this is the real, interactive village: click a
house, crate or note to open its issue, follow features down the canal.

One report per recording (a list when there are several), each named
`replay:<project>` (`--prefix`) so it never clashes with a live village of the
same repository.

State lives in `--state` (default $XDG_STATE_HOME/agents-tui/history-replay):
  * the clock, started the first time a recording is replayed; run the
    adapter once by hand with `--restart` to start it over (not in the
    --framework command, which agents-tui reruns on every poll);
  * a frame-per-line copy of each recording with byte offsets, rebuilt when the
    recording changes, so a poll reads one frame instead of decompressing the
    whole recording.

A recording may also be a frame-per-line `.jsonl` file (each line a frame,
`{"villages": [...]}`, or one village).

Subtasks closed as not planned are left out of the report: the report has no
"not planned", and the village would otherwise count them toward the feature.

Tests: python3 -m unittest discover -s tools/adapters -p 'test_history_replay.py'
"""
import argparse
import gzip
import hashlib
import json
import os
import subprocess
import sys
import time

INDEX_VERSION = 1


# ---------------------------------------------------------------------------
# Recordings

def repo_url(path):
    """https://github.com/owner/repo for a clone, from its origin remote."""
    try:
        u = subprocess.run(["git", "-C", path, "remote", "get-url", "origin"], capture_output=True, text=True,
                           timeout=5).stdout.strip()
    except Exception:
        return None
    if u.startswith("git@github.com:"):
        u = "https://github.com/" + u.split(":", 1)[1]
    elif u.startswith("ssh://git@github.com/"):
        u = "https://github.com/" + u[len("ssh://git@github.com/"):]
    u = u[:-4] if u.endswith(".git") else u
    return u if u.startswith("http") else None


def village_of(frame):
    """The first village of a recorded frame, or the frame itself if it is one."""
    if isinstance(frame, dict) and "villages" in frame:
        return frame["villages"][0] if frame["villages"] else None
    return frame if isinstance(frame, dict) and frame.get("project") else None


def read_villages(path):
    """Every village in a recording, in order: a history.py/record.py recording
    (gzipped or not, `{run: [frame, ...]}`), or a frame-per-line file."""
    with open(path, "rb") as f:
        raw = f.read()
    if raw[:2] == b"\x1f\x8b":
        raw = gzip.decompress(raw)
    text = raw.decode()
    try:
        doc = json.loads(text)
    except json.JSONDecodeError:
        frames = [json.loads(line) for line in text.splitlines() if line.strip()]
    else:
        if isinstance(doc, dict) and "villages" not in doc and "project" not in doc:
            frames = next(iter(doc.values()), [])  # {run: frames}: the first run
        elif isinstance(doc, list):
            frames = doc
        else:
            frames = [doc]
    return [v for v in map(village_of, frames) if v]


def state_dir(given=None):
    if given:
        return given
    base = os.environ.get("XDG_STATE_HOME") or os.path.join(os.path.expanduser("~"), ".local", "state")
    return os.path.join(base, "agents-tui", "history-replay")


def state_key(path):
    """A file-name-safe key per recording: its name plus a hash of where it is."""
    full = os.path.abspath(path)
    name = os.path.basename(full).split(".")[0] or "recording"
    return f"{name}-{hashlib.sha1(full.encode()).hexdigest()[:10]}"


def write_atomic(path, data):
    tmp = f"{path}.{os.getpid()}.tmp"
    with open(tmp, "wb") as f:
        f.write(data)
    os.replace(tmp, path)


class Recording:
    """A recording's villages, through a frame-per-line index in the state dir."""

    def __init__(self, path, state):
        self.path = path
        key = os.path.join(state, state_key(path))
        self.meta_path, self.frames_path = key + ".index.json", key + ".frames"
        st = os.stat(path)
        stamp = {"version": INDEX_VERSION, "source": os.path.abspath(path), "size": st.st_size,
                 "mtime": st.st_mtime}
        meta = self._read_meta()
        if not meta or any(meta.get(k) != v for k, v in stamp.items()) or not os.path.exists(self.frames_path):
            meta = self._build(stamp)
        self.offsets = meta["offsets"]
        self.url = meta.get("url")

    def _read_meta(self):
        try:
            with open(self.meta_path) as f:
                return json.load(f)
        except (OSError, ValueError):
            return None

    def _build(self, stamp):
        villages = read_villages(self.path)
        lines = [json.dumps(v, separators=(",", ":")).encode() + b"\n" for v in villages]
        offsets, at = [], 0
        for line in lines:
            offsets.append(at)
            at += len(line)
        first = villages[0].get("repo_path") if villages else None
        meta = stamp | {"offsets": offsets, "url": repo_url(first) if first else None}
        write_atomic(self.frames_path, b"".join(lines))
        write_atomic(self.meta_path, json.dumps(meta).encode())
        return meta

    def __len__(self):
        return len(self.offsets)

    def village(self, i):
        with open(self.frames_path, "rb") as f:
            f.seek(self.offsets[i])
            return json.loads(f.readline())


# ---------------------------------------------------------------------------
# The replay clock

def clock_start(path, state, restart=False, now=None):
    """When the replay of `path` started: now, the first time or on --restart."""
    clock = os.path.join(state, state_key(path) + ".clock")
    now = time.time() if now is None else now
    if not restart:
        try:
            with open(clock) as f:
                return float(f.read().strip())
        except (OSError, ValueError):
            pass
    write_atomic(clock, str(now).encode())
    return now


def position(start, now, every, total):
    """The frame to show: one per `every` seconds since `start`, looping."""
    if total <= 0:
        return 0
    return int(max(0.0, now - start) / every) % total


# ---------------------------------------------------------------------------
# Frame -> report

def report(v, prefix, position, total, base):
    """An agents-village/v1 report for one village of a history.py recording."""
    def issue_url(n):
        return f"{base}/issues/{n}" if base else None

    parent, dropped, feature_tasks = {}, set(), {}
    for f in v.get("features", []):
        fid = str(f["number"])
        feature_tasks[fid] = {"id": fid, "title": f["title"], "url": f.get("url") or issue_url(fid),
                              "state": "claimed", "labels": ["decomposed"]}
        for s in f["subtasks"]:
            if s.get("dropped"):
                dropped.add(str(s["number"]))
            else:
                parent[str(s["number"])] = (fid, s)

    tasks = {}

    def add(tid, **kw):
        t = tasks.setdefault(tid, {"id": tid})
        t.update({k: val for k, val in kw.items() if val is not None})
        if not t.get("url") and issue_url(tid):
            t["url"] = issue_url(tid)
        if tid in parent:
            t["parent"] = parent[tid][0]
            t.setdefault("title", parent[tid][1]["title"])

    for c in v.get("backlog", []):
        add(str(c["number"]), title=c["title"], priority=c.get("priority"), url=c.get("url"),
            labels=[c["kind"]] if c.get("kind") not in (None, "cottage") else None, state="open")
    for n in v.get("board", []):
        add(str(n["number"]), title=n["title"], url=n.get("url"), labels=[n["label"]], state="blocked")

    workstreams = []
    for h in v.get("houses", []):
        agent = {"name": h["role"], "state": h["status"]}
        ws = {"id": h["id"], "name": h["role"], "kind": h["kind"], "agents": [agent]}
        if h.get("issue"):
            tid = str(h["issue"])
            ws["task"] = tid
            agent["detail"] = f"working on #{tid}: {h.get('issue_title') or ''}".strip().rstrip(":")
            agent["action"] = "writing"
            # No link of our own: the village adds "GitHub issue #N" from the task's url.
            if h.get("issue_title"):
                ws["title"] = h["issue_title"]
            add(tid, title=h.get("issue_title"), priority=h.get("priority"), state="claimed")
        workstreams.append(ws)

    p = v.get("pipeline", {})
    quay, stops = p.get("quay", []), p.get("stops", [])
    for n in quay + [n for s in stops for n in s["issues"]]:
        add(str(n), state="done")
    # Subtasks closed, or not yet anywhere, still belong to their feature.
    for tid, (_, s) in parent.items():
        if tid not in tasks:
            add(tid, title=s["title"], state="done" if s["where"] in ("closed", "quay", "stop") else "open")
    for tid, t in feature_tasks.items():
        if tid not in tasks:
            tasks[tid] = t
    for tid in dropped:
        tasks.pop(tid, None)

    m = v.get("manager") or {"status": "idle"}
    return {
        "schema": "agents-village/v1",
        "project": f"{prefix}{v['project']}",
        "plane": "development",
        "framework": f"history replay {position + 1}/{total}",
        "location": v.get("repo_path"),
        "coordinator": {"id": "manager", "name": "manager", "agents": [{"name": "manager", "state": m["status"]}]},
        "workstreams": workstreams,
        "tasks": list(tasks.values()),
        "pipeline": {
            "integration": p.get("integration"),
            "gate": "green" if p.get("sentinel") == "green" else "unknown",
            "landed": [str(n) for n in quay],
            "stages": [{"name": s["name"], "tasks": [str(n) for n in s["issues"]], "source": s.get("source", "branch")}
                       | ({"sha": s["sha"]} if s.get("sha") else {}) for s in stops],
            "release": p.get("release"),
            "promotion": p.get("promotion", "subtask"),
        },
        "builds": [],
        "links": [{"label": "Repository", "url": base}] if base else [],
    }


def empty_report(path, prefix):
    name = os.path.basename(path).split(".")[0] or "recording"
    return {"schema": "agents-village/v1", "project": f"{prefix}{name}", "plane": "development",
            "framework": "history replay 0/0"}


def unique_names(reports):
    """Two recordings of one repository still make two villages."""
    seen = {}
    for r in reports:
        n = seen[r["project"]] = seen.get(r["project"], 0) + 1
        if n > 1:
            r["project"] = f"{r['project']} ({n})"
    return reports


def main(argv=None, now=None):
    ap = argparse.ArgumentParser(description="Play history.py recordings back as live villages.")
    ap.add_argument("recordings", nargs="+", help="history.py recordings (.json.gz), or frame-per-line .jsonl")
    ap.add_argument("--every", type=float, default=6.0,
                    help="wall seconds per recorded frame (default 6; agents-tui polls every 5 s at most)")
    ap.add_argument("--prefix", default="replay:", help="prefix for village names (default replay:)")
    ap.add_argument("--state", help="where the clock and frame index live "
                                    "(default $XDG_STATE_HOME/agents-tui/history-replay)")
    ap.add_argument("--restart", action="store_true", help="start the replay clock over")
    a = ap.parse_args(argv)
    if a.every <= 0:
        ap.error("--every must be positive")
    state = state_dir(a.state)
    os.makedirs(state, exist_ok=True)
    now = time.time() if now is None else now
    out = []
    for path in a.recordings:
        rec = Recording(path, state)
        start = clock_start(path, state, a.restart, now)
        if not len(rec):
            out.append(empty_report(path, a.prefix))
            continue
        i = position(start, now, a.every, len(rec))
        out.append(report(rec.village(i), a.prefix, i, len(rec), rec.url))
    unique_names(out)
    json.dump(out if len(out) > 1 else out[0], sys.stdout)
    sys.stdout.write("\n")


if __name__ == "__main__":
    main()
