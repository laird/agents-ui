"""CONFLICTS=1: merge-gate bounces, scripted, for watching #430's drawing.

swarm_sim.py --conflicts sends "shop"'s finished work through its simulated
merge gate (GateSim, as --gate does), but with each issue's attempts taken
from this script instead of the gate's rotating outcomes. Every issue that
reaches the gate gets the next story, in turn:

  conflict    conflicts (two files), goes back to its worker, and lands on
              the second attempt;
  gate-fail   fails the gate on the integrated tree once, then lands;
  push-retry  passes the gate, but its push is rejected twice and re-synced,
              re-gating each time, then lands;
  hotspot     conflicts three times on the same file, then lands: that file
              tops the hotspots.

The merge-job files are written as autocoder's merge-to-integration.sh
writes them (`.log` lines, `.exit` codes: 0 landed, 1 conflict, 2 gate
failed), and a conflict leaves the issue comment autocoder leaves.

    python3 tools/mock/conflicts.py --fixtures   # each story's attempts as JSON: the logs and exit codes
"""

import argparse
import json

HOTSPOT = "src/web/village/main.js"
TESTS = 745

# (story, attempts): each attempt is ("conflict", files), ("fail", None) or ("pass", re-syncs).
STORIES = [
    ("conflict", [("conflict", ["src/web/server.rs", "src/web/village/lifecycle.js"]), ("pass", 0)]),
    ("gate-fail", [("fail", None), ("pass", 0)]),
    ("push-retry", [("pass", 2)]),
    ("hotspot", [("conflict", [HOTSPOT]), ("conflict", [HOTSPOT]), ("conflict", [HOTSPOT]), ("pass", 0)]),
]


class Script:
    """Which attempt an issue is on: a new issue takes the next story, in turn."""

    def __init__(self):
        self.turn = 0
        self.plans = {}  # issue -> (story, attempts still to come)
        self.tries = {}  # issue -> attempts made

    def attempt(self, n):
        """The outcome of issue n's next attempt at the gate, and its number (1, 2, ...)."""
        if n not in self.plans or not self.plans[n][1]:
            story, attempts = STORIES[self.turn % len(STORIES)]
            self.turn += 1
            self.plans[n] = (story, list(attempts))
            self.tries[n] = 0
        story, left = self.plans[n]
        self.tries[n] += 1
        return left.pop(0), self.tries[n]

    def story(self, n):
        return self.plans.get(n, (None,))[0]


# -- what the merge job writes, phase by phase (merge-to-integration.sh's words) --

def merging(issue):
    return f"🔀 Merging [{issue}] onto master in a throwaway worktree\n"


def conflict(issue, files):
    return ("".join(f"Auto-merging {f}\nCONFLICT (content): Merge conflict in {f}\n" for f in files)
            + f"❌ Merge conflict integrating 'master' into feature/issue-{issue}.\n")


def gating(slot, tree, clock):
    return f"▶️  Gate slot {slot} acquired ({clock})\n🧪 Gating the integrated tree {tree}: cargo test\nrunning {TESTS} tests\n"


def passed_run():
    return f"test result: ok. {TESTS} passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 14.00s\n"


def resync(k, tree):
    """A rejected push after a green gate: re-sync, and gate the merged tree again."""
    return (passed_run()
            + f"⚠️  Push to 'master' rejected (a sibling worker likely pushed first); re-syncing (attempt {k}/5)…\n"
            + f"ℹ️  Re-sync brings in src/web/village/registry.js; re-gating the merged tree\n"
            + f"🧪 Gating the integrated tree {tree}: cargo test\nrunning {TESTS} tests\n")


def landed(issue):
    return passed_run() + f"✅ Issue #{issue} merged into 'master' and verified: tests passed on the published tree.\n✅ Issue #{issue} closed.\n"


def failed():
    return (f"test result: FAILED. {TESTS - 2} passed; 2 failed; 0 ignored\n❌ Tests fail on the integrated tree.\n"
            "❌ origin/master passes alone, so this change broke it. NOT pushing.\n")


def conflict_comment(issue):
    """The comment autocoder leaves on an issue whose merge conflicted."""
    return (f"⚠️ **Auto-merge to `master` hit a conflict** on branch `feature/issue-{issue}` and needs manual resolution. "
            f"The fix is committed on `feature/issue-{issue}`; resolve against `master` and push, then close this issue.")


def attempt_files(issue, outcome, slot=1):
    """The `.log` and `.exit` a finished attempt leaves, as GateSim writes them step by step."""
    kind, arg = outcome
    log = merging(issue)
    if kind == "conflict":
        return {"log": log + conflict(issue, arg), "exit": 1}
    log += gating(slot, "aaaa0001", "12:00:00")
    if kind == "fail":
        return {"log": log + failed(), "exit": 2}
    for k in range(1, arg + 1):
        log += resync(k, f"aaaa000{k + 1}")
    return {"log": log + landed(issue), "exit": 0}


def fixtures(issue_from=900):
    """Every story's attempts, as the files they leave: what the tests parse."""
    out = []
    for k, (story, attempts) in enumerate(STORIES):
        n = issue_from + k
        out.append({"story": story, "issue": n, "attempts": [attempt_files(n, a) for a in attempts]})
    return out


if __name__ == "__main__":
    ap = argparse.ArgumentParser(description="The CONFLICTS=1 merge-gate script.")
    ap.add_argument("--fixtures", action="store_true", help="print each story's attempts (logs and exit codes) as JSON")
    if ap.parse_args().fixtures:
        print(json.dumps(fixtures(), indent=1, ensure_ascii=False))
