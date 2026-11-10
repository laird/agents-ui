"""Tests for the CONFLICTS=1 merge-gate script (conflicts.py) and the gate
simulation that plays it (swarm_sim.GateSim with a conflicts.Script).

    python3 -m unittest discover -s tools/mock -p 'test_conflicts.py'
"""

import shutil
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import conflicts  # noqa: E402
import swarm_sim  # noqa: E402


class ScriptTest(unittest.TestCase):
    def test_each_issue_takes_the_next_story_and_keeps_it(self):
        s = conflicts.Script()
        # Issues interleave at the gate; each follows its own story.
        self.assertEqual(s.attempt(1), (("conflict", ["src/web/server.rs", "src/web/village/lifecycle.js"]), 1))
        self.assertEqual(s.attempt(2), (("fail", None), 1))
        self.assertEqual(s.attempt(1), (("pass", 0), 2), "fixed and landed on the second attempt")
        self.assertEqual(s.attempt(3), (("pass", 2), 1), "push rejected twice, then lands")
        self.assertEqual([s.attempt(4)[0][0] for _ in range(4)], ["conflict", "conflict", "conflict", "pass"])
        self.assertEqual(s.story(4), "hotspot")
        # The script goes round again.
        self.assertEqual(s.attempt(5)[0][0], "conflict")
        self.assertEqual(s.story(5), "conflict")

    def test_the_files_each_attempt_leaves(self):
        f = conflicts.attempt_files(7, ("conflict", [conflicts.HOTSPOT]))
        self.assertEqual(f["exit"], 1)
        self.assertIn(f"CONFLICT (content): Merge conflict in {conflicts.HOTSPOT}\n", f["log"])
        self.assertIn("❌ Merge conflict integrating 'master' into feature/issue-7.", f["log"])
        f = conflicts.attempt_files(7, ("pass", 2))
        self.assertEqual(f["exit"], 0)
        self.assertEqual(f["log"].count("🧪 Gating the integrated tree"), 3)
        self.assertIn("re-syncing (attempt 2/5)", f["log"])
        self.assertIn("✅ Issue #7 merged into 'master'", f["log"])
        self.assertEqual(conflicts.attempt_files(7, ("fail", None))["exit"], 2)
        self.assertTrue(conflicts.conflict_comment(7).startswith("⚠️ **Auto-merge to `master` hit a conflict**"))


class FakeSwarm:
    """What GateSim needs of a swarm: its repo (for the job prefix), issues, and landing."""

    def __init__(self, root):
        self.repo = root / "shop"
        self.issues = []
        self.landed = []

    def land_issue(self, n, close=True):
        self.landed.append(n)


class GateSimTest(unittest.TestCase):
    def setUp(self):
        self.dir = Path(tempfile.mkdtemp())
        self.swarm = FakeSwarm(self.dir)
        self.gate = swarm_sim.GateSim(self.swarm, self.dir / "tmp", conflicts.Script())

    def tearDown(self):
        self.gate.stop()
        shutil.rmtree(self.dir)

    def files(self, n):
        p = lambda ext: self.dir / "tmp" / f"{self.gate.prefix}{n}.{ext}"
        exit_ = p("exit").read_text().strip() if p("exit").exists() else None
        return p("log").read_text(), exit_

    def hand_off(self, n, t):
        self.swarm.issues.append({"number": n, "title": f"Issue {n}", "labels": ["working"]})
        self.gate.submit(n, t)

    def test_a_conflict_bounces_back_with_its_files_and_a_comment(self):
        self.hand_off(1, 0)
        self.gate.tick(0)
        log, exit_ = self.files(1)
        self.assertEqual((log, exit_), ("", None), "queued")
        self.gate.tick(4)
        self.assertIn("🔀 Merging", self.files(1)[0])
        self.gate.tick(7)
        log, exit_ = self.files(1)
        self.assertEqual(exit_, "1")
        self.assertIn("CONFLICT (content): Merge conflict in src/web/server.rs", log)
        issue = self.swarm.issues[0]
        self.assertIn("needs-clarification", issue["labels"])
        self.assertNotIn("merging", issue["labels"])
        self.assertIn("hit a conflict", issue["comments"][0]["body"])
        # Back from its worker, the second attempt gates and lands.
        issue["labels"] = ["working"]
        self.gate.submit(1, 40)
        for t in (40, 44, 47):
            self.gate.tick(t)
        self.assertIn("▶️  Gate slot 1 acquired", self.files(1)[0])
        self.gate.tick(61)
        log, exit_ = self.files(1)
        self.assertEqual(exit_, "0")
        self.assertNotIn("CONFLICT", log, "a new attempt's log starts afresh")
        self.assertEqual(self.swarm.landed, [1])

    def test_push_retries_regate_before_landing(self):
        self.gate.script.turn = 2  # the push-retry story
        self.hand_off(3, 0)
        for t in (0, 4, 7):
            self.gate.tick(t)
        self.gate.tick(21)
        log, exit_ = self.files(3)
        self.assertIsNone(exit_)
        self.assertIn("re-syncing (attempt 1/5)", log)
        self.gate.tick(35)
        self.assertIn("re-syncing (attempt 2/5)", self.files(3)[0])
        self.gate.tick(49)
        log, exit_ = self.files(3)
        self.assertEqual(exit_, "0")
        self.assertEqual(log.count("🧪 Gating the integrated tree"), 3)
        self.assertEqual(self.swarm.landed, [3])

    def test_no_trains_while_scripted(self):
        # Without a script every third hand-off waits for a partner; with one, each goes alone.
        self.gate.count = 2
        self.hand_off(5, 0)
        self.gate.tick(0)
        self.assertEqual([j["members"] for j in self.gate.jobs], [[5]])


if __name__ == "__main__":
    unittest.main()
