# Project History

This file tracks all significant changes, migrations, and decisions.


---

## 2026-04-01 11:58:38 - Fix #250 worker status refresh

**What Changed**: Updated worker status refresh to keep fix-loop.status authoritative and added merge tests in src/app.rs.

**Why Changed**: Pane inference was overriding status-file state, making Droid fix-loop progress appear incorrect.

**Impact**: Repo view now reflects worker loop state reliably when status files exist, reducing false idle/working transitions.

