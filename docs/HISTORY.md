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

