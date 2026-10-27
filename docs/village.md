# Village view

`/village` draws each swarm as a 16-bit village. Run agents-tui with `--web`
(alongside the TUI) or `--headless` (as a service) and open
http://127.0.0.1:7878/village.

Pick a style in the title bar: **Medieval village** or **Modern business
district**. The choice is remembered per browser; `?theme=modern` or
`?theme=medieval` in a link overrides it. Styles change the art, never the
meaning: lit windows mean working, an open door and an orange `!` mean an
agent is waiting on you, dust means stalled, fog means unknown.

The server listens on 127.0.0.1 only. To watch a swarm on another machine,
tunnel to it: `ssh -L 7878:localhost:7878 swarm-host`.

## Coding swarms (autocoder, modernize)

Found automatically from tmux. The manager runs the hall; every worker
session is a house, and agents sharing a worktree are one house.

| What you see | Where it comes from |
|---|---|
| House shape | Labels on the worker's issue (`bug`, `enhancement`, `test-failure`, `security`, ...) |
| Flag on a house | `P0`–`P3` label |
| People out front | Running Claude Code sub-agents (`~/.claude/projects/<worktree>/<session>/subagents/`) |
| Crates at the docks | Open, unclaimed, unblocked issues, highest priority first |
| Notes on the board | Issues with a blocking label (`needs-design`, `proposal`, ...) |
| Crates at the quay | `refs/autocoder/tips/<integration>/<sha>-<issue>` not yet green |
| Locks, then the castle | `<integration>-green`, then each environment in `.autocoder.json` |

## Temporal

```bash
agents-tui --headless --temporal agents --temporal-url http://temporal:7243
```

`--temporal <namespace>` (repeatable) adds a village per namespace, read from
the frontend's HTTP API (`--temporal-url`, default `http://localhost:7243`)
with `curl`. It turns on the web view by itself.

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
| Crates at the end | Top-level workflows completed in the last 24h |

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
