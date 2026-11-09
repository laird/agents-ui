# Village view

`/village` draws each swarm as a 16-bit village. Run agents-tui with `--web`
(alongside the TUI) or `--headless` (as a service) and open
http://127.0.0.1:7878/village.

Pick a style in the title bar: **Medieval village** or **Modern business
district**. The choice is remembered per browser; `?theme=modern` or
`?theme=medieval` in a link overrides it. Styles change the art, never the
meaning: lit windows mean working, an open door, an orange `!` and a villager
waving at you (arm up, hand swinging) mean an
agent is waiting on you, dust means stalled, fog means unknown.

The server listens on 127.0.0.1 only. To watch a swarm on another machine,
tunnel to it: `ssh -L 7878:localhost:7878 swarm-host`.

## Try it without agents

Watch a recording first. `python3 tools/mock/replay-build.py <recording> replay.html`
builds a self-contained page that replays it through the real renderer:

- `tools/mock/recordings/demo.json.gz`: six minutes of the demo, with Temporal or Kubernetes as the runtime.
- `tools/mock/recordings/story.json.gz`: one bug from start to finish, with captions. A crash
  loop in the prod cluster files an incident; a worker in the dev village fixes it (stopping
  once to ask permission); the fix lands, goes green, then staging, then prod; the cluster rolls
  out the new image and the crash stops. Record a new one with `STORY=1 RUNTIME=k8s tools/mock/demo.sh`
  and `tools/mock/record.py`.

```bash
tools/mock/demo.sh          # then open http://127.0.0.1:7878/village
```

runs agents-tui against simulated swarms: an autocoder swarm and a modernize
swarm under herdr (with real git worktrees, issues, tip refs and
environments) and a Temporal namespace of agentic workstreams, linked as a
world (`#world:shop`): failed workstreams file incident issues in the dev
swarm, and its prod releases roll out to the platform's workers. With
`RUNTIME=k8s` the platform is a simulated Kubernetes namespace instead
(`tools/mock/k8s_sim.py` behind a `kubectl` shim): releases roll out pod by
pod as new image tags, pods crash-loop and file incidents, a scale-up goes
unschedulable, a node goes NotReady. See the header of `tools/mock/demo.sh`;
`tools/mock/record.py` records running demos for the replay page.

## Drilling in

The village is for watching; when something needs a response, select it and
jump to the tool that handles it, in a new tab:

| Select | Opens |
|---|---|
| A house or the castle | the agent's live session in the dashboard; its issue on GitHub (or the tracker its adapter names) |
| A Temporal house | the workflow in Temporal's web UI (`--temporal-ui`) |
| The notice board | the blocked issues and proposals, each linked |
| The docks | the backlog, highest priority first, each linked |
| A feature's banner, then Details | the feature's spec (the document its issue links to, else the issue) and its issue; each subtask is linked too |

Outside adapters add their own links per house (`links` in the report,
see [village-protocol.md](village-protocol.md)).

### Quick answers

The one thing you can do without leaving the village: when a waiting agent
asked a structured question, its dialog shows the question with one button
per option.

- A numbered menu (Claude Code's "Do you want to proceed? ❯ 1. Yes / 2. … /
  3. No") becomes buttons 1, 2, 3; a `(y/n)` prompt becomes Yes and No.
- On a click, agents-tui reads the pane again and sends the keys only if the
  same question is still on screen: arrow keys to the option and Enter for a
  menu, `y` or `n` and Enter for a prompt. A changed question sends nothing.
- Anything open-ended is answered in the agent's session (its link).
- Not available with `--read-only`, or for herdr and Temporal agents, which
  agents-tui can't type into yet.

## What people are doing

Every agent's current action is acted out:

| Action | Prop | From Claude Code | From a Temporal activity named like |
|---|---|---|---|
| Thinking | thought cloud | model's turn, or reading a tool result | `call_llm`, `plan`, `summarize`, `classify` |
| Reading | open book | `Read`, `Grep`, `Glob` | `read`, `load`, `get`, `extract` |
| Writing | swinging hammer | `Edit`, `Write` | `write`, `commit`, `update`, `draft` |
| Running | terminal | `Bash` (and any other tool) | `run`, `test`, `build`, or anything else |
| Searching | magnifying glass, walking | `WebFetch`, `WebSearch`, MCP tools | `search`, `fetch`, `http`, `query` |
| Messaging | letter, walking | `Task`, `Agent`, `SendMessage` | `send`, `email`, `notify`, `slack` |

People out front act out their own action; a busy house shows its own
agent's action on a small sign over the door. Claude Code actions come from
the last step in each transcript; Temporal actions from each workflow's
first pending activity.

## Coding swarms (autocoder, modernize)

Found automatically from tmux and herdr. Under herdr (`herdr agent list`) an
agent in a `<repo>-wt-<N>` worktree is worker N and the agent in `<repo>` is
the manager, as in autocoder's `/monitor-workers`; a worker's issue comes
from its branch (`feature/issue-412`). The manager lives in the castle (medieval) or headquarters (modern); every worker
session is a house, and agents sharing a worktree are one house.

| What you see | Where it comes from |
|---|---|
| House shape | Labels on the worker's issue (`bug`, `enhancement`, `test-failure`, `security`, ...) |
| Flag on a house | `P0`–`P3` label |
| People out front | Running Claude Code sub-agents (`~/.claude/projects/<worktree>/<session>/subagents/`) |
| Crates at the docks | Open, unclaimed, unblocked issues, highest priority first |
| A roped crate | An issue waiting on another one: its body says "Depends on #N" or "Blocked by #N" and #N is still open. autocoder won't pick it up yet; its dialog says "waits on #N" |
| Notes on the board | Issues with a blocking label (`needs-design`, `proposal`, ...) |
| Crates at the quay | `refs/autocoder/tips/<integration>/<sha>-<issue>` not yet green |
| Locks, then the harbour or launch pad | `<integration>-green`, then each environment in `.autocoder.json` (see [Environments](#environments)) |
| Crates on the ship (a rocket downtown) | Issues at the stop before the last, waiting for the next release (see [The ship](#the-ship)) |
| Banners east of the houses | Features: parent issues split into subtasks (see [Following a feature](#following-a-feature)) |

### Environments

The canal's first lock is the sentinel's `<integration>-green` ref. After it
comes one lock per environment listed in `.autocoder.json`, in order; the
last is the harbour (medieval) or launch pad (modern).

```json
{
  "environments": [
    { "name": "dev" },
    { "name": "uat", "ref": "release/uat" },
    { "name": "prod" }
  ],
  "promotion": "subtask"
}
```

For each environment the village works out what it runs from a chain of
sources, tried in order. The first that names a commit wins:

| Source | What it reads |
|---|---|
| `deployment` | The newest GitHub Deployment to an environment with that `name` whose latest status is `success`, checking up to 10. A deployment later marked `inactive`, failed or still pending doesn't count. `tools/deploy.sh`, Actions `environment:` jobs, Vercel, Netlify and Heroku all record these. |
| `release` | The latest published GitHub Release (drafts and prereleases are skipped), at its tag's commit. A Release made on GitHub's website counts. |
| `tag:<glob>` | The newest tag matching the glob (`v*`, `prod-*`), in version order, so `v1.10.0` beats `v1.9.2`. |
| `branch` | `origin/<ref>`, where `ref` defaults to `env/<name>`. |
| `url:<endpoint>` | An `http(s)` endpoint whose response names the running commit: a bare sha, or JSON with a `sha`, `commit`, `git_sha`, `gitSha`, `revision` or `version` field (top level or one object down). This is the most truthful answer to "what is actually running". |

Set an environment's `source` to one of these or an ordered list:

```json
{
  "environments": [
    { "name": "staging", "source": "branch" },
    { "name": "prod", "source": ["deployment", "tag:v*", "url:https://app.example.com/version"] }
  ]
}
```

Without `source`, the chain is `deployment`, then `release` (for the last
environment only, so a repo that publishes Releases gets a real harbour with no
config), then `branch`. Entries the village can't read are dropped. All the
GitHub sources need `gh` signed in. A source that can't answer (no `gh`, no
Releases, an unreachable endpoint) falls through to the next one, never to an
error. A commit the clone hasn't fetched yet falls through too, since it can't
place anything.

With no source answering, the environment is **unknown**: the village can't
tell what it runs. It fogs the lock with a "?" rather than call it empty.

Work only reaches a lock after reaching every lock before it, so crates never
pass an unknown environment. A feature's Details dialog says where each
answer came from: `GitHub Deployment · 9b1e440`, `v1.2.0 (GitHub Release) ·
a1b2c3d`, `tag v1.10.0`, `its version endpoint`, `its branch ref`, or
`nothing says what runs here`.

`promotion` says how your project moves landed work through the environments.
It is `subtask` (the default) or `feature`; see below.

### An issue's life

Changes seen between two polls play out as trips across the map:

- **Arrives.** A supply boat (a truck downtown, a cargo shuttle in Future)
  comes down the river to the docks, and its crew unloads one crate per new
  issue into its slot.
- **Claimed.** The worker walks from its house to the docks, picks up that
  crate and carries it home.
- **Worked on.** The crate sits at the door, and the worker hammers at it,
  with sparks unless the action is reading, thinking, searching or messaging.
- **Landed.** The worker carries the sealed crate to the quay and walks home.
- **Board.** A blocked issue is carried to the notice board, and carried back
  to the docks when it is unblocked.
- **Closed without landing.** The crate is carried off the map's west edge.

Trips play in order, one starting 0.7 s after the last, and at most 12 wait
at once; past that, crates simply appear where they now are. Nothing replays
when the page loads, and a tab left in the background does not catch up on
what it missed. With reduced motion no one walks: a notice says what changed.
Click a crate in transit for the same dialog as at rest, plus where it is
going. `LIFECYCLE=1 tools/mock/demo.sh` plays every transition on a 90 s loop.

### The ship

A ship is moored in the harbour (a rocket stands on the launch pad). Work
waiting for the next release is loaded onto it: the issues at the stop just
before the last one, or at the quay when there is only one stop. Porters
carry each one along the canal as it arrives; past six crates a badge counts
the rest ("+7"). Intermediate locks open their gates as crates float through
and keep their own crates docked.

When the last stop's deployment changes (its `sha`, so a release of the same
issues still counts), the ship sails off east with what shipped, the
fireworks go up, and an empty ship comes in; the rocket lifts off and a new
one rolls out. This plays once per release, and a page opened afterwards just
shows the new ship. With reduced motion the ship is swapped without a voyage
and a "shipped vX.Y.Z" notice shows. In a world, the release cart sets off up
the road as the ship leaves the map.

Click the ship for its cargo and "next release: N issues since vX.Y.Z"; click
the harbour (or a departing ship's wake) for the last release and its GitHub
Release. The tag and link come from the deployment `tools/release.sh` records
(description `Release vX.Y.Z`, status `log_url`). In the demo,
`touch "$AGENTS_UI_DEMO_DIR/state/release-now"` cuts a release at once.

## Following a feature

A **feature** is a parent issue split into subtasks. The village answers "is
feature X live in production yet, and if not, where is it?" without asking
anyone.

**Which issues are features.** Every open issue that has GitHub sub-issues is
a feature, and so is every open issue labelled `decomposed`. A parent with
sub-issues uses them as its subtasks. A `decomposed` parent's subtasks are the
issues whose body says `Sub-task of #N`: autocoder writes that line and the
label when it splits an issue, but it doesn't create sub-issues. Outside
adapters get features by setting `parent` on their tasks
([village-protocol.md](village-protocol.md)).

**Banners.** Each feature has a banner in the strip cleared east of the
houses, in its own colour. Its spec lies at its foot: a scroll in the
village, a clipboard downtown. The spec is the first document in the same
repository that the parent's body links to (a `…/blob/…/*.md` URL, or a
relative `docs/…` link). Without one, the parent issue's body is the spec. The
label under each banner is a button that names the feature and how far it has
got:

| Label | Means |
|---|---|
| `Build 1/4` | 1 of the 4 counted subtasks has landed (at the quay or any lock) |
| `UAT 2/4` | all have landed; 2 have reached UAT, the first stage not everyone has reached |
| `UAT: unknown` | all have reached the stage before, and nothing says what UAT runs |
| `Stuck · #463 needs design` | a counted subtask is blocked (see below) |
| `Live in prod` | every counted subtask has reached the last stop |

**Following.** Select a banner or its label (Tab to it, then Enter or Space).
A marching trail runs from the banner to each counted subtask, wherever it
is: a crate at the docks, a note on the board, a house, the quay, a lock, the
harbour, or a barge. A strip appears above the counters:

`Spec ✓ › Build 4/4 › Master-green 4/4 › Dev 4/4 › UAT 2/4 › Prod 0/4`

The strip has one chip per pipeline stop, starting with the green ref. A chip
is green when every counted subtask has reached that stop, gold when some
have, and plain when none have. **Details** opens the feature's dialog, and
**Stop following** clears the trail. Selecting the banner again also stops.

**Counting.** A feature's total is its open subtasks plus those closed as
completed. A subtask closed as **not planned** leaves the total, so 4 becomes
3. The dialog lists it struck through, marked "closed as not planned · not
counted". A subtask closed as completed counts as landed only once the
village finds its landed commit.

**Promotion.** Set by `promotion` in `.autocoder.json`:

- `subtask` (the default): each subtask's crate moves through the locks on
  its own, so a feature can read `UAT 2/4`.
- `feature`: landed subtasks wait at the quay and move together. When every
  counted subtask is at the same stop, the village draws them as one barge (a
  flatbed truck downtown) flying the feature's colour, instead of loose
  crates.

**Stuck.** A counted subtask is stuck when it has a blocking label (it is a
note on the board) or when its worker is waiting on a person. The banner flies
a red pennant, and the strip's Build chip turns red: `Build 3/4: #463 needs
design`. A waiting worker reads `#463 waiting`, and the trail leads to the
house where the worker waves at you.

**Can't tell.** When nothing says what an environment runs (see
[Environments](#environments)), its lock is in fog with a "?" and its chip
reads `UAT: unknown` with a dashed border. The village never says "not live"
when it can't tell. A feature whose last stop is unknown is never shown as
live.

**Live.** A feature is live when every counted subtask's landed commit is in
what the last stop runs. The banner turns gold and sparkles, and the strip
adds `★ Live in prod`. This means the code runs in production. Feature flags
are out of scope, so it doesn't mean users can see it. Two timers clear live
banners away. The server keeps listing a live feature for a day after its
parent issue closes. Independently, an open page fades a banner over the
first hour after that page saw it go live, and hides it after two hours
unless you're following it.

**Details.** The dialog lists each subtask and where it is: being built by a
worker, waiting at the docks, on the board, landed and waiting at the quay,
in a stage, or closed. It also says where the feature's subtasks came from
(GitHub sub-issues, or `"Sub-task of #N"` in each body), where each
environment's answer came from, and the promotion mode. Its buttons open the
spec and the parent issue. Escape closes it.

Villages without features look exactly as they did.

## Kubernetes

```bash
agents-tui --headless --runtime k8s:prod/shop-prod      # context prod, namespace shop-prod
agents-tui --headless --runtime k8s:shop-prod           # the current context
```

A runtime village can be drawn from Kubernetes instead of Temporal: one
source per village (`--runtime temporal:<namespace>` is the same as
`--temporal`). agents-tui reads it with `kubectl ... -o json` (read-only: get
and list on pods, workloads, jobs and nodes is enough).

| What you see | Where it comes from |
|---|---|
| House | A workload: Deployment, StatefulSet, DaemonSet, CronJob, or a Job of its own; CronJobs and Jobs are barns, other names shape the house as issue labels do (`security-scanner` is a watchtower) |
| Scaffolding | A rollout in progress: fewer updated replicas than wanted |
| People out front | Its pods; a pod that needs you waves |
| Waving, orange `!` | CrashLoopBackOff, ImagePullBackOff, unschedulable, a rollout past its deadline; the pod's line says why ("CrashLoopBackOff · 7 restarts, last OOMKilled") |
| The hall's people | The nodes; a NotReady node waves, a cordoned one idles |
| Notice board | Failed Jobs and failed pods |
| Harbour | Completed Jobs |
| Builds (for worlds) | The image tags running pods use: a world's release lands when a pod runs an image tagged with it |

`--world shop=k8s:shop-prod` pairs a dev village with it like any other platform.

## Temporal

```bash
agents-tui --headless --temporal agents --temporal-url http://temporal:7243
```

`--temporal <namespace>` (repeatable) adds a village per namespace, read from
the frontend's HTTP API (`--temporal-url`, default `http://localhost:7243`)
with `curl`. It turns on the web view by itself. Add `--temporal-ui
http://temporal:8233` and each workflow links to its page in Temporal's web UI.

| What you see | Where it comes from |
|---|---|
| House | A running top-level workflow (a workstream) |
| People out front | Running child workflows beneath it, at any depth |
| House shape | Words in the workflow type: `SecurityReviewWorkstream` is the security building |
| Busy | Pending activities or children |
| Stalled | An activity retrying after a failure |
| Dark | Its task queue has no workers polling it |
| People in the hall's plaza | Workers polling the namespace's task queues |
| Notes on the board | Top-level workflows that failed, timed out or were terminated in the last 24h |
| Crates at the harbour or launch pad | Top-level workflows completed in the last 24h |

### Telling the village an agent is waiting on you

Temporal can't tell "waiting for a person" from "waiting for a timer", so
agents say so. Register a keyword search attribute named `AgentStatus`:

```bash
temporal operator search-attribute create --namespace agents --name AgentStatus --type Keyword
```

and set it while waiting for the human's signal (Python SDK shown):

```python
workflow.upsert_search_attributes([AGENT_STATUS.value_set("waiting_for_human")])
await workflow.wait_condition(lambda: self.approved is not None)
workflow.upsert_search_attributes([AGENT_STATUS.value_set("working")])
```

A workstream with any workflow set to `waiting_for_human` (or `blocked`)
shows an open door and a waving agent.

## Worlds: the dev village and the platform it builds

```bash
agents-tui --headless --temporal agents --world shop=temporal:agents
```

`--world <dev>=<platform>` (repeatable) pairs a dev village (the coding swarm
that writes the system) with the platform village (the Temporal namespace
that runs it). Pick `shop ⇄ temporal:agents` in the title bar, or open
`/village#world:shop`. The platform is drawn above, in the other style; work
moves between the two in a loop:

| What you see | Where it comes from |
|---|---|
| A burning platform building | An open issue with the incident label (`--incident-label`, default `incident`) whose title names that workflow's id or type |
| A crate floating down the river | A new incident issue, on its way from the building that raised it to the dev village's docks |
| A cart climbing the road | Issues at the dev village's last pipeline stop that no platform worker runs yet |
| Sparkles and fireworks over the platform | A platform worker reporting a build ID that matches the release |

The release is the commit at the dev village's last stop. It has landed when
a worker on one of the platform's task queues reports a build ID (Temporal
worker versioning: `deploymentOptions.buildId`, or the older
`workerVersionCapabilities.buildId`) that is a prefix of it, or it of the
build ID, at least 7 characters long, so build workers with the git sha.
Without build IDs the village can't tell, and says so in the counters.

## Sources

Every village is drawn from one neutral report. Any framework can provide one through an adapter (`--framework`); see [village-protocol.md](village-protocol.md).

The village reads its data in layers, and each layer's source can be swapped
without touching the others (`src/web/source.rs`):

| Layer | Answers | Sources | Trait |
|---|---|---|---|
| villages | what is drawn | swarm villages (from the layers below), `--temporal` namespaces | `VillageSource` |
| swarms | who is working | `tmux`, `herdr`, `saved` (stopped swarms) | `SwarmSource` |
| issues | the backlog | `github`, `none` | `IssueTracker` |
| pipeline | where pushed work is | `autocoder`, `none` | `PipelineSource` |
| activity | what each agent is doing | `claude` (transcripts), `none` | `ActivitySource` |

Below them all, commands run through the transport: locally, or over ssh
with `--server`. Above them, worlds only see villages and their issues, so
any village source can be a platform and any swarm village a dev village.

`--source <layer>=<name>[,<name>]` picks what runs; by default everything
does. Swarm sources are tried in order and the first to show a project owns
it.

```bash
agents-tui --headless --source swarms=herdr                  # herdr only
agents-tui --headless --source swarms=none --temporal agents # a pod watching only Temporal
agents-tui --headless --source issues=none --source pipeline=none
```

To add a source (an AgentEx platform, a Jira backlog, a different CI),
implement the layer's trait and give it a name in the registry at the bottom
of `source.rs`. The startup log lists the sources that are running.
