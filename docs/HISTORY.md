# Project History

This file tracks all significant changes, migrations, and decisions.


---

## 2026-04-01 11:58:38 - Fix #250 worker status refresh

**What Changed**: Updated worker status refresh to keep fix-loop.status authoritative and added merge tests in src/app.rs.

**Why Changed**: Pane inference was overriding status-file state, making Droid fix-loop progress appear incorrect.

**Impact**: Repo view now reflects worker loop state reliably when status files exist, reducing false idle/working transitions.


---

## 2026-04-01 12:04:18 - Triage issue #249 priority

**What Changed**: Assigned P1 label and added triage rationale comment on GitHub issue #249.

**Why Changed**: Workflow order requires triaging unprioritized issues before bug-fix execution.

**Impact**: Issue #249 is now prioritized for upcoming fix passes, reducing queue ambiguity for workers.


---

## 2026-04-01 12:14:35 - Fix #243 Codex loop supervision

**What Changed**: Routed Codex manager/workers to shell loop wrappers, added Codex drift recovery in pane supervision, and restored immediate passthrough keystrokes in Agent View.

**Why Changed**: Codex swarms were drifting into interactive prompts and losing loop ownership; passthrough keystrokes also lagged until Enter.

**Impact**: Codex sessions stay under wrapper supervision and Agent View input now reaches live sessions immediately.


---

## 2026-04-01 12:24:36 - Fix #249 Codex skill asset validation

**What Changed**: Updated Codex repo readiness checks to require autocoder SKILL.md alongside wrapper scripts, and added tests for both .factory and legacy skills paths.

**Why Changed**: Codex launch readiness produced false positives when wrappers existed but runtime skills were missing, causing broken starts.

**Impact**: Codex runtime setup now triggers install flow when skills are missing, reducing launch failures and ensuring consistent startup behavior.


---

## 2026-04-01 12:29:05 - Fix #248 monitor-workers assets

**What Changed**: Updated runtime asset checks so Codex requires codex-monitor-workers.sh and Droid requires monitor-workers command availability; added regression tests for both.

**Why Changed**: Issue #248 reported monitor-workers workflow failures when required autocoder assets were missing but installation checks passed.

**Impact**: atui now detects incomplete runtime installs earlier and prompts installer fallback, improving manager monitor-workers reliability across Codex/Droid runtimes.


---

## 2026-04-01 12:36:47 - Fix #238 approve action removes proposal label

**What Changed**: Updated the Issues-panel 'p' action in src/app.rs to execute gh issue edit <issue> --remove-label proposal directly and refresh issue data on success.

**Why Changed**: The previous implementation sent a runtime text command ('approve <issue>') that did not reliably remove the proposal label.

**Impact**: Pressing p now performs approval consistently across runtimes and gives clear success/failure status in the UI.


---

## 2026-04-01 12:40:54 - Fix #240 Codex session commands

**What Changed**: Updated Codex command generation in src/app.rs and src/adapter/claude.rs to send 'use autocoder to fix <issue>' and 'use autocoder to monitor-workers', and adjusted adapter unit tests.

**Why Changed**: Codex sessions do not handle the slash-command forms used by other runtimes, so dispatch/monitor commands were ineffective.

**Impact**: Codex swarms can now receive compatible fix and monitor-workers instructions from the TUI, improving issue dispatch reliability.


---

## 2026-04-01 12:52:46 - Fix #242 stale worker status detection

**What Changed**: Updated pane/status merging to prefer explicit idle prompts and shell states over stale working signals, added idle-prompt-first pane classification, and expanded regression tests for stale-history cases.

**Why Changed**: Workers could appear stuck because older 'working' text or stale status-file state overshadowed a current idle prompt, delaying recovery and obscuring true state.

**Impact**: Worker state now converges to idle/shell more reliably, clearing stale dispatch assignments and improving automatic recovery visibility.


---

## 2026-04-01 12:55:36 - Fix #246 tmux width sync

**What Changed**: Added tmux session resize calls in launch_with_progress and add_worker in src/adapter/claude.rs.

**Why Changed**: Issue #246 reported wrapped lines when session/window widths stayed at stale defaults during launch/manage flows.

**Impact**: New swarms and newly added worker windows now proactively resize to current terminal dimensions, improving pane readability.


---

## 2026-04-01 13:03:42 - Fix #244: wrap project-management gh calls

**What Changed**: Added src/project_management.rs wrapper and routed app/github/issue gh issue operations through it.

**Why Changed**: Centralizes gh command construction, execution context, and error handling behind one boundary for project-management workflows.

**Impact**: Improves maintainability and prepares backend abstraction work while preserving existing behavior.


---

## 2026-04-01 13:04:03 - Fix #244: centralize project-management gh calls

**What Changed**: Added src/project_management.rs and routed app/model/github issue operations through that wrapper for list/view/create/edit/auth flows.

**Why Changed**: This centralizes GitHub command construction, logging, and error-detail handling in one boundary for the project-management layer.

**Impact**: Behavior remains equivalent while making future backend/policy extensions easier.


---

## 2026-04-01 13:10:30 - Issue #245 backend abstraction

**What Changed**: Extended src/project_management.rs with a backend abstraction that supports GitHub, Linear, and Jira command generation while keeping legacy GitHub behavior.

**Why Changed**: Issue #245 requires project-management wrappers to be backend-agnostic and ready for non-GitHub integrations.

**Impact**: Core workflow remains GitHub-compatible, and backend-specific wrappers now exist for future Linear/Jira integration.


---

## 2026-04-01 13:33:13 - Approve #237 proposal

**What Changed**: Removed the proposal label from GitHub issue #237 and posted an approval comment using the autocoder approval script.

**Why Changed**: Queue review found no bugs/regressions; next actionable item was a proposal requiring approval to become implementable.

**Impact**: Issue #237 is now an approved enhancement (P3) ready for implementation in a future fix pass.


---

## 2026-04-01 13:38:09 - Implement #237 issue-detail clipboard shortcut

**What Changed**: Added Issue Detail key handling for 'c' to copy #<issue> to clipboard with pbcopy/wl-copy/xclip/xsel fallbacks, updated help bar text, and added regression tests.

**Why Changed**: Issue #237 was the highest-priority unblocked approved enhancement in the queue.

**Impact**: Issue detail navigation now supports faster issue-number reuse in terminal workflows, with graceful fallback when no clipboard utility exists.

