# CLAUDE.md — agents-tui

## Project overview

A Rust TUI application for launching, monitoring, and managing swarms of AI agents working on software repositories. See SPEC.md for full product specification.

## Build & run

```bash
source "$HOME/.cargo/env"
cargo build          # Build
cargo run            # Run the TUI
cargo install --path . # Install globally as `agents-tui`
agents-tui service install  # ...and serve the village as a systemd user service (--dry-run, --linger; see README)
```

To serve the web UI to tailnet devices and this machine together, give
`--web-bind` a comma-separated list (one listener per address; `tailscale`
expands to this machine's tailnet addresses):

```bash
agents-tui --headless --web-port 7890 --web-bind 127.0.0.1,tailscale --read-only
```

Avoid `0.0.0.0`: it also exposes the UI on the LAN, and it has no authentication.

To keep it serving as a user service (systemd on Linux, a launchd agent on macOS):

```bash
agents-tui service install --web-bind 127.0.0.1,tailscale --source swarms=herdr [--read-only] [--linger]
agents-tui service status      # state, URL, whether the village answers, the binary's version
agents-tui service restart     # or uninstall
```

`install --dry-run` prints the unit, what would change, and the commands, and
changes nothing. Re-running `install` shows what changed and restarts.

## Testing

```bash
cargo test           # Run all tests
cargo test -- --nocapture  # Run with stdout visible
cargo test status    # Run tests matching "status"
cargo test swarm     # Run tests matching "swarm"
```

### Test structure

Each module's tests are in its own `#[cfg(test)]` block, and its header comment
says what it is. Don't list modules here: a new module needs no edit to this
file.

- **Rust:** `cargo test` runs them all. The village page's tests (what the page
  embeds and wires up) are in `src/web/village_tests/`, one file per feature
  area.
- **The village's DOM-free decisions** (modules with no DOM, such as the Fit
  scale, the dock order, house labels, build weather and bounce marks) run
  under node: `node --test tools/village-tests/*.test.mjs` (files, not the
  directory: Node 22 and 24 reject a directory, #524),
  also run by `cargo test village_node_tests`.
  They need Node 22 or later.
  A test that draws people or anything else animated must pin the clock
  (`performance.now`): two drawings a few milliseconds apart can land on
  different animation frames, which passes on a fast machine and fails on CI
  (#450).

- **Each tool and area's own tests** (the mock demo and its `VOYAGES` and
  `CONFLICTS` stories, the screensaver, notifications, the bar widget,
  replay pages, history replays) are in `docs/testing/`, one file per tool:
  `ls docs/testing/`. A new tool's test instructions go in a new file there,
  not here, so two branches adding tools never edit the same lines (#521).

Filesystem tests use unique paths under `std::env::temp_dir()` (there is no `tempfile` crate). No mocking of tmux — integration-level tmux testing is done manually via `tmux new-session -d` + `cargo run`.

### Adding tests

Add tests to the `#[cfg(test)] mod tests` block in the relevant source file. For status parsing or data model changes, add unit tests. For tmux interaction changes, test manually.

## Architecture

Directory by directory; each file's header comment says what it does.

```
src/
  main.rs app.rs event.rs tui.rs   # entry point, the terminal UI's state, event loop and raw mode
  model/        # swarms, agent status, issues; recognize.rs: which agents make which swarm (every source shares it)
  tmux/         # tmux sessions, pane capture, send-keys
  adapter/      # agent runtimes (Claude Code): worktrees, tmux windows, launch
  ui/           # the terminal UI's screens: repos, a swarm, an agent, dialogs
  config/       # saved swarm state (~/.agents-ui/), keybindings, shortcuts
  service/      # `agents-tui service`: the served UI as a systemd unit or a launchd agent
  web/          # the HTTP server and API, the swarm and village sources, the village model
    village/        # /village: the swarm as a 16-bit village, one module per feature ("Adding to the village")
    village_tests/  # the village page's tests, one file per feature area
build.rs        # embeds every file under src/web/village/ as VILLAGE_ASSETS
tools/mock/         # simulated herdr/Temporal/Kubernetes swarms for the village; replay pages
tools/omarchy-plugin/ # the Omarchy bar widget and the village screensaver; install.sh
tools/adapters/     # outside framework adapters (docs/village-protocol.md)
tools/village-tests/ # node tests for the village's DOM-free modules
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
3. **Nothing to import by hand**: `build.rs` writes `modules.js`, which
   imports every module (sorted by path) after `main.js` has loaded the
   themes and `layout.js` (#520). Import what the module itself uses; where
   it registers into a shared list (`registerLayer`'s `z`, `registerKey`,
   `registerLabels`, `registerOverlay`, `registerDialogPart`), give it an
   `order` no other module uses, since load order decides nothing
   (`tools/village-tests/loadorder.test.mjs`).
4. **Tests next to the work**: page tests in `src/web/village_tests/` (the
   feature area's file, or a new one), and a DOM-free module's in
   `tools/village-tests/<module>.test.mjs`. No list to add them to.
5. **Nothing to embed by hand**: `build.rs` embeds every file under
   `src/web/village/` (`.html`, `.css`, `.js`; another extension stops the
   build until `content_type()` there names it) as `VILLAGE_ASSETS`.
   `cargo test every_module_main_js_loads` fails for a module whose
   imports aren't embedded, and `cargo test replay_build` fails if two
   modules declare the same top-level name: the replay page bundles them
   into one script.

## Key conventions

- Agent sessions run in tmux. The TUI proxies pane content via `tmux capture-pane -e` and sends input via `tmux send-keys`.
- Build the web model's shared structs (`AgentSnapshot`, `SwarmSnapshot`, `IssueSnapshot`, `VillageSnapshot`, `House`, `Crate`, `Note`) with `..Default::default()` outside their own module, tests included, so adding a field doesn't break other branches (#519). `cargo test shared_model_structs` checks it.
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
