# ADR 0001: Runtime Adapter Integration Pattern

## Status

Accepted

## Context

`agents-ui` manages multi-agent swarms across multiple CLI runtimes. Claude, Codex, and Droid all expose different operational models:

- Claude uses a persistent interactive CLI session plus slash commands.
- Codex uses installed skills plus shell wrappers such as `codex-fix-loop.sh` and `codex-monitor-workers.sh`.
- Droid uses plugin/skill assets plus shell wrappers such as `droid-fix-loop.sh` and `droid-monitor-workers.sh`.

The app originally treated runtimes too uniformly. That was good enough for Claude but incorrect for Codex and partially incorrect for Droid. Adding Codex and Droid support exposed the need for an explicit runtime-integration pattern.

## Decision

New runtimes must be integrated in three layers:

1. Runtime identity and command model
2. Installation and readiness checks
3. UI selection and swarm orchestration

The source of truth for runtime behavior is the local shared `../agents` repository. `agents-ui` must not invent command shapes when the scripts/skills already define them.

## Implementation Pattern

### 1. Model the runtime explicitly

Add the runtime to [`src/model/swarm.rs`](/Users/Laird.Popkin/src/agents-ui/src/model/swarm.rs):

- `AgentType`
- display name
- tmux session prefix
- status directory
- launch behavior
- worker-loop behavior

Important distinction:

- If a runtime is interactive and long-lived, `launch_cmd()` and `worker_loop_cmd()` may return inline commands.
- If a runtime is script-driven, `launch_cmd()` and `worker_loop_cmd()` should return empty strings and orchestration should invoke runtime scripts instead.

Examples:

- Claude: inline launch + slash commands
- Codex: no inline launch; use `codex-*.sh`
- Droid: no inline launch; use `droid-*.sh`

### 2. Keep the tmux adapter generic but runtime-aware

Use the shared tmux adapter in [`src/adapter/claude.rs`](/Users/Laird.Popkin/src/agents-ui/src/adapter/claude.rs) as the current runtime adapter implementation.

Requirements:

- Do not send an empty launch command.
- Do not assume every runtime starts with a resident REPL.
- When adding workers, only send a follow-up worker-loop command if the runtime actually uses one.

This keeps session creation shared while allowing runtime-specific behavior to differ.

### 3. Resolve runtime-specific orchestration commands in the app layer

Runtime-specific dispatch, review, and monitor actions belong in [`src/app.rs`](/Users/Laird.Popkin/src/agents-ui/src/app.rs), because the UI decides when to trigger them.

Current mapping:

- Claude:
  - dispatch: `/autocoder:fix <issue>`
  - review blocked: `/autocoder:review-blocked`
  - monitor workers: `/autocoder:monitor-workers`
- Codex:
  - dispatch: `bash .../codex-fix-loop.sh --issue <issue> --max-iterations 1`
  - review blocked: `bash .../codex-autocoder.sh review-blocked`
  - monitor workers: `bash .../codex-monitor-workers.sh`
- Droid:
  - dispatch: `bash .../droid-fix-loop.sh --issue <issue> --max-iterations 1`
  - review blocked: `bash .../droid-autocoder.sh review-blocked`
  - monitor workers: `bash .../droid-monitor-workers.sh`

Do not reuse Claude slash commands for non-Claude runtimes unless the runtime’s own docs/scripts say that is correct.

### 4. Add a readiness check before launch

Each runtime needs a “can this repo actually run?” check.

Current pattern:

- Droid checks installed plugin assets or repo-local `.factory` assets.
- Codex checks repo-local `scripts/codex-*.sh` wrappers or user-level Codex assets such as `~/.codex/skills/autocoder/SKILL.md`.

If readiness fails:

- Use an installer script from the shared agents repo when available.
- Prefer deterministic local checks over assumptions about user setup.

Installer discovery should search:

- a sibling `agents/scripts/` checkout
- `../agents/scripts/`
- the resolved `agents_dir`
- the root above `plugins/autocoder` when `agents_dir` points at a plugin install

### 5. Only ask for user choices when the runtime actually needs them

The install-scope dialog exists because Droid has meaningful scope differences.

Codex does not currently have a meaningful install-scope split in this app. Its installer sets up both user-level and repo-local assets, so `agents-ui` auto-installs Codex when required instead of presenting a fake scope choice.

### 6. Add focused tests for the runtime contract

Every new runtime should add tests for:

- CLI/runtime selection
- launch/worker-loop invariants
- install/readiness helper behavior
- script discovery helper behavior

Avoid brittle end-to-end TUI tests when pure helper tests cover the runtime contract.

## Consequences

### Positive

- New runtimes can be added with a clear checklist.
- Codex and Droid now behave according to their actual local integrations.
- The app is less likely to regress by accidentally assuming every runtime behaves like Claude.

### Negative

- `src/app.rs` currently contains runtime-specific command resolution logic, which is a step toward a richer runtime adapter abstraction but not the final architecture.
- Installer detection remains filesystem-driven and depends on the shared `agents` repo layout.

## Checklist For Adding Another Runtime

1. Add the runtime to `AgentType` and define session prefix, status directory, and launch model.
2. Verify the runtime’s real commands/scripts/skills in `../agents` before wiring anything.
3. Update runtime selection UI and CLI flags.
4. Add install/readiness detection.
5. Add installer-script discovery if the runtime has an installer.
6. Map worker dispatch, blocked review, and manager monitoring to runtime-native commands.
7. Ensure tmux session creation does not assume an interactive REPL when the runtime is script-driven.
8. Add tests for the new runtime contract.
9. Update docs/README as needed.

## Notes

This ADR documents the path used to add Codex support correctly and to align Droid support with its actual script-driven workflow. It should be treated as the minimum bar for any future runtime integration work.
