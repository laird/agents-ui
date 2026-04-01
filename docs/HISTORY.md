# Project History

This file tracks all significant changes, migrations, and decisions.


---

## 2026-04-01 12:18:14 - Fix #247 runtime consistency for --droid

**What Changed**: Added CLI runtime flag parsing and made swarm launch/discovery session handling runtime-aware so manager and workers use the same selected agent runtime.

**Why Changed**: Issue #247 reported mixed runtime startup; hardcoded Claude session/type handling caused mismatches.

**Impact**: New swarms now honor --droid/--codex/--gemini selection end-to-end and status discovery reads the correct runtime loop directories.

