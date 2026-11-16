"""Tests for almanac_history.py: python3 -m unittest discover -s tools/mock -p 'test_almanac_history.py'"""
import os
import sys
import unittest
from datetime import datetime, timedelta, timezone

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import almanac_history as ah  # noqa: E402


class HistoryTest(unittest.TestCase):
    def test_each_day_lands_what_it_says_and_nothing_after_now(self):
        for now in [datetime(2026, 11, 16, h, m, tzinfo=timezone(timedelta(hours=-5))) for h, m in [(11, 0), (0, 40), (0, 2), (23, 59)]]:
            with self.subTest(now=now):
                self.check_day(now)

    def check_day(self, now):
        issues = ah.history(now)
        self.assertEqual(len(issues), sum(ah.LANDED))
        per_day = {}
        for i in issues:
            closed = datetime.fromisoformat(i["closedAt"])
            self.assertLessEqual(closed, now)
            self.assertLess(datetime.fromisoformat(i["comments"][0]["createdAt"]), closed, "claimed before it landed")
            per_day[closed.date()] = per_day.get(closed.date(), 0) + 1
        for k, landed in enumerate(ah.LANDED):
            self.assertEqual(per_day.get((now - timedelta(days=13 - k)).date(), 0), landed)

    def test_bounces_are_the_gates_conflict_comments(self):
        issues = ah.history(datetime(2026, 11, 16, 23, 0, tzinfo=timezone.utc))
        bounces = sum(1 for i in issues for c in i["comments"] if "hit a conflict" in c["body"])
        self.assertEqual(bounces, sum(min(b, l) for b, l in zip(ah.BOUNCED, ah.LANDED)))


if __name__ == "__main__":
    unittest.main()
