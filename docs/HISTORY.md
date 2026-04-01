# Project History

This file tracks all significant changes, migrations, and decisions.


---

## 2026-04-01 12:18:14 - Fix #247 runtime consistency for --droid

**What Changed**: Added CLI runtime flag parsing and made swarm launch/discovery session handling runtime-aware so manager and workers use the same selected agent runtime.

**Why Changed**: Issue #247 reported mixed runtime startup; hardcoded Claude session/type handling caused mismatches.

**Impact**: New swarms now honor --droid/--codex/--gemini selection end-to-end and status discovery reads the correct runtime loop directories.


---

## 2026-04-01 12:28:20 - Fix #249 runtime skill bootstrap

**What Changed**: Added runtime skill auto-install checks that copy missing Codex/Droid autocoder skill files into launch repos and existing worker worktrees before/after swarm startup.

**Why Changed**: Selected runtimes could launch without required skill files, causing runtime-specific autocoder workflows to fail.

**Impact**: Swarm startup now verifies and bootstraps required skill assets automatically, reducing launch-time runtime failures.


---

## 2026-04-01 12:39:26 - Fix #240 codex command translation

**What Changed**: Added runtime-aware input normalization so Codex sessions convert legacy autocoder slash commands (for example /monitor-workers and /fix 240) into 'use autocoder to ...' text before tmux send-keys, and added unit tests.

**Why Changed**: Codex sessions did not reliably handle legacy slash command syntax, so manager/worker commands could fail to execute.

**Impact**: Codex-directed commands now execute in a compatible format while Claude/Droid behavior remains unchanged.


---

## 2026-04-01 12:47:08 - Fix #242: surface stalled worker states

**What Changed**: Added stale-status detection for working/starting workers, surfaced stalled states in UI styling, and counted stalled/unknown/stopped workers in attention indicators.

**Why Changed**: Workers can appear stuck with no obvious status changes; this makes stalled workers explicit and visible in the repo attention summary.

**Impact**: Operators can detect stalled workers faster and recover without waiting for manual deep inspection.


---

## 2026-04-01 12:53:33 - Fix #246 tmux session width sync

**What Changed**: Updated Claude adapter to resize tmux session windows to current terminal dimensions when connecting to sessions, with terminal-size parsing safeguards and tests.

**Why Changed**: tmux sessions launched without an attached client were defaulting to narrow widths and wrapping output.

**Impact**: Agent and manager panes now align with terminal width during launch/discovery, improving readability in monitoring views.


---

## 2026-04-01 13:00:05 - Fix #244 project-management GitHub wrapper

**What Changed**: Added a dedicated src/project_management.rs GitHub CLI wrapper for issue creation and routed feedback submission through it instead of ad hoc gh invocation in app.rs.

**Why Changed**: Issue #244 requires centralizing project-management GitHub operations for command construction, auth handling, and logging consistency.

**Impact**: GitHub issue creation behavior stays equivalent while providing a single extension point for future policy and provider support.


---

## 2026-04-01 13:24:20 - Autocoder pass: queue triage + regression check

**What Changed**: Ran one Droid autocoder workflow pass; triaged open issues and validated regression state with cargo test and cargo build --release.

**Why Changed**: Workflow requires bugs/regressions/approved enhancements before proposals.

**Impact**: No unblocked actionable work exists (only P3 proposal issues awaiting human approval), so agent exited idle.


---

## 2026-04-01 14:00:45 - Autocoder pass: triage + regression check

**What Changed**: Read available Droid autocoder workflow references, triaged the GitHub queue, and ran cargo test plus cargo build --release with all checks passing.

**Why Changed**: Workflow priority requires triage and regression verification before considering enhancements; only proposal-labeled P3 enhancements remain and are not approved for implementation.

**Impact**: Confirmed no unblocked actionable issues for this pass; repository remains stable and ready for human proposal approval or new bug intake.


---

## 2026-04-01 14:19:27 - Autocoder pass: queue check + validation

**What Changed**: Reviewed Droid autocoder workflow references, evaluated the issue queue for unblocked approved work, and validated repository health with cargo test and cargo build --release.

**Why Changed**: Workflow order requires triage and regression verification before enhancements/proposals; all remaining open issues are proposals or marked working.

**Impact**: Confirmed no unblocked actionable issue for this pass and preserved a verified-green baseline.

