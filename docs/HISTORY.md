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

