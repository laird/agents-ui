"""Tests for history.py: house assignment and folding events into frames.

    python3 -m unittest discover -s tools/mock -p 'test_history.py'
    (cd tools/mock && python3 -m unittest test_history)
"""
import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import history as hm  # noqa: E402


def busy_at(claims, t):
    return [(n, k) for s, e, n, k in claims if s <= t and (e is None or t < e)]


class AssignHouses(unittest.TestCase):
    def test_overlapping_claims_get_separate_houses_and_free_houses_are_reused(self):
        claims, houses = hm.assign_houses([(0, 10, "a"), (2, 8, "b"), (9, 20, "c"), (11, 12, "d")])
        by = {n: k for _, _, n, k in claims}
        self.assertEqual(houses, 2)
        self.assertNotEqual(by["a"], by["b"])
        self.assertEqual(by["c"], by["b"], "b's house is free at 9, a's is not")
        self.assertEqual(by["d"], by["a"])

    def test_never_more_houses_than_peak_and_never_two_claims_in_one_house(self):
        import random
        rnd = random.Random(7)
        for _ in range(200):
            iv = []
            for k in range(rnd.randint(1, 12)):
                s = rnd.uniform(0, 100)
                iv.append((s, None if rnd.random() < 0.1 else s + rnd.uniform(0.1, 40), k))
            claims, houses = hm.assign_houses(iv)
            peak = max(len(busy_at(claims, s)) for s, _, _, _ in claims)
            self.assertEqual(houses, peak)
            for s, _, _, _ in claims:
                ks = [k for _, k in busy_at(claims, s)]
                self.assertEqual(len(ks), len(set(ks)), "two claims share a house")

    def test_a_reclaimed_issue_returns_to_its_house_when_free(self):
        claims, _ = hm.assign_houses([(0, 10, "a"), (1, 5, "b"), (6, 30, "c"), (12, 20, "b")])
        by = [(n, k) for _, _, n, k in claims]
        self.assertEqual(by[1], ("b", 1))
        self.assertEqual(by[3], ("b", 0), "b's house was taken by c; it gets the free one")
        claims, _ = hm.assign_houses([(0, 10, "a"), (1, 5, "b"), (12, 20, "b")])
        self.assertEqual([k for _, _, n, k in claims if n == "b"], [1, 1], "lowest-free would be 0; b keeps 1")

    def test_open_claims_never_free_their_house(self):
        claims, houses = hm.assign_houses([(0, None, "a"), (50, 60, "b")])
        self.assertEqual(houses, 2)


class Claims(unittest.TestCase):
    def issue(self, events, closed=None, releases=()):
        i = hm.Issue(1, "x", created=0, closed=closed, labels=events)
        i.releases = list(releases)
        return i

    def test_working_label_spans_are_claims_ended_by_close(self):
        i = self.issue([(10, "add", "working"), (300, "remove", "working"), (500, "add", "working"),
                        (900, "remove", "working")], closed=800)
        self.assertEqual(i.claims(), [(10, 300), (500, 800)])

    def test_short_relabels_by_the_merge_job_are_one_claim(self):
        i = self.issue([(10, "add", "working"), (300, "remove", "working"), (330, "add", "working")], closed=600)
        self.assertEqual(i.claims(), [(10, 600)])

    def test_a_release_comment_ends_the_claim_however_short_the_gap(self):
        i = self.issue([(10, "add", "working"), (22, "remove", "working"), (70, "add", "working")], closed=600,
                       releases=[21])
        self.assertEqual(i.claims(), [(10, 22), (70, 600)])

    def recorded(self, events, closed=None, releases=(), starts=(), commits=()):
        """An issue read from GitHub: its comments are known, so spans need evidence."""
        i = self.issue(events, closed, releases)
        i.starts, i.commits = list(starts), list(commits)
        return i

    def test_a_stranded_label_with_no_worker_is_no_claim(self):
        i = self.recorded([(10, "add", "working"), (400, "remove", "working")])
        self.assertEqual(i.claims(), [])
        self.assertEqual(self.recorded([(10, "add", "working")]).claims(), [], "still stranded")

    def test_a_claim_released_for_dependencies_is_still_a_short_one(self):
        i = self.recorded([(10, "add", "working"), (40, "remove", "working")], releases=[39])
        self.assertEqual(i.claims(), [(10, 40)])

    def test_a_start_comment_or_a_commit_shows_a_worker_held_it(self):
        ev = [(10, "add", "working"), (400, "remove", "working")]
        self.assertEqual(self.recorded(ev, starts=[14]).claims(), [(10, 400)])
        self.assertEqual(self.recorded(ev, commits=[200]).claims(), [(10, 400)])
        self.assertEqual(self.recorded(ev, commits=[500]).claims(), [], "a commit after the span is not evidence")

    def test_a_label_put_back_with_no_worker_does_not_extend_the_claim(self):
        # #346 wave: the worker released, a stranded label followed; the real claim came later.
        i = self.recorded([(10, "add", "working"), (40, "remove", "working"), (50, "add", "working"),
                           (360, "remove", "working"), (380, "add", "working")], closed=900,
                          releases=[39], starts=[385])
        self.assertEqual(i.claims(), [(10, 40), (380, 900)])
        # #347: the worker handed off, and the label came back 30 s later with no worker.
        i = self.recorded([(10, "add", "working"), (300, "remove", "working"), (330, "add", "working")],
                          closed=600, starts=[12])
        self.assertEqual(i.claims(), [(10, 300)])

    def test_work_started_comments(self):
        for body in ("🤖 **Automated Fix Started**\n\nStarting…",
                     "🔒 [autocoder-claim] Starting fix for issue #350 — lock established",
                     "🚀 **Enhancement Implementation Started**"):
            self.assertTrue(hm.WORK_STARTED.search(body), body)
        self.assertFalse(hm.WORK_STARTED.search("🔒 claim nuc-526287 1794150997174"), "the gate's claim alone")

    def test_commit_times_by_issue(self):
        log = ["a 200 190 Merge origin/master into feature/issue-7 (pre-integration sync)",
               "b 150 120 Fix #7: the thing", "c 100 90 Implement #8: other", "d 50 40 Tidy up"]
        self.assertEqual(hm.commit_times(log), {7: [190.0, 120.0], 8: [90.0]})

    def test_still_claimed(self):
        self.assertEqual(self.issue([(10, "add", "working")]).claims(), [(10, None)])

    def test_labels_at_follow_events_in_order(self):
        i = self.issue([(0, "add", "P1"), (0, "add", "enhancement"), (5, "add", "needs-design"), (9, "remove", "needs-design")])
        self.assertEqual(i.labels_at(6), ["P1", "enhancement", "needs-design"])
        self.assertEqual(i.labels_at(9), ["P1", "enhancement"])


class FrameTimes(unittest.TestCase):
    def test_steps_and_skips_quiet_stretches(self):
        times = hm.frame_times([0, 30, 5000], 0, 5100, 60, 600)
        self.assertEqual(times[:2], [0, 60])
        self.assertIn(4940, times, "a frame just before the next event")
        self.assertFalse(any(200 < t < 4900 for t in times), "the quiet stretch is skipped")
        self.assertEqual(times[-1], 5060)

    def test_stops_after_the_last_event_when_the_rest_is_quiet(self):
        self.assertEqual(hm.frame_times([10], 0, 10_000, 60, 600), [0, 60], "one frame after the last event")


class Landings(unittest.TestCase):
    def test_merge_into_feature_branch_wins_over_the_fix_commit(self):
        log = ["m1 200 190 Merge origin/master into feature/issue-350 (pre-integration sync)",
               "f1 150 140 Fix #350: Features in the village",
               "f2 100 90 Implement #349: banners",
               "x 50 40 Prototype run 2: unrelated"]
        out = hm.landings_from_log(log)
        self.assertEqual(out[350][:2], (200.0, "m1"))
        self.assertEqual(out[349][:2], (100.0, "f2"))
        self.assertNotIn(2, out)


def make_history(issues, landed=None, stops=None, contains=None):
    return hm.History("shop", "/src/shop", "master", issues, landed or {}, stops or [hm.Stop("master-green")],
                      contains=contains or (lambda commit, sha: False))


class Frames(unittest.TestCase):
    def setUp(self):
        L = lambda t, *names: [(t, "add", n) for n in names]
        self.parent = hm.Issue(10, "Saved carts", "https://github.com/o/shop/issues/10",
                               "Spec: [it](docs/specs/carts.md)", 0, None, None, L(0, "enhancement", "decomposed"))
        self.a = hm.Issue(11, "Cart table", "", "", 0, 400, "COMPLETED",
                          L(0, "enhancement", "P1") + [(100, "add", "working"), (400, "remove", "working")])
        self.b = hm.Issue(12, "Restore API", "", "", 0, None, None, L(0, "bug", "P2") + [(150, "add", "needs-design")])
        self.c = hm.Issue(13, "Excel export", "", "", 0, 200, "NOT_PLANNED", L(0, "enhancement", "P3"))
        self.d = hm.Issue(14, "Unrelated", "", "", 0, None, None, L(0, "documentation", "P0"))
        for k in (self.a, self.b, self.c):
            k.parent, k.linked_by, k.linked_at = 10, "sub-issues", 0
        self.h = make_history([self.parent, self.a, self.b, self.c, self.d], landed={11: (350, "sha11")})

    def village(self, t, window=0):
        return hm.frame_at(self.h, t, window)["villages"][0]

    def test_backlog_board_and_houses_as_they_were(self):
        v = self.village(120)
        self.assertEqual([h["issue"] for h in v["houses"]], [11])
        self.assertEqual(v["houses"][0]["kind"], "feature")
        self.assertEqual(v["houses"][0]["priority"], "P1")
        self.assertEqual([c["number"] for c in v["backlog"]], [14, 12, 13], "P0 first; the decomposed parent is not a crate")
        self.assertEqual(v["board"], [])
        v = self.village(160)
        self.assertEqual([(n["number"], n["label"]) for n in v["board"]], [(12, "needs-design")])
        self.assertEqual(v["manager"]["status"], "working")

    def test_landed_work_waits_at_the_quay_and_closed_issues_leave(self):
        v = self.village(360)
        self.assertEqual(v["pipeline"]["quay"], [11])
        self.assertEqual(v["pipeline"]["stops"][0]["source"], "unknown")
        self.assertEqual(v["pipeline"]["sentinel"], "unknown")
        v = self.village(450)
        self.assertEqual([h["issue"] for h in v["houses"]], [None])
        self.assertEqual(v["manager"]["status"], "idle")
        self.assertNotIn(13, [c["number"] for c in v["backlog"]], "closed as not planned")

    def test_features_count_subtasks_as_they_were(self):
        f = self.village(120)["features"][0]
        self.assertEqual((f["number"], f["linked_by"], f["promotion"]), (10, "sub-issues", "subtask"))
        self.assertEqual(f["spec_url"], "https://github.com/o/shop/blob/HEAD/docs/specs/carts.md")
        self.assertEqual(f["build"], {"landed": 0, "total": 3})
        self.assertEqual({s["number"]: s["where"] for s in f["subtasks"]}, {11: "house", 12: "backlog", 13: "backlog"})
        f = self.village(360)["features"][0]
        self.assertEqual(f["build"], {"landed": 1, "total": 2}, "13 was dropped at 200")
        subs = {s["number"]: s for s in f["subtasks"]}
        self.assertEqual((subs[11]["where"], subs[12]["where"], subs[12]["blocked"]), ("quay", "board", "needs-design"))
        self.assertTrue(subs[13]["dropped"])
        self.assertEqual(f["stuck"], [12])
        self.assertEqual(f["stages"], [{"name": "master-green", "reached": 0, "total": 2, "unknown": True}])
        self.assertFalse(f["live"])

    def test_a_claim_shorter_than_a_frame_still_shows(self):
        short = hm.Issue(20, "Docs", "", "", 0, None, None, [(500, "add", "working"), (512, "remove", "working")])
        short.releases = [511]
        h = make_history([short])
        at = lambda t, w: [x["issue"] for x in hm.frame_at(h, t, w)["villages"][0]["houses"]]
        self.assertEqual(at(540, 60), [20])
        self.assertEqual(at(540, 0), [None])
        self.assertEqual(at(600, 60), [None])

    def test_deployments_place_landed_work_and_say_where_they_come_from(self):
        stops = [hm.Stop("master-green", branch_head="green"), hm.Stop("uat", deployments=[(300, "d1")]),
                 hm.Stop("prod")]
        contains = lambda commit, sha: (commit, sha) in {("green", "s1"), ("green", "s2"), ("d1", "s1")}
        h = make_history([hm.Issue(1, "a"), hm.Issue(2, "b")], {1: (100, "s1"), 2: (200, "s2")}, stops, contains)
        p = hm.frame_at(h, 250)["villages"][0]["pipeline"]
        self.assertEqual([(s["name"], s["source"], s["issues"]) for s in p["stops"]],
                         [("master-green", "branch", [2, 1]), ("uat", "unknown", []), ("prod", "unknown", [])])
        p = hm.frame_at(h, 350)["villages"][0]["pipeline"]
        self.assertEqual([(s["source"], s.get("sha"), s["issues"]) for s in p["stops"]],
                         [("branch", None, [2]), ("deployment", "d1", [1]), ("unknown", None, [])])
        self.assertEqual(p["sentinel"], "green")



def pr(number, title="", body="", merged="2026-11-08T15:00:00Z", oid="m1", commits=("2026-11-08T14:00:00Z",),
       closing=(), base="master", created="2026-11-08T14:30:00Z"):
    url = "https://github.com/o/r"
    return {"number": number, "title": title, "body": body, "url": f"{url}/pull/{number}", "createdAt": created,
            "mergedAt": merged, "mergeCommit": {"oid": oid} if oid else None, "baseRefName": base,
            "closingIssuesReferences": [{"number": n, "url": f"{url}/issues/{n}"} for n in closing],
            "commits": [{"authoredDate": c} for c in commits]}


class PullRequests(unittest.TestCase):
    def test_closing_reference_wins_then_body_then_title(self):
        self.assertEqual(hm.pull_issues(pr(10, "Fix #3", "Closes #4", closing=[5])), [5])
        self.assertEqual(hm.pull_issues(pr(10, "Fix #3", "Fixes #4, resolves: #6")), [4, 6])
        self.assertEqual(hm.pull_issues(pr(10, "feat: thing (#3)", "see #9")), [3])
        self.assertEqual(hm.pull_issues(pr(10, "feat: thing", "mentions #9 only")), [])

    def test_other_repos_closing_references_are_ignored(self):
        p = pr(10, "x", closing=[5])
        p["closingIssuesReferences"].append({"number": 7, "url": "https://github.com/other/r/issues/7"})
        self.assertEqual(hm.pull_issues(p), [5])

    def test_landing_is_the_merge_commit_at_merge_time_claimed_from_the_first_commit(self):
        [l] = hm.landings_from_pulls([pr(10, "Fix #3", commits=("2026-11-08T14:10:00Z", "2026-11-08T14:00:00Z"))],
                                     "master")
        self.assertEqual((l["issues"], l["sha"]), ([3], "m1"))
        self.assertEqual(l["time"], hm.parse_time("2026-11-08T15:00Z"))
        self.assertEqual(l["start"], hm.parse_time("2026-11-08T14:00Z"))

    def test_squash_merge_and_no_commits_fall_back_to_created(self):
        [l] = hm.landings_from_pulls([pr(10, "Squashed (#3)", oid="sq", commits=())], "master")
        self.assertEqual((l["sha"], l["start"]), ("sq", hm.parse_time("2026-11-08T14:30Z")))

    def test_unmerged_and_other_base_pulls_are_not_landings(self):
        self.assertEqual(hm.landings_from_pulls([pr(1, merged=None), pr(2, base="dev"), pr(3, oid=None)], "master"),
                         [])

    def test_issue_is_landed_and_claimed_by_its_pull(self):
        i = hm.Issue(3, "thing", created=hm.parse_time("2026-11-08T13:00Z"))
        issues = [i]
        landed = hm.apply_pulls(issues, {}, hm.landings_from_pulls([pr(10, "Fix #3")], "master"))
        self.assertEqual(landed, {3: (hm.parse_time("2026-11-08T15:00Z"), "m1")})
        self.assertEqual(i.claims(), [(hm.parse_time("2026-11-08T14:00Z"), hm.parse_time("2026-11-08T15:00Z"))])

    def test_pull_claim_counts_as_held_for_an_issue_with_a_comment_record(self):
        i = hm.Issue(3, "thing", created=0)
        i.starts = []  # read from GitHub: no work-started comment, so labels alone wouldn't count
        hm.apply_pulls([i], {}, hm.landings_from_pulls([pr(10, "Fix #3")], "master"))
        self.assertEqual(len(i.claims()), 1, "the PR's commits are the evidence a worker held it")

    def test_own_working_claims_are_kept(self):
        i = hm.Issue(3, "thing", created=0, labels=[(100, "add", "working"), (200, "remove", "working")])
        hm.apply_pulls([i], {}, hm.landings_from_pulls([pr(10, "Fix #3")], "master"))
        self.assertEqual(i.claims(), [(100, 200)])

    def test_git_landing_wins(self):
        landed = hm.apply_pulls([], {3: (5.0, "g")}, hm.landings_from_pulls([pr(10, "Fix #3")], "master"))
        self.assertEqual(landed[3], (5.0, "g"))

    def test_pull_with_no_issue_is_its_own_task(self):
        issues = []
        landed = hm.apply_pulls(issues, {}, hm.landings_from_pulls([pr(10, "Tidy the docs")], "master"))
        [i] = issues
        self.assertEqual((i.number, i.title, i.url), (10, "Tidy the docs", "https://github.com/o/r/pull/10"))
        self.assertEqual(landed[10][1], "m1")
        self.assertEqual(len(i.claims()), 1)
        h = hm.History("p", "/r", "master", issues, landed, [])
        self.assertIn(10, hm.frame_at(h, hm.parse_time("2026-11-08T15:01Z"))
                      ["villages"][0]["pipeline"]["quay"])

    def test_pulls_outside_the_window_are_skipped(self):
        issues = []
        landed = hm.apply_pulls(issues, {}, hm.landings_from_pulls([pr(10, "x")], "master"),
                                since=hm.parse_time("2026-11-09T00:00Z"))
        self.assertEqual((issues, landed), ([], {}))


class Parsing(unittest.TestCase):
    def test_times(self):
        self.assertEqual(hm.parse_time("2026-11-08T14:50Z"), hm.parse_time("2026-11-08T14:50:00+00:00"))

    def test_body_links_to_a_parent(self):
        kid = hm.Issue(21, "x", body="Dependencies:\n- Sub-task of #20\n", created=5)
        other = hm.Issue(22, "y", body="Sub-task of #200", created=5)
        hm.link_features(".", [hm.Issue(20, "p"), kid, other], sub_issues={})
        self.assertEqual((kid.parent, kid.linked_by, kid.linked_at), (20, "body", 5))
        self.assertIsNone(other.parent, "#200 is not in the history")


if __name__ == "__main__":
    unittest.main()
