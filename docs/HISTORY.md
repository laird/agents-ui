# Project History

This file tracks all significant changes, migrations, and decisions.


---

## 2026-04-01 12:17:09 - Fix #249 Codex skill prerequisite detection

**What Changed**: Updated codex repo asset detection to require autocoder SKILL.md plus core wrapper scripts, and added tests for .factory and legacy skills paths.

**Why Changed**: atui could treat Codex as installed when wrappers existed but required skill files were missing, causing workflow failures at runtime.

**Impact**: Codex runtime install checks now prevent false-ready state and prompt installation when skills are missing.


---

## 2026-04-01 12:25:08 - Fix #247 runtime drift relaunch

**What Changed**: Added Codex-runtime drift detection in pane supervision and forced relaunch with expected runtime; added regression test for drift detection.

**Why Changed**: Prevent mixed-runtime swarms where a non-Codex session reconnects to a Codex manager pane.

**Impact**: Improves runtime consistency during session reconnects and reduces manager/worker runtime mismatches.


---

## 2026-04-01 12:30:19 - Fix #243 Codex loop supervision

**What Changed**: Codex manager/workers now launch via shell loop wrappers, Codex monitor/fix prompt injection from App was disabled, and Agent View key passthrough is direct to tmux.

**Why Changed**: Codex panes could drift into interactive prompts and escape loop supervision, which broke monitor-workers dispatch and input behavior.

**Impact**: Codex swarms remain under loop control with automatic relaunch on drift; session input behavior is consistent and issue dispatch remains managed by loop scripts.


---

## 2026-04-01 12:37:02 - Fix #238 approval hotkey

**What Changed**: Changed issue-panel 'p' handling to directly remove the proposal label from the selected issue via gh issue edit and refresh issue cache.

**Why Changed**: The previous behavior sent a manager command that did not reliably approve proposals.

**Impact**: Pressing 'p' now performs approval in-app with clear success/failure status messaging.


---

## 2026-04-01 12:42:46 - Fix #240 codex command phrases

**What Changed**: Updated Codex command strings for issue dispatch, monitor-workers, and review-blocked flows to use plain-language autocoder prompts; aligned adapter tests.

**Why Changed**: Codex sessions do not reliably support slash commands used by other runtimes.

**Impact**: Codex-targeted commands now execute with runtime-compatible phrasing and dispatch behavior is validated by unit tests.


---

## 2026-04-01 12:50:59 - Fix #246: tmux width follows terminal

**What Changed**: Updated tmux session resizing to run through ServerTransport, enforce manual window sizing, and resize sessions before/after launch and on terminal resize events.

**Why Changed**: Tmux sessions were sometimes left at default width (especially outside local direct tmux execution), causing wrapped output in agent panes.

**Impact**: Pane/session widths now consistently track the active terminal width for new, discovered, and live sessions.


---

## 2026-04-01 12:57:08 - Fix #241 manual refresh hotkeys

**What Changed**: Bound r to refresh repos list, workers list, and issues list; moved review-blocked to uppercase R in issues pane; updated swarm help text.

**Why Changed**: Issue #241 requested lightweight manual refresh from each pane without navigation.

**Impact**: Users can refresh the active list/status in place, improving responsiveness during triage and swarm monitoring.


---

## 2026-04-01 13:07:51 - Fix #245: add project-management backend abstraction

**What Changed**: Added src/project_management.rs with GitHub/Linear/Jira issue adapters, backend resolution via .agents-ui.toml/env, and wired app/main issue fetch/auth through the new wrapper.

**Why Changed**: Enable non-GitHub project-management backends without changing existing GitHub-first runtime behavior.

**Impact**: Issue polling/auth now support Linear and Jira credentials while default GitHub workflow remains intact.

