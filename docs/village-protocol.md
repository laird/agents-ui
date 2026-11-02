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
- `state`:
  - `open` (the default): a crate at the docks;
  - `claimed`: taken, so not in the backlog;
  - `blocked`: a note on the board;
  - `failed`: a note on the board;
  - `done`: not drawn.

  A blocked task is pinned under its blocking label (`needs-design`, `needs-approval` and so on) when it has one.

**Pipeline**

- `integration`: the branch or queue where finished work lands.
- `gate`: `green`, `pending` or `unknown`.
- `landed`: task ids that have landed but not yet reached the first stage. These wait at the quay.
- `stages`: in order, each `{name, tasks}`. A task appears in the furthest stage it has reached.
- `release`: the version at the last stage. A world compares it with the platform's `builds`, and either may be a prefix of the other (at least 7 characters).

## Built-in adapters

| Framework | Plane | Reads |
|---|---|---|
| autocoder, modernize (tmux) | development | tmux panes, status files, the issue tracker, autocoder's git refs, Claude Code transcripts |
| herdr | development | `herdr agent list`; the issue comes from the branch name |
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
| `refs/autocoder/tips/<integration>/<sha>-<issue>` | where a landed fix is on the canal |
| `<integration>-green` | the sentinel's lock |
| `env/<name>` from `.autocoder.json` `environments` | one lock per environment, the last being the castle |
| a pane waiting on a prompt, herdr `blocked` | a house with an open door and an orange "!" |
| Task-tool sub-agents, modernize specialists | villagers working for the house |
