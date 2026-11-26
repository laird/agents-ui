#!/usr/bin/env python3
"""Issues an application raises about itself, going round a world (#560).

Prints two agents-village reports (docs/village-protocol.md): `relay`, a
development village, and `agentex:relay`, the runtime it builds. Every 80 s
one of the runtime's workstreams raises an issue in its channel; the issue is
built, lands, is deployed to prod, and the runtime verifies the fix, 20 s each.
The next one comes from the other channel. Draw it as a world:

    agents-tui --headless --framework exec:tools/mock/channels.py \\
      --world relay=agentex:relay --incident-label channel:

`tools/mock/demo.sh` runs it with CHANNELS=1. It keeps no state: where the
story is comes from the clock (`--at` pins it, for a test).
"""
import argparse
import json
import time
from datetime import datetime, timezone

CYCLE, STEP = 80, 20
CHANNELS = [("log-analyzer", "Summaries cut off mid-sentence"), ("user-feedback", "Replies arrive twice")]
RELEASE = "4c5d6e7f8091"


def reports(at):
    cycle, phase = divmod(int(at), CYCLE)
    step = phase // STEP  # 0 raised and being built, 1 landed, 2 deployed, 3 verified
    channel, title = CHANNELS[cycle % len(CHANNELS)]
    n = 200 + cycle % 100
    label = f"channel:{channel}"
    # When the step began: the deployment's time, then the verification's.
    stamp = datetime.fromtimestamp(at - phase % STEP, timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    dev_tasks = [] if step == 3 else [{"id": str(n), "title": title, "labels": [label, "bug"], "priority": "P1"}]
    dev = {
        "schema": "agents-village/v1",
        "project": "relay",
        "plane": "development",
        "framework": "mock",
        "coordinator": {"id": "manager", "name": "manager", "agents": [{"name": "manager", "state": "working"}]},
        "workstreams": [{
            "id": "/src/relay-wt-1",
            "task": str(n) if step == 0 else None,
            "agents": [{"name": "worker-1", "state": "working" if step == 0 else "idle", "action": "writing" if step == 0 else None}],
        }],
        "tasks": dev_tasks,
        "pipeline": {
            "integration": "main",
            "gate": "green",
            "landed": [str(n)] if step == 1 else [],
            "stages": [{"name": "prod", "tasks": [str(n)] if step == 2 else [], "sha": RELEASE, "deployed_at": stamp}],
            "release": RELEASE,
        },
    }
    task = {"id": str(n), "title": title, "labels": [label], "state": "claimed", "raised_by": f"{channel}-1"}
    if step == 3:
        task.update(state="done", verified_at=stamp)
    platform = {
        "schema": "agents-village/v1",
        "project": "agentex:relay",
        "plane": "runtime",
        "framework": "mock",
        "workstreams": [
            {"id": f"{c}-1", "name": c, "title": f"{c}-1", "state": "working", "action": "thinking"} for c, _ in CHANNELS
        ],
        "tasks": [task],
        "pipeline": {"gate": "green"},
        "builds": [RELEASE],
    }
    return {"reports": [dev, platform]}


if __name__ == "__main__":
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--at", type=float, help="the time to tell the story at, in seconds since the epoch")
    at = ap.parse_args().at
    print(json.dumps(reports(time.time() if at is None else at)))
