# Project History

This file tracks all significant changes, migrations, and decisions.


---

## 2026-04-01 12:24:04 - Fix #249 codex runtime skill checks

**What Changed**: Updated Codex runtime asset detection to require the autocoder skill when relying on repo wrappers, and added regression tests for the gating logic.

**Why Changed**: The previous readiness check could treat repo wrappers as sufficient and skip required skill installation.

**Impact**: Codex runtime setup now correctly triggers installation when skills are missing, reducing workflow failures from missing runtime skills.


---

## 2026-04-01 12:27:43 - Fix #243 codex loop supervision

**What Changed**: Cherry-picked commit 95af0b9 onto fix/issue-132-auto to keep Codex manager/workers under loop wrappers and restore direct key passthrough in Agent View.

**Why Changed**: Issue #243 was the highest-priority unblocked P1 bug in the queue and already had a validated implementation available on worker-2.

**Impact**: Codex sessions are now relaunched under loop wrappers when drift is detected, reducing stalled autonomous workflows.


---

## 2026-04-01 12:32:37 - Fix #247 runtime mismatch relaunch

**What Changed**: Added runtime fingerprint checks in pane-idle handling and relaunches mismatched manager/worker panes into the expected swarm runtime.

**Why Changed**: --droid sessions could keep a Codex manager pane after reconnect/probe, causing mixed-runtime behavior.

**Impact**: Manager and workers now converge to the selected runtime consistently; added regression tests for runtime hint/mismatch detection.


---

## 2026-04-01 12:37:42 - Fix #238 approve hotkey

**What Changed**: Updated issue-list 'p' handler to run gh issue edit --remove-label proposal on the selected issue and refresh issues after success.

**Why Changed**: Sending 'approve <issue>' to manager panes no longer approved proposal issues in the current Droid workflow.

**Impact**: Pressing 'p' now performs approval directly from the UI across runtimes without relying on manager command support.


---

## 2026-04-01 12:42:35 - Resolve #239 as already implemented

**What Changed**: Validated that Repos List already supports adding a repo and starting a swarm via the n key New Swarm flow, then closed issue #239 with clarification comments.

**Why Changed**: Issue #239 was the highest-priority unblocked P2 bug in queue order, but the behavior exists in current code.

**Impact**: Queue is unblocked for the next P2 bug; no product code changes were required in this pass.

