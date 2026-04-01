# Project History

This file tracks all significant changes, migrations, and decisions.


---

## 2026-04-01 12:17:09 - Fix #249 Codex skill prerequisite detection

**What Changed**: Updated codex repo asset detection to require autocoder SKILL.md plus core wrapper scripts, and added tests for .factory and legacy skills paths.

**Why Changed**: atui could treat Codex as installed when wrappers existed but required skill files were missing, causing workflow failures at runtime.

**Impact**: Codex runtime install checks now prevent false-ready state and prompt installation when skills are missing.

