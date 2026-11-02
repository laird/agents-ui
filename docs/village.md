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
| Notes on the board | Issues with a blocking label (`needs-design`, `proposal`, ...) |
| Crates at the quay | `refs/autocoder/tips/<integration>/<sha>-<issue>` not yet green |
| Locks, then the harbour or launch pad | `<integration>-green`, then each environment in `.autocoder.json` |

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
