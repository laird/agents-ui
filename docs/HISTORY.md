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

