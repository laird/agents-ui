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

