# agents-ui: orientation

## What the product is

agents-ui (command `agents-tui`) is one place to launch, watch, and supervise swarms of AI
coding agents on a developer's own repositories. A **swarm** is a **manager** agent plus
several **workers**; each worker has its own **worktree** (a separate working copy of the
repository on its own branch). It runs as a terminal app (the **TUI**), as a **web
dashboard** in the browser, and as the **village** view, which draws each swarm as a 16-bit
village.

## Who the persona is

A **solo developer** who runs one or two swarms on their own repositories, from a laptop or a
remote machine over SSH. The PM's brief on their behalf: **never miss a blocked agent**.

## The lane: attention and answering

Surface: desktop web (the dashboard and village), 1440×900. Chosen because the PM named herdr
and Temporal/AgentEx as the source of truth for waiting state, and those reach the web side
only; the TUI reads tmux panes directly. *Inference; confirm at the case check-in.*

How it works today, *Inference (from code, not from using the product)*:

1. An agent stops and asks something: a permission prompt ("Allow?", "(y/n)"), a question, or
   it was interrupted and asks what to do instead.
2. agents-ui notices in one of three ways. For a tmux pane it reads the last fifteen lines of
   the screen and looks for those phrases. For herdr (a terminal multiplexer that tracks each
   agent's state itself) it takes herdr's `blocked` status. For a Temporal workflow (a
   long-running job on the Temporal platform) it reads a field the agent sets, `AgentStatus =
   waiting_for_human`.
3. It shows it: the TUI's repos list says "2 input", the repo view header says "2 NEED INPUT",
   the web dashboard turns that agent's dot amber with "needs input", and the village opens the
   house door with an orange "!".
4. The developer gets there: in the TUI a key jumps to the next waiting agent and opens its live
   view; in the browser they open the agent and type into its pane. Nothing is pushed to them:
   they have to be looking.

Out of the lane: launching and recovering swarms; the issue-to-production pipeline.

## Repository map

- `src/model/`: the swarm and agent model, including the waiting detector (`swarm.rs`).
- `src/ui/`: the TUI screens (repos list, repo view, agent view).
- `src/app.rs`: the TUI's event loop and keys, including "jump to next waiting".
- `src/web/`: the web server, dashboard page (`ui.html`), village, and the herdr and Temporal
  readers.
- `tools/mock/`: simulated herdr and Temporal swarms for trying the web views.

## Where to start public research

1. https://github.com/laird/agents-ui/issues
2. https://github.com/maxjendrall/herdr/issues
3. https://github.com/smtg-ai/claude-squad/issues
4. https://github.com/anthropics/claude-code/issues
5. https://github.com/laird/agents/issues
6. Hacker News and Lemmy searches for the lane terms.

## Cautions

No licence file at the pinned commit. Read only: do not run builds, tests, or the mock demo
during discovery. AgentEx is named by the PM but has no reader in the code yet.
