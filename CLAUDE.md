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
- **A test with a fixed date must not read the real clock** (`Local::now`,
  `Utc::now`, `SystemTime::now`, `Date.now()`, `new Date()` with no argument):
  pass `today` and `now` in. A fixture's day against the real one passes
  until midnight UTC and then fails, on CI first (#535, #536). Check with `TZ=Pacific/Kiritimati
  cargo test` and `TZ=Etc/GMT+12 cargo test`.

- **Each tool's own tests** (the mock demos, the Omarchy plugins, the
  replay tools, ...) are in `docs/testing/`, one file per tool: `ls
  docs/testing/`. A new tool's test instructions go in a new file there, and
  nothing here names it, so two branches adding tools never edit the same
  lines (#521).

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
  adapter/      # agent runtimes (Claude Code): autocoder-ctl verbs (ctl.rs), resume, heal, tmux windows
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
- Swarms are started only with autocoder's `autocoder-ctl start` (`adapter/ctl.rs`), from the TUI and from the web server's `POST /api/swarms` alike; it installs the autocoder plugin when it is missing. agents-tui has no launcher of its own (#546): an autocoder without `start`, or a remote host, is told what to do instead. `resume()` still rebuilds a stopped swarm's worktrees and panes itself.
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

Before merging, the full suite must pass (the Regression Test Suite command
above). How a branch reaches master (rebasing, conflicts, rejected pushes) is
autocoder's merge job's to enforce, not this file's. One fact about
this repo for resolving conflicts: most work here edits the village page
(`src/web/village/`), so other workers' changes land there constantly; read
what they added rather than taking "ours".

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
what `src/web/deployments.rs` reads to place each stop.

Deploying and releasing are autocoder's (laird/agents#200): its
`scripts/deploy.sh` and `scripts/release.sh`, in the installed plugin
(`~/.claude/plugins/cache/<marketplace>/autocoder/<version>/scripts/`). What is
agents-ui's own is the `deploy` entry in `.autocoder.json` (#533), which `cargo
test deploy_environment_deploys_through_autocoder` checks. `deploy` is a
generic name for the machine where agents-tui is installed and served. Rename
that entry, and the `release` entry's `requires`, to whatever you call that
host, and pass the same name to `deploy.sh --env`.

- **`deploy`**: what is installed and served. From a clean checkout of
  a commit already on `origin/master`, run `<autocoder scripts>/deploy.sh --env
  deploy` (add `--dry-run` to check without changing anything). Its steps come
  from `.autocoder.json`: the preflight refuses, before recording anything,
  when the systemd user unit `agents-tui.service` is not installed (the served
  instance is that unit and nothing else; install it with `agents-tui service
  install`, or from the reference unit `docs/agents-tui.service`); `deploy`
  runs `cargo install`; `restart` runs `agents-tui service restart`, which
  restarts the unit and fails unless the village answers at its own
  `--web-port`/`--web-bind` within 30s. The Deployment is recorded as
  `success`, or `failure` if the install or the restart fails.
- **`release`**: a GitHub Release, recorded as a Deployment of its commit.
  **Every deploy releases** (`"release": true` on `deploy`): once the deploy is
  `success`, autocoder's `release.sh` releases the same commit, so the
  village's ship sails on each deploy. Tags are automatic, `vYYYY.MM.DD.N`, and
  a commit that already is the newest release is not released again. A failed
  release leaves the deploy standing: `deploy.sh` exits 3 and the next deploy
  releases it. `--no-release` skips it for one deploy.
  **Pushes of master from the main checkout release too**, once autocoder's
  pre-push hook is installed (`<autocoder scripts>/install-release-hook.sh`,
  once per clone; it replaces the hook agents-ui used to install, which now
  only runs `tools/hooks/pre-push`, a stub that lets pushes through without
  releasing. Delete that stub once every clone has autocoder's hook). Pushes from
  the swarm's worktrees and merge jobs never do. `AUTOCODER_NO_RELEASE=1 git
  push` skips it; the log is `~/.local/state/autocoder/release.log`.
  `release.sh [vX.Y.Z]` releases by hand.

There is no sentinel here, so no `master-green` ref exists. `deploy` is marked
`"requires": "integration"`, which drops the green stop, and the village shows
the gate as `absent`. The environments are moved by deployments, not by
autocoder's `promote.sh`, which only accepts `green` or an earlier environment.
Run `main-sentinel.sh` and remove that `requires` if a green gate is wanted.
