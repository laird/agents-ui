"""Tests for history-replay.py: frame -> report, the replay clock, the frame index.

    python3 -m unittest discover -s tools/adapters -p 'test_history_replay.py'
"""
import contextlib
import gzip
import importlib.util
import io
import json
import os
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "mock"))
import history as hm  # noqa: E402

_spec = importlib.util.spec_from_file_location("history_replay", os.path.join(HERE, "history-replay.py"))
hr = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(hr)

BASE = "https://github.com/o/shop"


def make_history(stops=None, contains=None):
    """A feature (#10) with three subtasks: #11 worked then landed, #12 blocked
    on design, #13 closed as not planned at 200; and #14, unrelated."""
    L = lambda t, *names: [(t, "add", n) for n in names]
    parent = hm.Issue(10, "Saved carts", f"{BASE}/issues/10", "", 0, None, None, L(0, "enhancement", "decomposed"))
    a = hm.Issue(11, "Cart table", "", "", 0, 400, "COMPLETED",
                 L(0, "enhancement", "P1") + [(100, "add", "working"), (300, "remove", "working")])
    b = hm.Issue(12, "Restore API", "", "", 0, None, None, L(0, "bug", "P2") + [(150, "add", "needs-design")])
    c = hm.Issue(13, "Excel export", "", "", 0, 200, "NOT_PLANNED", L(0, "enhancement", "P3"))
    d = hm.Issue(14, "Unrelated", "", "", 0, None, None, L(0, "documentation", "P0"))
    for k in (a, b, c):
        k.parent, k.linked_by, k.linked_at = 10, "sub-issues", 0
    return hm.History("shop", "/src/shop", "master", [parent, a, b, c, d], {11: (350, "sha11")},
                      stops or [hm.Stop("master-green")], contains=contains or (lambda commit, sha: False))


def village(t, **kw):
    return hm.frame_at(make_history(**kw), t)["villages"][0]


def by_id(r):
    return {t["id"]: t for t in r["tasks"]}


class Report(unittest.TestCase):
    def test_working_house_is_an_interactive_workstream(self):
        r = hr.report(village(120), "replay:", 4, 10, BASE)
        self.assertEqual((r["schema"], r["project"], r["plane"]), ("agents-village/v1", "replay:shop", "development"))
        self.assertEqual(r["framework"], "history replay 5/10")
        (ws,) = r["workstreams"]
        self.assertEqual((ws["id"], ws["name"], ws["task"], ws["title"]), ("history/worker-1", "worker-1", "11",
                                                                           "Cart table"))
        agent = ws["agents"][0]
        self.assertEqual((agent["state"], agent["action"]), ("working", "writing"))
        self.assertEqual(agent["detail"], "working on #11: Cart table")
        self.assertNotIn("links", ws, "the village links the house to its task's url itself")
        self.assertEqual(r["coordinator"]["agents"][0]["state"], "working")
        self.assertEqual(r["links"], [{"label": "Repository", "url": BASE}])

    def test_tasks_carry_states_parents_and_issue_urls(self):
        t = by_id(hr.report(village(120), "replay:", 0, 1, BASE))
        self.assertEqual({k: v["state"] for k, v in t.items()},
                         {"10": "claimed", "11": "claimed", "12": "open", "13": "open", "14": "open"})
        self.assertEqual({k: v.get("parent") for k, v in t.items()},
                         {"10": None, "11": "10", "12": "10", "13": "10", "14": None})
        self.assertEqual(t["14"]["labels"], ["docs"])
        self.assertEqual(t["14"]["priority"], "P0")
        self.assertEqual(t["10"]["labels"], ["decomposed"])
        self.assertEqual(t["12"]["url"], f"{BASE}/issues/12")
        self.assertEqual(t["10"]["url"], f"{BASE}/issues/10")

    def test_blocked_landed_and_not_planned(self):
        r = hr.report(village(360), "replay:", 0, 1, BASE)
        t = by_id(r)
        self.assertEqual(t["12"]["state"], "blocked")
        self.assertEqual(t["12"]["labels"], ["needs-design"])
        self.assertEqual((t["11"]["state"], t["11"]["parent"]), ("done", "10"))
        self.assertNotIn("13", t, "a subtask closed as not planned would count toward the feature")
        self.assertEqual(r["pipeline"]["landed"], ["11"])
        self.assertEqual(r["workstreams"][0].get("task"), None, "the house is idle once the claim ends")

    def test_pipeline_stages(self):
        stops = [hm.Stop("master-green", branch_head="g"), hm.Stop("prod", deployments=[(380, "sha11")])]
        r = hr.report(village(390, stops=stops, contains=lambda commit, sha: True), "replay:", 0, 1, BASE)
        p = r["pipeline"]
        self.assertEqual((p["integration"], p["gate"], p["landed"], p["promotion"]), ("master", "green", [], "subtask"))
        self.assertEqual(p["stages"], [{"name": "master-green", "tasks": [], "source": "branch"},
                                       {"name": "prod", "tasks": ["11"], "source": "deployment", "sha": "sha11"}])
        self.assertEqual(p["release"], "sha11")
        self.assertEqual(by_id(r)["11"]["state"], "done")

    def test_unknown_stages_leave_the_gate_unknown(self):
        p = hr.report(village(360), "replay:", 0, 1, BASE)["pipeline"]
        self.assertEqual(p["gate"], "unknown")
        self.assertEqual(p["stages"], [{"name": "master-green", "tasks": [], "source": "unknown"}])

    def test_without_a_github_origin_there_are_no_made_up_links(self):
        r = hr.report(village(120), "replay:", 0, 1, None)
        self.assertEqual(r["links"], [])
        t = by_id(r)
        self.assertNotIn("url", t["12"])
        self.assertEqual(t["10"]["url"], f"{BASE}/issues/10", "the issue's own url is kept")

    def test_prefix(self):
        self.assertEqual(hr.report(village(0), "h/", 0, 1, None)["project"], "h/shop")


class Clock(unittest.TestCase):
    def test_position_advances_one_frame_per_interval_and_loops(self):
        self.assertEqual([hr.position(100, 100 + s, 6, 4) for s in (0, 5.9, 6, 13, 23.9, 24, 30)],
                         [0, 0, 1, 2, 3, 0, 1])
        self.assertEqual(hr.position(100, 50, 6, 4), 0, "a clock in the future shows the first frame")
        self.assertEqual(hr.position(100, 200, 6, 0), 0)

    def test_the_clock_starts_once_and_restart_starts_it_over(self):
        with tempfile.TemporaryDirectory() as state:
            self.assertEqual(hr.clock_start("a.json.gz", state, now=1000), 1000)
            self.assertEqual(hr.clock_start("a.json.gz", state, now=2000), 1000)
            self.assertEqual(hr.clock_start("b.json.gz", state, now=2000), 2000, "each recording has its own clock")
            self.assertEqual(hr.clock_start("a.json.gz", state, restart=True, now=3000), 3000)
            self.assertEqual(hr.clock_start("a.json.gz", state, now=4000), 3000)

    def test_state_dir_defaults_to_xdg_state(self):
        with mock.patch.dict(os.environ, {"XDG_STATE_HOME": "/x/state"}):
            self.assertEqual(hr.state_dir(), "/x/state/agents-tui/history-replay")
            self.assertEqual(hr.state_dir("/given"), "/given")


def write_recording(path, frames, run="history"):
    with open(path, "wb") as f:
        f.write(gzip.compress(json.dumps({run: frames}).encode()))


class Adapter(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.dir = self.tmp.name
        self.state = os.path.join(self.dir, "state")
        h = make_history()
        self.frames = [hm.frame_at(h, t) for t in (0, 120, 360)]

    def tearDown(self):
        self.tmp.cleanup()

    def run_main(self, *args, now):
        out = io.StringIO()
        with contextlib.redirect_stdout(out):
            hr.main([*args, "--state", self.state], now=now)
        return json.loads(out.getvalue())

    def test_the_village_advances_with_the_clock(self):
        rec = os.path.join(self.dir, "shop.json.gz")
        write_recording(rec, self.frames)
        first = self.run_main(rec, "--every", "6", now=1000)
        self.assertEqual((first["project"], first["framework"]), ("replay:shop", "history replay 1/3"))
        later = self.run_main(rec, "--every", "6", now=1013)
        self.assertEqual(later["framework"], "history replay 3/3")
        self.assertEqual(self.run_main(rec, "--every", "6", now=1018)["framework"], "history replay 1/3", "loops")
        self.assertEqual(self.run_main(rec, "--restart", now=1013)["framework"], "history replay 1/3")
        self.assertFalse(any(n.endswith(".clock") for n in os.listdir(self.dir)), "state stays in --state")

    def test_two_recordings_give_two_villages(self):
        a, b = os.path.join(self.dir, "a.json.gz"), os.path.join(self.dir, "b.json.gz")
        write_recording(a, self.frames)
        other = json.loads(json.dumps(self.frames))
        for f in other:
            f["villages"][0]["project"] = "laird-agents"
        write_recording(b, other)
        out = self.run_main(a, b, now=1000)
        self.assertEqual([r["project"] for r in out], ["replay:shop", "replay:laird-agents"])
        same = self.run_main(a, a, now=1000)
        self.assertEqual([r["project"] for r in same], ["replay:shop", "replay:shop (2)"])

    def test_frames_are_indexed_once_and_rebuilt_when_the_recording_changes(self):
        rec = os.path.join(self.dir, "shop.json.gz")
        write_recording(rec, self.frames)
        self.run_main(rec, now=1000)
        with mock.patch.object(hr, "read_villages", side_effect=AssertionError("re-read")):
            r = self.run_main(rec, "--every", "6", now=1007)
        self.assertEqual(r["framework"], "history replay 2/3")
        write_recording(rec, self.frames[:2])
        os.utime(rec, (2000, 2000))
        self.assertEqual(self.run_main(rec, now=1000)["framework"], "history replay 1/2")

    def test_frame_per_line_recordings(self):
        rec = os.path.join(self.dir, "shop.jsonl")
        with open(rec, "w") as f:
            f.write(json.dumps(self.frames[0]) + "\n" + json.dumps(self.frames[1]["villages"][0]) + "\n")
        self.assertNotIn("task", self.run_main(rec, "--every", "6", now=1000)["workstreams"][0])
        self.assertEqual(self.run_main(rec, "--every", "6", now=1006)["workstreams"][0]["task"], "11")

    def test_an_empty_recording_is_an_empty_village(self):
        rec = os.path.join(self.dir, "empty.json.gz")
        write_recording(rec, [])
        r = self.run_main(rec, now=1000)
        self.assertEqual((r["project"], r["framework"]), ("replay:empty", "history replay 0/0"))


class RepoUrl(unittest.TestCase):
    def test_origin_remotes_become_https_urls(self):
        with tempfile.TemporaryDirectory() as repo:
            subprocess.run(["git", "init", "-q", repo], check=True)
            for remote, want in [("git@github.com:o/shop.git", BASE), ("https://github.com/o/shop.git", BASE),
                                 ("ssh://git@github.com/o/shop.git", BASE), ("/local/bare", None)]:
                subprocess.run(["git", "-C", repo, "remote", "remove", "origin"], capture_output=True)
                subprocess.run(["git", "-C", repo, "remote", "add", "origin", remote], check=True)
                self.assertEqual(hr.repo_url(repo), want, remote)
        self.assertIsNone(hr.repo_url("/nonexistent/repo"))


if __name__ == "__main__":
    unittest.main()
