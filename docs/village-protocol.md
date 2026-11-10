# Village protocol: drawing any agent framework

The village draws two kinds of system:

- **Development:** a swarm of agents that writes the code. Examples are autocoder, modernize, claude-squad and herdr.
- **Runtime:** a platform where agents run in production. Examples are Temporal, AgentEx, and LangGraph or CrewAI deployments.

Each framework names the same few things differently. The village reads them in one neutral shape, an **agents-village report**. Every village is drawn from a report, including the built-in tmux, herdr and Temporal ones (`src/web/framework.rs`).

To add a framework, write an adapter that prints a report. No Rust is needed.

```bash
agents-tui --headless --framework exec:./my-adapter.py           # a command that prints reports
agents-tui --headless --framework https://agentex.local/village  # a URL that returns them
agents-tui --headless --framework ./reports.json                 # a file (file:<path> works too)
```

`--framework` can be repeated. agents-tui reads each adapter every 5 seconds or less often, through `--server` when one is set. An adapter that fails, or that prints something other than a report, is shown as its own village with an "unreachable" note on the board. A broken adapter is therefore visible rather than silently missing.

## How a report maps to the village

| Report | Development village | Runtime village |
|---|---|---|
| `coordinator` | the manager's hall or castle | the worker fleet |
| `workstreams[]` | one house per worktree or session | one house per top-level workflow or task |
| `workstreams[].agents[]` | the house's crew: the sessions on that worktree | the workflow itself |
| `workstreams[].helpers[]` | sub-agents (the Task tool, modernize specialists) | child workflows: the agents working for it |
| `tasks[]` | issues: crates at the docks, notes on the board | failed runs pinned to the board |
| `pipeline` | the canal: landed, then each stage to production | completed runs at the castle |
| `tasks[].parent` | features: a banner per parent, a trail to its subtasks; a subtask's crate edged in its flag's colour | features, the same way |
| `tasks[].depends_on` | a roped crate that "waits on #N" | the same |
| `builds[]` | none | versions the workers run; a world matches releases against these |
| `plane` | `development` | `runtime` |

A world (`--world dev=platform`) pairs a development village with a runtime village. Any two villages can be paired, whatever framework drew them.

## The report

```json
{
  "schema": "agents-village/v1",
  "project": "larkspur",
  "plane": "development",
  "framework": "claude-squad",
  "location": "/src/larkspur",
  "coordinator": { "id": "manager", "name": "manager", "agents": [{ "name": "manager", "state": "working" }] },
  "workstreams": [
    {
      "id": "/src/larkspur-wt-2",
      "task": "#43",
      "agents": [{ "name": "worker-2", "state": "blocked", "detail": "Allow `npm publish`?" }],
      "helpers": [{ "name": "a7", "kind": "Explore", "state": "running", "action": "searching" }]
    }
  ],
  "tasks": [
    { "id": "43", "title": "Checkout 500 on empty basket", "labels": ["bug"], "priority": "P0" },
    { "id": "45", "title": "Which payment provider?", "labels": ["needs-design"], "state": "blocked" }
  ],
  "pipeline": {
    "integration": "main", "gate": "green", "landed": ["40"],
    "stages": [{ "name": "main-green", "tasks": ["39"] }, { "name": "prod", "tasks": ["37"] }],
    "release": "1f2e3d4c5b6a7980"
  },
  "builds": []
}
```

[`tools/adapters/example-report.json`](../tools/adapters/example-report.json) is a complete example with one development and one runtime village. [`tools/adapters/adapter-template.py`](../tools/adapters/adapter-template.py) is an adapter skeleton to copy.

An adapter may print one report, a list of reports, or `{"reports": [...]}`. Only `project` is required; everything else has a default.

### Fields

**Report**

- `project` (required): the village's name. Use a prefix for runtimes, such as `agentex:support`, so names don't clash.
- `plane`: `development` (the default) or `runtime`.
- `framework`: the framework's name, shown in the title bar.
- `workflow`: optional, a finer name such as `Modernize`.
- `location`: a repository path or server URL.
- `coordinator`: a workstream. Without one, the hall stands empty.
- `workstreams`, `tasks`, `pipeline`, `builds`.

**Workstream**

- `id` (required): a stable key, such as a worktree path or workflow id. A house keeps its spot on the map while its `id` stays the same.
- `name`: shown as the house's role. Defaults to the first agent's name.
- `task`: the id of the task it is working on. The task's labels decide the building.
- `title`: shown instead of the task's title.
- `branch`: the branch the workstream works on. The house's dialog is titled by worktree and branch, and links a compare against the integration branch.
- `kind`: the building to draw when no task labels decide it. One of `cottage`, `security`, `tests`, `bug`, `perf`, `feature`, `refactor`, `docs`, `chores`, `research`, `workshop`.
- `state`: the house's state. Set it when the framework knows better than its agents do (a Temporal workflow that is retrying, for example). By default the crew decides: anyone waiting wins, then any work at all, and a running helper makes an idle house busy.
- `action`: what the house is doing now (see below).
- `agents`, `helpers`: lists of agents.
- `question`, `answer_url`: a structured question the house's agent is waiting on (`{prompt, kind: menu | yes_no, options: [{key, label}], selected}`) and where to POST `{prompt, option}` to answer it. The dialog shows one button per option. Built-in tmux swarms fill these in; an outside adapter may too, if it serves the answer endpoint itself.
- `links`: where to drill in, each `{label, url}`: the agent's session, the run in its platform's UI, a CI job. Shown as buttons when the house is selected, before the task's own link.

**Agent**

- `name` (required).
- `kind`: the agent or workflow type, when it differs from the name. For helpers, `kind` picks a specialist building: `*:security`, `*:tester`, `*:architect`, `*:documentation`, `*:coder`.
- `state`: see below.
- `action`: see below.
- `detail`: a few words on what it is doing, shown when you click the house.

**State.** Frameworks' own words are accepted:

| State | Also accepted |
|---|---|
| `waiting` (needs a person) | `blocked`, `waiting_for_human`, `needs_input`, `awaiting`, `approval`, `paused` |
| `working` | `running`, `busy`, `active`, `executing`, `in_progress`, `Working #412` |
| `idle` | `ready`, `available`, `sleeping` |
| `starting` | `pending`, `scheduled`, `launching`, `restarting` |
| `stalled` | `stuck`, `retrying`, `backoff` |
| `done` | `completed`, `finished`, `succeeded` |
| `stopped` | `dead`, `exited`, `terminated`, `cancelled`, `failed`, `timed_out` |

Matching ignores case. Anything else counts as `unknown`.

**Action.** Each action has its own animation: `thinking` (a thought cloud), `reading` (a book), `writing` (a hammer or keyboard), `running` (a terminal), `searching` (a magnifying glass), `messaging` (a letter).

**Task**

- `id` (required): `412`, `#412`, `PROJ-12`, or a workflow id. Numeric ids keep their number; other ids are hashed to a stable one.
- `title`, `labels`, `priority` (`P0` to `P3`), `url`: the task in its tracker. Its house, crate or note links to it.
- `parent`: the id of the task this one is a subtask of. Tasks that share a parent make a **feature**: a banner the village follows from spec to production (see [Following a feature](village.md#following-a-feature)). Setting `parent` is all an adapter does; the village works out where each subtask is and how far the feature has got. The parent's title and url come from its own task when the report lists it. A subtask in state `done` counts as closed; a report has no "not planned", so every subtask counts. A feature is shown only while one of its subtasks is not yet at the last stop (see [When a banner flies](village.md#following-a-feature)); once all are, it is left out of `/api/village`.
- `depends_on`: ids of tasks this one waits on. While any of them is in the report and not `done`, the task's crate is drawn roped at the docks, stands after the crates it waits on, and says "waits on #N". A dependency the report doesn't list counts as met. Swarms fill it from "Depends on" / "Blocked by" lines in the issue body, the rule autocoder uses before claiming.
- `state`:
  - `open` (the default): a crate at the docks;
  - `claimed`: taken, so not in the backlog;
  - `blocked`: a note on the board;
  - `failed`: a note on the board;
  - `done`: not drawn.

  A blocked task is pinned under its blocking label (`needs-design`, `needs-approval` and so on) when it has one.
- `gate`: where the task is in the **merge gate**, once its work has left the worker. Leave it out when the framework has no gate, or can't tell. Fields:
  - `state`:

    | `state` | Meaning |
    |---|---|
    | `queued` | handed to a merge job, waiting for a gate slot |
    | `merging` | merging onto the integration branch, re-syncing, or publishing |
    | `gating` | the regression suite is running on the integrated tree |
    | `passed` | landed, tested and closed |
    | `landed-unproven` | landed on another basis, waiting for the sentinel to prove a green tip |
    | `failed` | the gate failed on the integrated tree |
    | `conflict` | the merge conflicted |
    | `red-base` | the integration branch is red without this change |

  - `landed`: whether the work is on the integration branch. A `red-base` landing is, a refused one isn't.
  - `slot`: the gate slot it holds while gating, numbered from 1.
  - `started_at` (RFC 3339) and `elapsed_secs`: when the merge job started, and for how long it has run.
  - `tests_run`, `progress`: how far the gate has got, e.g. `412 tests run (3 suites)`.
  - `attempt`: gate runs in this job. It is 2 once a re-sync made it gate again.
  - `failure_url`: where the gate said what went wrong, for `failed`, `conflict` and `red-base`.
  - `train`: the id of the merge train the task rides in.

  A task that is `queued`, `merging` or `gating` is not at the quay, even if `landed` lists it: the quay is for landed work. A `failed` or `conflict` task should be `state: failed`, and is pinned to the board as `failed` or `conflict`.

**Pipeline**

- `integration`: the branch or queue where finished work lands.
- `gate`: `green`, `pending` or `unknown`.
- `landed`: task ids that have landed but not yet reached the first stage. These wait at the quay.
- `stages`: in order, each `{name, tasks}`. A task appears in the furthest stage it has reached. A stage may also give:
  - `source`: what says what the stage runs: `deployment` (a deployment record), `release` (a published release), `tag` (a version tag), `url` (a version endpoint of the running service), `branch` (a branch or ref, the default), or `unknown` (nothing does, so the village can't tell, which is not the same as "nothing has reached it");
  - `sha`: the commit the stage runs, when a deployment, release, tag or endpoint names it;
  - `tag`: the release the stage runs, such as `v0.5.0` (the harbour's ship says "next release: N issues since v0.5.0");
  - `url`: a page about that release, such as its GitHub Release;
  - `kind`: `environment` (the default: a village of its own in the environment row) or `gate` (a check such as a green ref, drawn as a lock);
  - `deployed_at` and `deployed_by`: when the running version was deployed (RFC 3339) and by whom;
  - `deployment_url`: the page listing the stage's deployments;
  - `cargo`: every task the stage runs, newest first, and `cargo_more`, how many more were left out. When no stage gives `cargo`, the village works it out from `tasks`: a stage runs its own tasks and every later stage's;
  - `health`: `{state, reason, url}`, where `state` is `ok`, `failed` (its newest deployment failed), `incident` or `unknown`.
- `voyages`: deployments between stages, newest first, each `{from, to, state, sha, tag, started_at, finished_at, url, tasks, tasks_more}`. `from` is a stage's name, or `dev` for the village that builds the work. `state` is `loading` (queued or pending), `sailing` (in progress), `docked` (succeeded) or `failed`. `tasks` are what the deployment adds to what `to` ran before; `tasks_more` says there is more than the adapter could list. See [the environment row](environment-row.md).
- `release`: the version at the last stage. A world compares it with the platform's `builds`, and either may be a prefix of the other (at least 7 characters).
- `promotion`: `subtask` (the default: each task moves through the stages on its own) or `feature` (a feature's landed tasks wait at the quay and move together).
- `merge`: the merge gate on the host that lands the work:
  - `slots`: how many gates may run at once;
  - `busy`: the slots in use, by number;
  - `trains`: merge trains, the branches gated together as one tree. Each is `{id, task, members, state}`, where `task` is the task the train's merge job runs under, `members` the tasks it carries, and `state` a gate state as above.

`/api/village` reports the same per village: `pipeline.gates` (one `{issue, state, ...}` per task in the gate) and `pipeline.merge`.
Each stage is a `pipeline.stops` entry with the fields above (`cargo` as issue numbers), and `pipeline.voyages` lists the voyages with `issues` and `issues_more` in place of `tasks` and `tasks_more`.

## Replaying history

`tools/adapters/history-replay.py` plays a repository's past back in the live village. It reads recordings made by `tools/mock/history.py`. The village is fully interactive: houses, crates and notes open their GitHub issues, and features follow their subtasks down the canal.

```bash
python3 tools/mock/history.py --repo ~/src/agents-ui --since 2026-03-20T00:00Z --until 2026-11-08T17:00Z \
    --step 1800 --idle-gap 3600 --run agents-ui hist-agents-ui.json.gz
agents-tui --headless --read-only --source swarms=none \
    --framework "exec:python3 tools/adapters/history-replay.py hist-agents-ui.json.gz --every 6"
```

- Each recording becomes one village named `replay:<project>` (change the prefix with `--prefix`), so it never clashes with a live village of the same repository. Give several recordings to get several villages.
- The title bar reads `history replay i/N`.
- The replay clock starts the first time a recording is played. It advances one recorded frame every `--every` seconds (default 6, since agents-tui polls every 5 seconds at most) and loops at the end.
- The clock and an index of each recording's frames live in `--state` (default `$XDG_STATE_HOME/agents-tui/history-replay`). The index lets a poll read one frame instead of decompressing the whole recording.
- To start a replay over, run the adapter once by hand with `--restart`. Don't put `--restart` in the `--framework` command: agents-tui reruns that command on every poll, so the replay would never leave the first frame.
- Subtasks closed as not planned are left out, because a report has no "not planned" state and every subtask counts toward its feature.

## Built-in adapters

| Framework | Plane | Reads |
|---|---|---|
| autocoder, modernize (tmux) | development | tmux panes, status files, the issue tracker, autocoder's git refs, Claude Code transcripts |
| herdr | development | `herdr agent list`; the issue comes from the branch name |
| Kubernetes | runtime | `kubectl get <workloads>,pods -o json` and `get nodes` (`--runtime k8s:[<context>/]<namespace>`): workloads are houses, pods the people, nodes the hall; stuck pods need you; image tags are the builds |
| Temporal | runtime | the frontend HTTP API: running and closed workflows, pending activities, task-queue pollers and their build IDs; with `--temporal-ui <url>`, each workflow links to its page in Temporal's web UI |

The `--source` layers (see `village.md`) choose which of these run. AgentEx runs its agents as Temporal workflows, so `--temporal <namespace>` already shows them. Use an adapter for anything AgentEx keeps outside Temporal, such as task messages.

### What autocoder tells the village

autocoder's manager and workers do not message each other. They coordinate through the issue tracker, git and the terminal multiplexer. The village reads the same signals:

| autocoder | Village |
|---|---|
| label `working`, the `feature/issue-N` branch | the worker's house is on that issue |
| labels `needs-design`, `needs-clarification`, `needs-feedback`, `needs-approval`, `too-complex`, `future`, `proposal`, `blocked` | a note on the board |
| label `decomposed` | taken: its sub-issues are what gets worked |
| labels `merging`, `awaiting-integration` | the fix has left the worker and waits at the quay |
| the merge job's `${AUTOCODER_TMP:-/tmp}/autocoder-merge-<repo-id>-<issue>.{pid,log,exit,run.sh}`, the gate slot locks `autocoder-gate-slot-<k>.lock` | the task's `gate`: queued, merging, gating, then passed, landed-unproven, failed, conflict or red-base; its slot, timing, attempt and test progress. Work still in the gate, or handed back by it, is not at the quay. A `train/<UTC>` branch is a merge train. These files are local, so only a village served on the host that runs the merges shows them. |
| `refs/autocoder/tips/<integration>/<sha>-<issue>` | where a landed fix is on the canal |
| `<integration>-green` | the sentinel's lock |
| `env/<name>` from `.autocoder.json` `environments`, or the newest successful GitHub Deployment to `<name>` | one lock per environment, the last being the castle |
| GitHub sub-issues, or `Sub-task of #N` in a body under a `decomposed` parent | a feature's banner and its subtasks |
| a pane waiting on a prompt, herdr `blocked` | a house with an open door and an orange "!" |
| Task-tool sub-agents, modernize specialists | villagers working for the house |
