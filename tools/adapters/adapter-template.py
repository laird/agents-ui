#!/usr/bin/env python3
"""A skeleton agents-village adapter: copy it, replace read_framework().

Run it from agents-tui:

    agents-tui --headless --framework exec:./tools/adapters/adapter-template.py

It prints one report (agents-village/v1, see docs/village-protocol.md) on
stdout. agents-tui reruns it every few seconds, so keep it quick and read-only.
"""
import json
import sys


def read_framework():
    """Return what your framework is doing right now, in its own words.

    Replace this with calls to your framework's CLI, API or state files.
    """
    return {
        "name": "my-crew",
        "lead": {"state": "running"},
        "runs": [
            {"id": "run-1", "type": "ResearchAgent", "status": "RUNNING", "step": "web_search"},
            {"id": "run-2", "type": "ReviewAgent", "status": "AWAITING_APPROVAL", "step": None},
        ],
        "failed": [{"id": "run-0", "type": "ResearchAgent"}],
        "version": "9f3c2e1d",
    }


def action(step):
    """Map a step name to a village action; unknown steps run."""
    words = {
        "search": "searching", "fetch": "searching", "read": "reading",
        "write": "writing", "llm": "thinking", "plan": "thinking",
        "send": "messaging", "notify": "messaging",
    }
    step = (step or "").lower()
    return next((a for w, a in words.items() if w in step), "running" if step else None)


def report(fw):
    return {
        "schema": "agents-village/v1",
        "project": f"crew:{fw['name']}",
        "plane": "runtime",
        "framework": "my-framework",
        "coordinator": {"id": "lead", "name": "lead", "agents": [{"name": "lead", "state": fw["lead"]["state"]}]},
        "workstreams": [
            {
                "id": run["id"],
                "name": run["type"],
                "title": run["id"],
                "state": run["status"],  # RUNNING, AWAITING_APPROVAL, ... are understood as-is
                "action": action(run["step"]),
                "agents": [{"name": run["type"], "state": run["status"]}],
            }
            for run in fw["runs"]
        ],
        "tasks": [
            {"id": f["id"], "title": f"{f['type']} {f['id']}", "state": "failed"} for f in fw["failed"]
        ],
        "builds": [fw["version"]],
    }


if __name__ == "__main__":
    try:
        print(json.dumps(report(read_framework())))
    except Exception as e:  # say why on stderr; agents-tui shows it on the board
        print(f"adapter failed: {e}", file=sys.stderr)
        sys.exit(1)
