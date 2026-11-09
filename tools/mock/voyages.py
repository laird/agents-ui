"""Scripted GitHub Deployments for the environment row (#376, #386).

`VoyageScript` plays this repo's own pipeline, `nuc-1` then `release`, as
GitHub Deployments over a loop, so the village shows every voyage state
without real deployments:

  * a deploy to `nuc-1` of the integration tip: pending, then in_progress,
    then success (loading, sailing, docked);
  * a promotion of what `nuc-1` runs to `release`, as `tools/release.sh`
    records one ("Release v0.N.0", linking its GitHub Release);
  * a deploy to `nuc-1` that fails (a failed voyage; `nuc-1` keeps running
    what it ran).

A deployment that succeeds marks the one it replaces `inactive`, as GitHub
does. `state()` is what the mock `gh` serves for
`repos/{owner}/{repo}/deployments?environment=…` and its statuses, in the
shape `tools/mock/bin/gh` reads from `<demo>/state/deployments/<name>.json`.

The clock and the shas are passed in, so the timeline is testable without git:
`tip()` names the commit a deploy to `nuc-1` ships, and `released(sha)` is
told when a promotion to `release` succeeds.
"""

import datetime

PERIOD = 120
# (seconds into the loop, environment, how it ends)
STEPS = [(0, "nuc-1", "success"), (40, "release", "success"), (80, "nuc-1", "failure")]
# Status times after a deployment is created.
IN_PROGRESS, FINISHED = 8, 18
REPO_URL = "https://github.com/sim/shop"


def iso(t):
    return datetime.datetime.fromtimestamp(t, datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


class VoyageScript:
    def __init__(self, tip, t0, released=None, creator="deploy-bot"):
        self.tip, self.t0, self.released, self.creator = tip, t0, released or (lambda sha: None), creator
        self.deploys = {"nuc-1": [], "release": []}  # newest first
        self.next_id, self.next_tag = 1, 1
        self.started = set()  # (cycle, step) already started

    def current(self, env):
        """The sha the environment runs: its newest successful deployment."""
        for d in self.deploys[env]:
            if d["statuses"][0]["state"] == "success":
                return d["sha"]
        return None

    def start(self, env, sha, t, outcome):
        d = {"id": self.next_id, "sha": sha, "environment": env, "created_at": iso(t),
             "creator": {"login": self.creator}, "statuses": [{"state": "pending", "created_at": iso(t)}],
             "_t0": t, "_outcome": outcome}
        self.next_id += 1
        if env == "release":
            tag = f"v0.{self.next_tag}.0"
            self.next_tag += 1
            d["description"] = f"Release {tag}"
            d["_url"] = f"{REPO_URL}/releases/tag/{tag}"
        self.deploys[env].insert(0, d)

    def advance(self, d, t):
        """Move one deployment's statuses along to where they are at `t`."""
        state = d["statuses"][0]["state"]
        if state == "pending" and t >= d["_t0"] + IN_PROGRESS:
            d["statuses"].insert(0, {"state": "in_progress", "created_at": iso(d["_t0"] + IN_PROGRESS)})
            state = "in_progress"
        if state == "in_progress" and t >= d["_t0"] + FINISHED:
            at = iso(d["_t0"] + FINISHED)
            status = {"state": d["_outcome"], "created_at": at}
            if d.get("_url"):
                status["log_url"] = d["_url"]
            if d["_outcome"] == "success":
                # The deployment it replaces is no longer what runs.
                for old in self.deploys[d["environment"]]:
                    if old is not d and old["statuses"][0]["state"] == "success":
                        old["statuses"].insert(0, {"state": "inactive", "created_at": at})
                if d["environment"] == "release":
                    self.released(d["sha"])
            d["statuses"].insert(0, status)

    def tick(self, t):
        cycle, at = divmod(t - self.t0, PERIOD)
        for k, (when, env, outcome) in enumerate(STEPS):
            if at < when or (cycle, k) in self.started:
                continue
            # A promotion ships what nuc-1 runs; with nothing there yet it waits a loop.
            sha = self.tip() if env == "nuc-1" else self.current("nuc-1")
            self.started.add((cycle, k))
            if sha:
                self.start(env, sha, self.t0 + cycle * PERIOD + when, outcome)
        for ds in self.deploys.values():
            for d in ds:
                self.advance(d, t)

    def promote_now(self, t):
        """Promote what nuc-1 runs to release at once (the demo's release-now)."""
        sha = self.current("nuc-1")
        if sha:
            self.start("release", sha, t, "success")
        return sha

    def state(self):
        """What the mock gh serves: each environment's deployments, newest first."""
        return {env: [{k: v for k, v in d.items() if not k.startswith("_")} for d in ds[:10]] for env, ds in self.deploys.items()}


def config():
    """`.autocoder.json` for "shop" in this mode: this repo's own environments."""
    return {"environments": [{"name": "nuc-1", "requires": "integration", "source": "deployment"},
                             {"name": "release", "requires": "nuc-1", "source": "deployment"}]}
