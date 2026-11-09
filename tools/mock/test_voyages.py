"""Tests for the scripted Deployments (voyages.py).

    python3 -m unittest discover -s tools/mock -p 'test_voyages.py'
"""

import json
import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import voyages  # noqa: E402
from voyages import FINISHED, IN_PROGRESS, PERIOD, VoyageScript  # noqa: E402

T0 = 1_800_000_000


def states(script, env):
    """Each deployment's latest status, newest deployment first."""
    return [(d["sha"], d["statuses"][0]["state"]) for d in script.state()[env]]


class VoyageScriptTest(unittest.TestCase):
    def setUp(self):
        self.tip = "a1"
        self.released = []
        self.s = VoyageScript(lambda: self.tip, T0, released=self.released.append)

    def at(self, seconds):
        self.s.tick(T0 + seconds)

    def test_a_deploy_to_nuc_1_loads_sails_and_docks(self):
        self.at(0)
        self.assertEqual(states(self.s, "nuc-1"), [("a1", "pending")])
        self.at(IN_PROGRESS)
        self.assertEqual(states(self.s, "nuc-1"), [("a1", "in_progress")])
        self.at(FINISHED)
        self.assertEqual(states(self.s, "nuc-1"), [("a1", "success")])
        self.assertEqual(self.s.current("nuc-1"), "a1")
        self.assertEqual(self.s.current("release"), None, "nothing promoted yet")

    def test_a_promotion_ships_what_nuc_1_runs(self):
        self.at(0)
        self.at(FINISHED)
        self.tip = "b2"  # newer work landed: the promotion still ships what nuc-1 runs
        self.at(40)
        d = self.s.state()["release"][0]
        self.assertEqual((d["sha"], d["statuses"][0]["state"], d["description"]), ("a1", "pending", "Release v0.1.0"))
        self.assertEqual(self.released, [])
        self.at(40 + FINISHED)
        d = self.s.state()["release"][0]
        self.assertEqual(d["statuses"][0], {"state": "success", "created_at": voyages.iso(T0 + 40 + FINISHED),
                                            "log_url": "https://github.com/sim/shop/releases/tag/v0.1.0"})
        self.assertEqual(self.released, ["a1"])

    def test_a_failed_deploy_leaves_nuc_1_on_what_it_ran(self):
        self.at(0)
        self.at(FINISHED)
        self.tip = "b2"
        self.at(80)
        self.at(80 + FINISHED)
        self.assertEqual(states(self.s, "nuc-1"), [("b2", "failure"), ("a1", "success")])
        self.assertEqual(self.s.current("nuc-1"), "a1")

    def test_the_next_success_supersedes_the_last(self):
        for t in (0, FINISHED):
            self.at(t)
        self.tip = "b2"
        for t in (80, 80 + FINISHED):
            self.at(t)
        self.tip = "c3"
        self.at(PERIOD)
        self.at(PERIOD + FINISHED)
        self.assertEqual(states(self.s, "nuc-1"), [("c3", "success"), ("b2", "failure"), ("a1", "inactive")])
        self.assertEqual(self.s.current("nuc-1"), "c3")

    def test_a_promotion_waits_when_nuc_1_runs_nothing(self):
        self.tip = None  # nothing landed: no deploy, so nothing to promote either
        self.at(0)
        self.at(40)
        self.assertEqual(self.s.state(), {"nuc-1": [], "release": []})

    def test_a_late_tick_catches_up_with_the_right_times(self):
        self.at(FINISHED + 5)  # the loop slept through the whole deploy
        d = self.s.state()["nuc-1"][0]
        self.assertEqual([s["state"] for s in d["statuses"]], ["success", "in_progress", "pending"])
        self.assertEqual(d["statuses"][0]["created_at"], voyages.iso(T0 + FINISHED))

    def test_each_step_runs_once_per_loop(self):
        for t in range(0, PERIOD * 2, 3):
            self.at(t)
        nuc = self.s.state()["nuc-1"]
        self.assertEqual(len(nuc), 4, "two deploys a loop: one that docks, one that fails")
        self.assertEqual(len(self.s.state()["release"]), 2)
        self.assertEqual([d["description"] for d in self.s.state()["release"]], ["Release v0.2.0", "Release v0.1.0"])

    def test_state_is_what_the_gh_shim_serves(self):
        for t in (0, FINISHED, 40):
            self.at(t)
        served = json.loads(json.dumps(self.s.state()))
        d = served["release"][0]
        self.assertEqual(set(d), {"id", "sha", "environment", "created_at", "creator", "statuses", "description"})
        self.assertEqual((d["environment"], d["creator"]), ("release", {"login": "deploy-bot"}))
        ids = [d["id"] for ds in served.values() for d in ds]
        self.assertEqual(len(ids), len(set(ids)), "ids are unique across environments")

    def test_release_now_promotes_what_nuc_1_runs(self):
        self.assertIsNone(self.s.promote_now(T0), "nothing on nuc-1 yet")
        self.at(0)
        self.at(FINISHED)
        self.assertEqual(self.s.promote_now(T0 + 20), "a1")
        self.at(20 + FINISHED)
        self.assertEqual(states(self.s, "release"), [("a1", "success")])
        self.assertEqual(self.released, ["a1"])

    def test_config_is_this_repos_environments(self):
        envs = voyages.config()["environments"]
        self.assertEqual([(e["name"], e["requires"], e["source"]) for e in envs],
                         [("nuc-1", "integration", "deployment"), ("release", "nuc-1", "deployment")])


if __name__ == "__main__":
    unittest.main()
