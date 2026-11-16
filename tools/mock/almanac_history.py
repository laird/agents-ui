"""A recorded fortnight of GitHub history for the demo's almanac (#499).

The swarm simulator's issues are open ones with no dates, so on their own
they give the village's `github` summary (src/web/github.rs, #498) fourteen
empty days. The gh shim adds these closed issues to `gh issue list --state
all`: each day's landings, a claim comment before each (the median claimed to
landed), and the gate's conflict comments for its bounces, dated relative to
now, so the almanac has two weeks to draw. Tests: test_almanac_history.py."""
from datetime import datetime, timedelta

from conflicts import conflict_comment

# Landed and bounced per day, oldest first; the last is today.
LANDED = [3, 5, 2, 0, 4, 6, 7, 3, 5, 8, 4, 6, 9, 4]
BOUNCED = [0, 1, 0, 0, 2, 0, 1, 3, 0, 1, 0, 2, 1, 1]
FIRST = 9000


def iso(t):
    return t.isoformat(timespec="seconds")


def history(now=None):
    """The closed issues, as `gh issue list --json number,state,createdAt,closedAt,stateReason,comments` gives them."""
    now = now or datetime.now().astimezone()
    out, n = [], FIRST
    days = len(LANDED)
    for k, (landed, bounced) in enumerate(zip(LANDED, BOUNCED)):
        back = days - 1 - k
        midnight = (now - timedelta(days=back)).replace(hour=0, minute=0, second=0, microsecond=0)
        if back:
            # A working day: 09:00 to 18:00.
            start, end = midnight + timedelta(hours=9), midnight + timedelta(hours=18)
        else:
            # Today: up to a few minutes ago, never before midnight.
            end = max(now - timedelta(minutes=5), midnight + (now - midnight) / 2)
            start = max(midnight, end - timedelta(hours=9))
        for j in range(landed):
            closed = start + (end - start) * (j + 1) / (landed + 1)
            claimed = closed - timedelta(minutes=50 + 35 * (j % 4))
            comments = [{"body": "🔒 claim sim-worker", "createdAt": iso(claimed)}]
            if j < bounced:
                comments.append({"body": conflict_comment(n), "createdAt": iso(claimed + (closed - claimed) / 2)})
            out.append({"number": n, "title": f"Recorded work {n}", "state": "CLOSED", "stateReason": "COMPLETED",
                        "createdAt": iso(claimed - timedelta(hours=6)), "closedAt": iso(closed), "comments": comments,
                        "labels": []})
            n += 1
    return out
