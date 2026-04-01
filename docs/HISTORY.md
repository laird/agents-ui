# Project History

This file tracks all significant changes, migrations, and decisions.


---

## 2026-04-01 12:19:06 - Fix runtime consistency for swarm launch

**What Changed**: Made swarm launch/discovery/add-worker honor selected AgentType for both manager and workers, including session naming and loop command routing; added runtime parsing tests.

**Why Changed**: atui --droid should not launch mixed runtimes and runtime-prefixed sessions must map back correctly.

**Impact**: Droid/Codex/Gemini swarms now launch and reconnect with matching runtime behavior, reducing manager/worker mismatch risk.

