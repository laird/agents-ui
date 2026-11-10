# CLAUDE.md — agents-tui

## Project overview

A Rust TUI application for launching, monitoring, and managing swarms of AI agents working on software repositories. See SPEC.md for full product specification.

## Build & run

```bash
source "$HOME/.cargo/env"
cargo build          # Build
cargo run            # Run the TUI
cargo install --path . # Install globally as `agents-tui`
```

To serve the web UI to tailnet devices and this machine together, give
`--web-bind` a comma-separated list (one listener per address; `tailscale`
expands to this machine's tailnet addresses):

```bash
agents-tui --headless --web-port 7890 --web-bind 127.0.0.1,tailscale --read-only
```

Avoid `0.0.0.0`: it also exposes the UI on the LAN, and it has no authentication.

## Testing

```bash
cargo test           # Run all tests
cargo test -- --nocapture  # Run with stdout visible
cargo test status    # Run tests matching "status"
cargo test swarm     # Run tests matching "swarm"
```

### Test structure

Tests are inline `#[cfg(test)]` modules in each source file:

- `src/model/status.rs` — Status line parsing, state detection, issue extraction, file I/O
- `src/model/swarm.rs` — Agent counting, lookups, state queries, type methods
- `src/model/recognize.rs` — swarm recognition: repo identity (git common dir), worktree membership at any depth, worker numbers, same-name repos, one repo through two sources; the manager by evidence (manifest pane, `AUTOCODER_MANAGER` marker in a descendant process, label, main checkout), ties marked ambiguous, manifest parsing
- `src/config/persistence.rs` — TOML save/load round-trips, listing, missing files
- `src/app.rs` — Key-to-tmux conversion, pane status inference, tab completion helpers
- `src/web/village.rs` — House kinds and statuses, crew grouping, backlog/board sorting, tip-ref and environment parsing, transcript actions
- `src/web/framework.rs` — state words, task numbers, report → village mapping, autocoder labels, outside adapters (`tools/adapters/example-report.json`)
- `src/web/source.rs` — `--source` parsing, swarm-source precedence, any village source as a world's platform
- `src/web/deployments.rs` — newest successful GitHub Deployment per environment, from JSON fixtures
- `src/web/gate.rs` — the merge gate from a merge job's `.pid`/`.log`/`.exit` files and labels: every state, slots from `/proc/locks`, test progress, trains, repo ids
- `src/web/feature.rs` — features from links and the village: where each subtask is, Build/stage counts, not-planned leaves the total, stuck, live, one-day retention, report tasks with a `parent`
- `src/model/issue.rs` — feature links: sub-issues else `Sub-task of #N` bodies, spec links in the parent body
- `src/web/kubernetes.rs` — runtime specs, pod states and why they need you, image versions, workloads → houses, from JSON fixtures
- `src/web/herdr.rs`, `src/web/temporal.rs` — herdr and Temporal parsing and village building, from JSON fixtures
- `src/web/world.rs` — world specs, build-ID matching, incident origins, shipping vs landed
- `src/web/dormant.rs` — dormant swarms: a manifest with nothing live, live winning over it, no manifest no swarm

For the village UI, `tools/mock/demo.sh` runs agents-tui against simulated herdr and Temporal swarms.
`VOYAGES=1 tools/mock/demo.sh` scripts Deployments for this repo's own environments (`nuc-1`, then `release`): a deploy, a promotion and a failed deploy on a 120 s loop, so the environment row's voyages can be seen without real ones (`tools/mock/voyages.py`; tests: `python3 -m unittest discover -s tools/mock -p 'test_voyages.py'`).
The village screensaver (`/village?screensaver=1`, `src/web/village/saver.js` + `screensaver.js`; the Omarchy service, launcher and installer in `tools/omarchy-plugin/agents-village-screensaver/`): `node --test tools/omarchy-plugin/agents-village-screensaver/tests/` and `bash tools/omarchy-plugin/agents-village-screensaver/tests/test_install.sh` (both run by `cargo test screensaver`), and `python3 tools/omarchy-plugin/agents-village-screensaver/tests/qml_check.py` (the real `Service.qml` in Quickshell, offscreen). Install with `tools/omarchy-plugin/install.sh --screensaver [--never-sleep] [--host <ssh host>]`.
The Omarchy bar widget (`tools/omarchy-plugin/agents-village/`): `node --test tools/omarchy-plugin/agents-village/tests/` (its model, against recorded `/api/swarms` and `/api/village`) and `python3 tools/omarchy-plugin/agents-village/tests/qml_check.py` (the real `Panel.qml` in Quickshell, offscreen). Install with `tools/omarchy-plugin/install.sh [--host <ssh host>] [--enable]`.
The village's DOM-free decisions, the Fit scale (`src/web/village/fitscale.js`), each epic's flag colour (`featureColours` in `palette.js`) and the dock arrangement (`docks.js`: crates in dependency order, ready or waiting): `node --test tools/village-tests/` (also run by `cargo test village_node_tests`).
`tools/mock/replay-build.py` bundles the village's modules into one self-contained replay page (each module after its imports; two modules declaring the same top-level name stop the build); its tests: `python3 -m unittest discover -s tools/mock -p 'test_replay_build.py'` (also run by `cargo test replay_build`).
`tools/mock/history.py` rebuilds a replay of a repo's real development from git and GitHub history (`--no-github` for git only); its tests: `python3 -m unittest discover -s tools/mock -p 'test_history.py'`.

Tests use `tempfile` for filesystem tests. No mocking of tmux — integration-level tmux testing is done manually via `tmux new-session -d` + `cargo run`.

### Adding tests

Add tests to the `#[cfg(test)] mod tests` block in the relevant source file. For status parsing or data model changes, add unit tests. For tmux interaction changes, test manually.

## Architecture

```
src/
  main.rs           # Entry point, logging, terminal init
  app.rs            # App state, event loop, key handling, navigation
  event.rs          # Async event system (keys, ticks, pane updates)
  tui.rs            # Terminal raw mode setup/teardown
  model/
    status.rs       # AgentState/AgentStatus types, status file parsing
    swarm.rs        # Swarm/AgentInfo types, AgentType, Workflow
    recognize.rs    # Swarm recognition: agents → repos by git common dir and worktree (shared by every source)
  tmux/
    session.rs      # Session discovery, pane listing
    proxy.rs        # Pane capture, send-keys, background watchers
  adapter/
    traits.rs       # AgentRuntime trait
    claude.rs       # Claude Code adapter (worktrees, tmux, launch)
  scripts/
    launcher.rs     # Script/plugin path resolution
  ui/
    repos_list.rs   # Level 1: repos dashboard
    repo_view.rs    # Level 2: manager + workers
    agent_view.rs   # Level 3: live agent session (passthrough)
    new_swarm.rs    # Launch dialog
    theme.rs        # Color/style constants
  config/
    persistence.rs  # Swarm state save/load (~/.agents-ui/)
  web/
    server.rs       # HTTP API + embedded pages (`--web` / `--headless`)
    framework.rs    # Neutral agents-village report: every village is drawn from one; --framework reads outside adapters
    source.rs       # Swappable source layers (villages, swarms, issues, pipeline, activity), `--source`
    discovery.rs    # Polls the swarm sources; the tmux and saved-swarm sources
    dormant.rs      # Dormant swarms: a known repo with a swarm manifest but nothing running
    herdr.rs        # Swarms under herdr: `herdr agent list`, grouped by worktree cwd
    pane.rs         # Read and type into an agent's pane through its multiplexer (tmux or herdr)
    temporal.rs     # Temporal namespaces as villages, via the frontend HTTP API
    kubernetes.rs   # A Kubernetes namespace as a runtime village (alternative to Temporal), via kubectl JSON
    village.rs      # Village model: houses per worktree, issue crates, pipeline from git
    deployments.rs  # What each environment runs: newest successful GitHub Deployment (else its branch ref)
    gate.rs         # The merge gate: autocoder's merge jobs on this host (queued, gating, failed, ...), slots, trains
    feature.rs      # Features: a parent issue's subtasks followed from spec to production
    world.rs        # Worlds: a dev village paired with the platform it builds (incidents down, releases up)
    village/        # /village: the swarm as a 16-bit village, embedded in the binary and served from /village/<path>
      index.html    #   the page's markup
      village.css   #   its styles
      main.js       #   only imports the modules (each registers itself) and starts the page
      registry.js   #   registerLayer/Hit/Labels/Overlay, and the feature-free core: paint(), tap(), hitAt()
      state.js      #   what the drawing reads and the page sets: `state.T` (theme), the scene being drawn, scenes, view
      layout.js     #   where things stand (layout(), world geometry, shownScenes) and the ground layers
      lifecycle.js  #   each issue's life: docks, hall, board, houses, walkers (layers, hits, labels)
      docks.js      #   no DOM: the docks' crate order (prerequisites first, a chain as one run) and each crate's state
      trips.js      #   what each change between two polls sets off: diff(), the queued trips and where each is
      ship.js       #   the canal: quay, locks, harbour and the ship that sails on a release
      features.js   #   feature banners, trails, barges, fog, the strip and the feature dialog
      envrow.js     #   the environment row: a village per environment (sign, warehouse of cargo, health, rank), the mini-map
      voyages.js    #   ships between the row's villages: loading, sailing, docked, failed, and voyages the page plays itself
      worlds.js     #   a world drawn as platform over dev village; draw(); the bar's line for a world (worldWords)
      panels.js     #   toast, zoom and Fit
      fitscale.js   #   Fit's scale for a map in the room left for it (no DOM; tools/village-tests/)
      dialogs.js    #   explain()/act() dispatch on registered hits; the hit test; keyboard targets
      labels.js     #   area labels from registerLabels, kept clear of the art and each other
      key.js        #   the Key (legend), drawn with the renderer's own functions
      data.js       #   the feed: poll, scenes, select a village or world, the theme picker, the loop
      saver.js      #   the screensaver's decisions (no DOM: options, strip counts, cycling, drift, night dimming)
      screensaver.js #  ?screensaver=1: the map full screen, the big status strip, "can't reach", burn-in care
      palette.js    #   colours and drawing primitives (`P`, `R`, `hash`, `blink`, `shadow`)
      sprites/houses.js # houses, one sprite per kind
      scenery.js    #   crates, bubbles, +N badges, grass, paths, water, trees, castle, board, locks, harbour, ship, boat
      people.js     #   people, robots, drones and what they act out
      themes/       #   registry.js (`registerTheme`, `THEMES`: the Style picker lists them) and medieval/modern/future.js, each registering itself
tools/mock/         # Simulated herdr/Temporal/Kubernetes for the village: demo.sh (RUNTIME=k8s), record.py + replay-build.py, history.py (replay from git/GitHub history)
tools/omarchy-plugin/ # Omarchy bar widget (agents-village/) and the village as the screensaver (agents-village-screensaver/); install.sh
tools/adapters/     # Outside framework adapters: example report, adapter template (docs/village-protocol.md)
```

### Adding to the village

The village page's core (`paint()`, the hit test, `explain()`/`act()`,
`placeLabels()`) names no feature: it walks what the feature
modules register (`registry.js`). To add something to the village:

1. **A new module** in `src/web/village/`, importing what it draws with from
   `palette.js`, `scenery.js`, `people.js`, and `layout.js` for positions.
   Keep each file under ~400 lines; split one that grows past that.
2. **Register what it is**, at the module's top level:
   - `registerLayer({ z, draw(c, scene, t) })` to draw. `z` places it among
     the others (ground 0–40, the hall 50, houses 70, ship 110, walkers 120,
     banners 140). Inside `draw`, `tap(x, y, w, h, { kind, ... })` makes what
     was just drawn clickable.
   - `registerHit({ kind, dialog(obj, scene, theme) })` for what a click on
     that kind says. `act` instead of a dialog does something; `key: false`
     gives it no keyboard target.
   - `registerLabels(fn, order)` for its names on the map, and
     `{ avoid: rect }` items for sprites labels must keep clear of.
   - There is no row of counters under the map (#405): a count worth
     showing goes in the area's label (`sub`) or its dialog.
   - A theme registers with `registerTheme` (`themes/registry.js`).
3. **Import it in `main.js`.** That is all `main.js` does: import the
   modules, then `start()`.
4. **Embed it**: add the file to `VILLAGE_ASSETS` in `src/web/server.rs`.
   `cargo test every_module_main_js_loads` fails for a module that is
   imported but not embedded, or embedded but never imported.
   `cargo test replay_build` fails if two modules declare the same
   top-level name: the replay page bundles them into one script.

## Key conventions

- Agent sessions run in tmux. The TUI proxies pane content via `tmux capture-pane -e` and sends input via `tmux send-keys`.
- Session naming: `claude-<project>` with window 0 "agents" (workers) and window 1 "review" (manager).
- Worktree naming: `<repo>-wt-<N>` in the repo's parent directory.
- Status is inferred from pane content when no `.codex/loops/fix-loop.status` file exists.
- ANSI colors are preserved via `ansi-to-tui` crate.
- Keystroke passthrough in agent view — all keys forwarded to tmux pane except: double-Esc (back), Alt+0-9/m (navigation), PgUp/PgDn (scroll).
- The autocoder plugin is auto-installed via `claude plugin marketplace add laird/agents` + `claude plugin install autocoder` if not present.
- Claude is launched with `--append-system-prompt` to force tmux (not cmux) usage.

## Automated Testing & Issue Management

This section configures the `/fix` command for autonomous issue resolution.

### Regression Test Suite
```bash
source "$HOME/.cargo/env" && cargo test
```

### Build Verification
```bash
source "$HOME/.cargo/env" && cargo build
```

The `source` matters: merge gates and other non-interactive shells do not have
`~/.cargo/bin` on `PATH`, and without it the gate fails with
`cargo: command not found` before any test runs.

### Test Framework Details

**Unit Tests**:
- Framework: Rust built-in test framework
- Location: inline `#[cfg(test)]` modules in each source file

**Test Reports**:
- Location: `docs/test/regression-reports/`


## Landing work

### Merge Mode
```
merge
```
Options: `merge` (merge to the integration branch and push) or `pr` (push a
feature branch and open a pull request, then stop).

**Do not open pull requests in this repository.** Merge completed, tested work
straight to the integration branch and push. A PR here is not review latency
that buys safety — it is a window in which another worker picks up the same
issue and writes the fix a second time, because the issue is still open and the
work is invisible on the integration branch. That has already happened: #329
was fixed twice, on two branches, by two workers.

Before merging: the full suite must pass (`cargo test`), and the merge must be
conflict-free. If it conflicts, rebase onto the integration branch and re-run
the tests rather than merging with conflicts resolved blindly.

**Always rebase before you merge.** Immediately before launching the merge
(`merge-launch.sh` / `merge-to-integration.sh`), every worker must:

1. `git fetch origin` and `git rebase origin/master` on its feature branch.
2. Resolve any conflicts itself, keeping both sides' behaviour. Most work here
   edits the village page (`src/web/village/`), so other workers' changes land there
   constantly; read what they added rather than taking "ours".
3. Re-run the full suite (`source "$HOME/.cargo/env" && cargo test`) on the
   rebased branch, and only then launch the merge.

The merge job's own pre-integration sync then finds nothing to do. If the merge
job still reports a conflict (master moved again in the meantime), that is not
a `needs-clarification` question for the user: rebase again, re-test and
re-launch. Escalate to the user only if the same conflict comes back twice.

### Integration Branch
```
master
```
The shared branch all completed work is merged into. In a parallel-worktree
swarm this MUST be the real shared branch, never a per-worktree branch —
otherwise fixes strand on `master-wt-N` and never converge.

Note this repository's default branch is `master`, not `main`.

### Deploying and releasing

Landed work waits at the village quay until it is deployed. `.autocoder.json`
lists two environments, and both are recorded as GitHub Deployments, which is
what `src/web/deployments.rs` reads to place each stop:

- **`nuc-1`**: what is installed and served on nuc-1. From a clean checkout of
  a commit already on `origin/master`, run `tools/deploy.sh` (add `--dry-run`
  to check without changing anything). It runs `cargo install`, restarts the
  systemd user unit `agents-tui.service`, waits until the village answers at
  the unit's own `--web-port`/`--web-bind`, and records the Deployment as
  `success`. If the install, the restart or that check fails, it records
  `failure`. The served instance is that unit and nothing else: when it is
  not installed, `deploy.sh` exits 1 before recording anything, and it never
  starts a server of its own. The reference unit, with install steps, is
  `docs/agents-tui.service`. Its tests: `bash tools/test_deploy.sh` (also run
  by `cargo test deploy_script`).
  CI runs these on macOS too, so the scripts and their tests stay portable:
  bash 3.2 (an empty array is `${a[@]+"${a[@]}"}` under `set -u`), BSD
  `sort`/`date`/`sed` (no `sort -V`, no `date -I`, no `sed -i`). `cargo test
  deploy_and_release_scripts_avoid_linux_only_shell` catches the known traps.
- **`release`**: a GitHub Release, recorded as a Deployment of its commit.
  **Every deploy releases**: once `deploy.sh` has recorded `nuc-1` as
  `success`, it runs `tools/release.sh` for the same commit, so the village's
  ship sails on each deploy. Tags are automatic, `vYYYY.MM.DD.N` (N counts up
  within the day, from origin's tags), and a commit that already is the newest
  release is not released again. A failed release leaves the deploy standing:
  `deploy.sh` exits 3 and the next deploy releases it. `--no-release` skips it
  for one deploy, and `--dry-run` shows the tag and commit it would release.
  **Pushes of master from the main checkout release too**, once its pre-push
  hook is installed (`tools/install-hooks.sh`, once per clone). Pushes from the
  swarm's worktrees and merge jobs never do: they share the hooks but not the
  main checkout. `AGENTS_UI_NO_RELEASE=1 git push` skips it; the log is
  `~/.agents-ui/release.log`. `tools/release.sh [vX.Y.Z]` still releases by
  hand, and for a release made on GitHub it only records the Deployment. Its
  tests: `bash tools/test_release.sh` (also run by `cargo test release_script`).

There is no sentinel here, so no `master-green` ref exists. `nuc-1` is marked
`"requires": "integration"`, which drops the green stop, and the village shows
the gate as `absent`. The environments are moved by deployments, not by
autocoder's `promote.sh`, which only accepts `green` or an earlier environment.
Run `main-sentinel.sh` and remove that `requires` if a green gate is wanted.
