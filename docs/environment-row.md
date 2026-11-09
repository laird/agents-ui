# The environment row: one village per deploy environment

Design note for #376 (phase 1, #383). Phases 2 and 3 (#384 data, #385
rendering, #386 demo) build on it. Rendering also waits for the
`village.html` module split (#387).

## Today

The map is one dev village. Each environment in `.autocoder.json` is a lock
on its canal, and the last one is the harbour. Every lock holds the issues
that reached it but not the next one (`place_tips` in `src/web/village.rs`).
A ship in the harbour carries the issues waiting for the next release (#362)
and sails when the last stop's sha changes. That shows *where* each issue
is. It does not show *what each environment runs*, what is in a deployment,
or a deployment that is still running or has failed.

## Goal

```
[ dev village ] ──ship──▶ [ nuc-1 ] ──ship──▶ [ release ]
  docks, houses,            what nuc-1          what the latest
  gate, quay, harbour       runs                Release contains
```

A row of villages, left to right. The dev village comes first, then one
village per environment in `.autocoder.json` order, with production
right-most. Here that is `nuc-1` (what `tools/deploy.sh` installs and serves
on nuc-1), then `release` (the latest GitHub Release, `tools/release.sh`).
The user decided there is no staging. A ship sails between neighbouring
villages for each deployment.

## Layout

### The row

- **The dev village** is today's village: docks, houses, gatehouse (#375),
  quay, and the harbour where the first ship loads. It keeps the sentinel's
  `<integration>-green` lock, which is a gate, not an environment. The
  environment locks leave its canal, because each environment becomes a
  village of its own.
- **Each environment village** is a fixed-size scene, narrower than the dev
  village (about 120 by the dev village's height in scene pixels). It has:
  - a **sign** with the running version: its tag, else a short sha, with
    the deploy time underneath;
  - a **warehouse** holding the cargo, crates grouped under the banners of
    the features they belong to (#367's banners appear in each environment
    a feature has reached);
  - a **harbour** on its west side where ships dock and unload, and one on
    its east side where the next ship loads;
  - a **health mark**: smoke and a red flag for a failed newest deployment,
    a fire for an open incident against it (worlds), and nothing when
    nothing says.
- **Between villages**, a stretch of river 40 px wide carries the ships. It
  is long enough to see a ship under sail, and short enough that the whole
  row fits a wide screen at the "fit" zoom.
- **Production** is the right-most village, and the last environment in the
  list. No extra configuration is needed.

### Narrow widths, zoom and the mini-map

- The canvas is the whole row. The panel scrolls horizontally, and #369's
  zoom (−, Fit, +) applies to the whole row, so "Fit" shows every village.
- The view **starts on the dev village** (scrolled to the left edge) unless
  the URL names another one (`/village#shop@release`).
- A **mini-map** strip above the map draws one box per village and a dot per
  ship under sail. It is coloured by health and shows the visible part of
  the row as a frame. Clicking a box scrolls to that village. It only
  appears when the row is wider than the panel.

### Rank-based look

Each environment village uses the dev village's theme, with its size taken
from its rank in the row:

| Rank | Medieval | Modern | Future |
|---|---|---|---|
| dev (first) | hamlet: today's village | office park | outpost |
| middle environments | town: walled, a market warehouse | business district | orbital station |
| production (last) | capital: castle keep, a big warehouse | skyline | capital ship dock |

With two environments, `nuc-1` is a town and `release` is the capital. The
rank only changes the drawing, never what is shown. A theme without rank art
draws every environment as a town.

## Data model: what `/api/village` adds

Everything sits on the existing pipeline, so a village without environments
(a runtime, an outside report without stages) is unchanged.

### Per environment: on each `pipeline.stops[i]`

`stops` keeps its order and meaning. The fields below are added. A stop that
is the sentinel's green ref gets `kind: "gate"`, and every configured
environment gets `kind: "environment"`. The renderer draws a village for
each `environment` stop and keeps a `gate` stop as a lock in the dev
village.

```json
{
  "name": "release",
  "kind": "environment",
  "issues": [361, 362],
  "source": "deployment",
  "sha": "9b1e440…",
  "tag": "v0.6.0",
  "url": "https://github.com/laird/agents-ui/releases/tag/v0.6.0",
  "deployed_at": "2026-11-09T17:40:02Z",
  "deployed_by": "laird",
  "deployment_url": "https://github.com/laird/agents-ui/deployments/release",
  "cargo": [358, 359, 360, 361, 362],
  "cargo_more": 41,
  "health": { "state": "ok" }
}
```

| Field | Meaning | Source |
|---|---|---|
| `kind` | `gate` or `environment` | `read_pipeline` already knows: the green stop comes from `sentinel_gate`, and the rest from `parse_environments` |
| `issues` | (unchanged) issues that reached this stop but not the next | `place_tips` |
| `source`, `sha`, `tag`, `url` | (unchanged) what it runs and what says so | `deployments::locate` / `deployed` through the source chain (`default_chain`, `parse_sources`) |
| `deployed_at` | when the running version was deployed | the successful status's `created_at` (Deployment), the Release's `published_at`, the tag's commit date. Absent for a branch |
| `deployed_by` | who deployed it | `creator.login` on the Deployment, `author.login` on the Release. `Deployment` and `Release` in `src/web/deployments.rs` gain these fields, which the API already returns |
| `deployment_url` | the page for this environment's deployments | `{repo}/deployments/{name}`. The renderer builds it today from `repoUrl` (`stopLinks`), so the server only adds it when it knows the repo, e.g. an outside report |
| `cargo` | every issue the environment **runs**, newest first: issues whose landed tip is in this stop's history. Cumulative, unlike `issues` | `place_tips` already computes `reached` per issue (stops reached, not truncated). Cargo of stop *i* is the issues with `reached > i`, in tip order. Kept as `#[serde(skip)]` today, so this is the first time it is exposed |
| `cargo_more` | how many older issues were cut from `cargo` (capped at 40; an issue's crate stays clickable through the stop dialog's full list) | the cap, like `DELIVERED_KEEP` for `issues` |
| `health` | `{ state: "ok" \| "failed" \| "incident" \| "unknown", reason?, url? }` | `failed`: the newest deployment's latest status is `failure` or `error` (the statuses `deployed` already fetches). `incident`: a world says this environment's platform has open incidents. `unknown`: a stop whose source is `unknown`. Otherwise `ok` |

Grouping by feature is not sent. The renderer already has
`features[].subtasks` (`src/web/feature.rs`), and groups `cargo` by
whichever feature lists each issue. The rest go under a "fixes" banner.

### Voyages: `pipeline.voyages`

A voyage is one deployment to one environment, newest first, at most one
in flight and one finished per environment:

```json
{
  "from": "nuc-1",
  "to": "release",
  "state": "sailing",
  "sha": "c0ffee1…",
  "tag": "v0.7.0",
  "started_at": "2026-11-09T18:02:11Z",
  "finished_at": null,
  "url": "https://github.com/laird/agents-ui/deployments/release",
  "issues": [363, 368, 375]
}
```

| Field | Meaning | Source |
|---|---|---|
| `from` | the village the ship leaves: the environment's `requires`, or `dev` for `integration`/`green` | `Environment.requires` from `parse_environments` |
| `to` | the environment | the deployment's `environment` |
| `state` | `loading`, `sailing`, `docked` or `failed` (below) | the deployment's latest status, from `GET …/deployments/{id}/statuses`, already fetched by `deployments::deployed` for the newest `DEPLOYMENTS_CHECKED` (10) deployments |
| `sha`, `tag` | what is being deployed | `Deployment.sha`, and `release_tag(description)` |
| `started_at`, `finished_at` | for durations and for retiring old voyages | `Deployment.created_at`, and the finishing status's `created_at` |
| `url` | the status's `log_url` (e.g. the GitHub Release), else the environment's deployments page | `DeploymentStatus.log_url` |
| `issues` | the cargo **new to the target**: issues whose landed tip is in `sha`'s history but not in the target's previous successful sha | two `history()` calls (`rev-list`) in `read_pipeline`, the same way it builds stop histories, then tips filtered as `place_tips` does. For a promotion, this is exactly what leaves the source village's warehouse for the target's |

States, from the deployment's latest status:

| Deployment status | Voyage state | On the map |
|---|---|---|
| `queued`, `pending` | `loading` | the ship at the source's east harbour, crew carrying the cargo aboard |
| `in_progress` | `sailing` | the ship under sail on the river between the two villages |
| `success` | `docked` | the ship at the target's west harbour, crew unloading into the warehouse. The target's sign shows the new version. Dropped from the list an hour after `finished_at` |
| `failure`, `error` | `failed` | the ship turned back, sail torn and flag red, docked at the source with the cargo still aboard. The target keeps its old version, and its health goes `failed`. Kept until a newer deployment to the same environment |
| `inactive` | (none) | a superseded deployment. It is history and never becomes a voyage |

Only Deployments have in-progress statuses. An environment whose version
comes from a Release, tag, version URL or branch has no `loading` or
`sailing`, and the server reports no voyage for it. A server with no memory
between polls can't tell when such a version changed. The page does that
instead, as #362's ship does today: when the stop's `sha` (or `tag`) changes
between two polls, it plays a voyage from the source village with the issues
the target's `cargo` gained. The page also plays a reported voyage only once,
keyed by (`to`, `sha`, `state`), so a reload doesn't replay a finished one.

Cost: `deployments::deployment_state` reads each environment's deployments
once, for both what runs and the voyage. It fetches statuses newest first
until it finds two successful deployments (what runs, and what ran before
it), at most `DEPLOYMENTS_CHECKED` (10), so a successful voyage costs at most
one more status call than before. Its cargo needs `rev-list` on the
deployed and previous shas (local, bounded by `HISTORY_DEPTH` = 400
commits). `issues_more` is true when the deployed commit isn't fetched yet,
or its history reaches that depth, so the list may be short.

## Worlds

A world (`--world shop=temporal:agents`) stacks a platform village above the
dev village today. In the row:

- **The platform is the production village with runtime detail.** It is
  drawn at the right-most slot, and its houses are the live workloads
  (Temporal workstreams or Kubernetes pods), drawn in the other style as
  now. Its warehouse still shows the cargo, so the village shows both what
  was released and what runs. A platform can attach to another environment
  with `--world shop=temporal:agents@nuc-1`. Without `@env` it attaches to
  the last.
- **Releases going up**: today's cart on the road becomes the voyage to
  that environment. "Landed" (a platform worker reports a build ID matching
  the release, `world::same_build`) is the voyage's `docked` for a runtime.
  Until then a docked ship waits at the quay unloaded, which is today's
  "shipping".
- **Incidents still come back to the dev village's docks.** A crate floats
  down the river from the burning building, right to left along the row,
  past the villages in between, and lands at the dev docks as an issue.
  While the platform has open incidents, the environment it runs has health
  `incident` ("2 open incidents"). That is set when the worlds are built
  (`world::mark_incidents`, from `Villages::snapshot` in `src/web/source.rs`).
- `WorldSnapshot` keeps its fields. It gains `environment` (the stop it
  attaches to), and the renderer places the platform scene there instead of
  above.

## The neutral report (`docs/village-protocol.md`)

Outside adapters describe the same row through `pipeline.stages`, so
nothing new is required of them. A report that only gives `{name, tasks}`
stages still gets one village per stage, with an unknown version and no
voyages. The protocol gains, all optional:

- on each **stage**: `kind` (`gate` or `environment`, default
  `environment`), `deployed_at`, `deployed_by`, `deployment_url`, `cargo`
  (task ids the stage runs; when absent, the village derives it from the
  stages' `tasks` order, cumulatively), and `health` (`{state, reason, url}`);
- on the **pipeline**: `voyages`, each `{from, to, state, sha, tag,
  started_at, finished_at, url, tasks}`, with `from` = a stage name or
  `"dev"`.

`framework.rs` maps them one to one (`Stage` ↔ `Stop`, as `sha`, `tag` and
`url` are mapped today). `swarm_report` fills them for autocoder villages.

## What each phase delivers

- **#384, data:**
  - `kind`, `deployed_at`, `deployed_by`, `cargo`/`cargo_more` and `health`
    on stops, and `pipeline.voyages`, in `village.rs`, `deployments.rs`
    (with `creator`), `framework.rs` and the protocol doc;
  - `environment` on worlds, in `world.rs`;
  - JSON fixtures for every voyage state, including an inactive deployment,
    a promotion's cargo, a Release-sourced environment, and the depth cap.
- **#385, rendering**, in the post-#387 modules:
  - the row, the environment villages with their signs, warehouses and
    health;
  - ships loading, sailing, docking and failing;
  - the mini-map, the rank art, and worlds in their slot;
  - page tests.
- **#386, demo:** the `gh` shim and `swarm_sim.py` script a deploy to
  `nuc-1` (pending → in_progress → success), a promotion `nuc-1` →
  `release`, and a failed deployment.

## Open questions for later

- Health from a smoke check (`url:` sources could report a status as well as
  a version) is left out until an environment has one.
- An environment that runs more than one version at once (a canary) is drawn
  with its newest successful version. A second ship docked at the warehouse
  for the canary could follow.
