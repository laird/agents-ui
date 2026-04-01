# Project History

This file tracks all significant changes, migrations, and decisions.


---

## 2026-04-01 12:24:04 - Fix #249 codex runtime skill checks

**What Changed**: Updated Codex runtime asset detection to require the autocoder skill when relying on repo wrappers, and added regression tests for the gating logic.

**Why Changed**: The previous readiness check could treat repo wrappers as sufficient and skip required skill installation.

**Impact**: Codex runtime setup now correctly triggers installation when skills are missing, reducing workflow failures from missing runtime skills.

