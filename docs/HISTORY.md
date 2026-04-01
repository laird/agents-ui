# Project History

This file tracks all significant changes, migrations, and decisions.


---

## 2026-04-01 12:19:06 - Fix runtime consistency for swarm launch

**What Changed**: Made swarm launch/discovery/add-worker honor selected AgentType for both manager and workers, including session naming and loop command routing; added runtime parsing tests.

**Why Changed**: atui --droid should not launch mixed runtimes and runtime-prefixed sessions must map back correctly.

**Impact**: Droid/Codex/Gemini swarms now launch and reconnect with matching runtime behavior, reducing manager/worker mismatch risk.


---

## 2026-04-01 12:25:30 - Fix #249 runtime skill auto-install preflight

**What Changed**: Added runtime preflight in ClaudeAdapter launch to auto-run Codex/Droid installer scripts when selected, with installer-path resolution tests.

**Why Changed**: Launching non-Claude runtimes could fail when required skills/wrappers were missing; preflight now installs them before session startup.

**Impact**: Selected runtime launches now validate and install required assets up front, reducing broken workflow starts due to missing skills.

