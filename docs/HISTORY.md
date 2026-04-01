# Project History

This file tracks all significant changes, migrations, and decisions.


---

## 2026-04-01 12:48:59 - Fix #246 tmux session width sync with terminal size

**What Changed**: Added tmux window resizing support, wired terminal resize events through the app event loop, and applied session sizing on startup, session creation, and worker-add flows.

**Why Changed**: Issue #246 reported wrapped output caused by tmux windows not matching the active terminal size.

**Impact**: Manager and worker panes now stay aligned with terminal dimensions, reducing unnecessary line wrapping in captured output.


---

## 2026-04-01 12:19:06 - Fix runtime consistency for swarm launch

**What Changed**: Made swarm launch/discovery/add-worker honor selected AgentType for both manager and workers, including session naming and loop command routing; added runtime parsing tests.

**Why Changed**: atui --droid should not launch mixed runtimes and runtime-prefixed sessions must map back correctly.

**Impact**: Droid/Codex/Gemini swarms now launch and reconnect with matching runtime behavior, reducing manager/worker mismatch risk.


---

## 2026-04-01 12:25:30 - Fix #249 runtime skill auto-install preflight

**What Changed**: Added runtime preflight in ClaudeAdapter launch to auto-run Codex/Droid installer scripts when selected, with installer-path resolution tests.

**Why Changed**: Launching non-Claude runtimes could fail when required skills/wrappers were missing; preflight now installs them before session startup.

**Impact**: Selected runtime launches now validate and install required assets up front, reducing broken workflow starts due to missing skills.


---

## 2026-04-01 12:30:39 - Fix #248 runtime-specific monitor-workers command

**What Changed**: Added AgentType::monitor_workers_cmd and switched auto-dispatch to use runtime-specific monitor command; added unit test coverage for loop/monitor command mapping.

**Why Changed**: Claude runtime requires autocoder-prefixed monitor command while other runtimes use the generic command; hardcoding /monitor-workers broke Claude manager monitoring.

**Impact**: Idle-worker auto-dispatch now invokes the correct command per runtime, restoring monitor-workers workflow reliability.


---

## 2026-04-01 12:36:49 - Fix #238 approve key removes proposal label

**What Changed**: Added p-key handlers in Issue List and Issue Detail to call gh issue edit --remove-label proposal, update local label state, and show status/help text for approval action.

**Why Changed**: Issue #238 reported that approving with p did not work; approval should remove the proposal label.

**Impact**: Users can now approve proposal issues directly from the TUI, with immediate feedback and updated labels.


---

## 2026-04-01 12:42:10 - Fix #240 Codex session command translation

**What Changed**: Updated runtime command mapping so Codex receives 'use autocoder to ...' for monitor-workers/fix-loop, added Codex slash-command normalization for direct input, and surfaced runtime-specific worker-loop command text in status messages.

**Why Changed**: Issue #240 reported that commands sent from the UI did not execute in Codex because Claude-style slash syntax was being sent directly.

**Impact**: Codex manager/worker sessions now receive executable autocoder prompts (including fix issue commands), improving cross-runtime workflow reliability.


---

## 2026-04-01 12:55:20 - Fix #241: add manual refresh keys

**What Changed**: Added manual r refresh in Repo View worker pane, expanded Repos List refresh to re-scan available repos and statuses, and updated Repo View key hints.

**Why Changed**: Issue #241 requested lightweight refresh actions directly from worker/task/repo views without navigation churn.

**Impact**: Users can now refresh repo and worker/task state in-place via r, reducing stale UI state and extra navigation.


---

## 2026-04-01 13:03:23 - Fix #244: add project-management GitHub wrapper

**What Changed**: Added a project_management::github wrapper module and routed issue list/view/edit plus browser-open actions in App through it; added parsing/error-handling tests for issue detail mapping.

**Why Changed**: Issue #244 requires centralizing project-management gh command construction and execution behind a dedicated boundary for future backend expansion.

**Impact**: GitHub operations are now centralized in one wrapper API, reducing duplicated command logic and making future policy/observability changes easier.

