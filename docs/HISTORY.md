# Project History

This file tracks all significant changes, migrations, and decisions.


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

