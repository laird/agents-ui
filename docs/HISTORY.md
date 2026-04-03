# Project History

This file tracks all significant changes, migrations, and decisions.


---

## 2026-04-03 00:44:17 - Autocoder one-pass idle verification

**What Changed**: Read the required autocoder workflow references, confirmed `AGENTS.md`, `plugins/autocoder/scripts/regression-test.sh`, and `scripts/append-to-history.sh` are not vendored in this checkout, queried the live GitHub queue (`gh issue list --repo laird/agents-ui --state open --limit 100`, none open), and ran `cargo test` plus `cargo build` successfully.

**Why Changed**: The required single-pass workflow reached regression verification because there were no unprioritized issues, prioritized bugs, regression-failure issues, approved enhancements, or actionable proposals available in the queue.

**Impact**: Confirmed the repository is idle for autonomous work on this pass with a green baseline of 120 passing tests and a successful build; no code changes, commit, or push were required.


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


---

## 2026-04-01 12:32:37 - Fix #247 runtime mismatch relaunch

**What Changed**: Added runtime fingerprint checks in pane-idle handling and relaunches mismatched manager/worker panes into the expected swarm runtime.

**Why Changed**: --droid sessions could keep a Codex manager pane after reconnect/probe, causing mixed-runtime behavior.

**Impact**: Manager and workers now converge to the selected runtime consistently; added regression tests for runtime hint/mismatch detection.


---

## 2026-04-01 12:37:42 - Fix #238 approve hotkey

**What Changed**: Updated issue-list 'p' handler to run gh issue edit --remove-label proposal on the selected issue and refresh issues after success.

**Why Changed**: Sending 'approve <issue>' to manager panes no longer approved proposal issues in the current Droid workflow.

**Impact**: Pressing 'p' now performs approval directly from the UI across runtimes without relying on manager command support.


---

## 2026-04-01 12:42:35 - Resolve #239 as already implemented

**What Changed**: Validated that Repos List already supports adding a repo and starting a swarm via the n key New Swarm flow, then closed issue #239 with clarification comments.

**Why Changed**: Issue #239 was the highest-priority unblocked P2 bug in queue order, but the behavior exists in current code.

**Impact**: Queue is unblocked for the next P2 bug; no product code changes were required in this pass.


---

## 2026-04-01 12:48:48 - Fix #246 tmux session resize transport

**What Changed**: Updated tmux session resize helpers to execute through ServerTransport and wired all resize call sites to pass transport.

**Why Changed**: Session width syncing previously used local tmux commands, which could miss remote sessions and leave wrapped output.

**Impact**: Terminal resize and session discovery now resize tmux windows consistently in local and remote modes.


---

## 2026-04-01 12:51:52 - Close #246 as resolved

**What Changed**: Verified that issue #246 was already fixed by commit 0a91b66 and closed the GitHub issue.

**Why Changed**: The fix was implemented and pushed but the issue stayed open.

**Impact**: Bug queue now advances to the next unblocked item without duplicate implementation work.


---

## 2026-04-01 12:57:27 - Fix #242 stale worker status detection

**What Changed**: Updated pane state classification and status merging to prefer current idle/shell signals over stale working state, and added regression tests in app/adapter status inference paths.

**Why Changed**: Workers could appear stuck due to stale pane or status-file signals overriding current idle prompts.

**Impact**: Worker state transitions now recover to idle/shell more reliably, improving visibility and reducing manual intervention.


---

## 2026-04-01 13:10:00 - Add project-management backend wrapper

**What Changed**: Added a project-management command wrapper with GitHub/Linear/Jira backends and routed existing issue command construction through it.

**Why Changed**: Issue #245 requires backend abstraction beyond GitHub while keeping current behavior stable.

**Impact**: Project-management calls are now centralized, reducing backend leakage and enabling future auth/field mapping extensions.


---

## 2026-04-01 13:32:24 - Autocoder one-pass queue check

**What Changed**: Reviewed the issue queue and ran regression checks (cargo test, cargo build).

**Why Changed**: To execute one autocoder workflow pass in required order and verify project health before acting.

**Impact**: No actionable unblocked issues were found; only proposal-labeled enhancements remain open (#234-#237).


---

## 2026-04-01 14:10:35 - Autocoder pass: queue triage + regression check

**What Changed**: Reviewed open issue queue by workflow priority, confirmed only proposal-labeled P3 enhancements are open, and ran cargo test + cargo build regression checks.

**Why Changed**: To execute one full autocoder workflow pass and verify no active bug/regression work is pending before proposals.

**Impact**: All tests/build passed; no unblocked actionable issue available without human proposal approval.


---

## 2026-04-01 14:28:18 - Autocoder one-pass queue evaluation

**What Changed**: Reviewed open GitHub issue queue in workflow order, found no unprioritized issues, no P0-P3 bugs, no approved enhancements, and only proposal issues awaiting human approval; ran cargo test and cargo build regression checks (all passing).

**Why Changed**: To complete exactly one autocoder pass and verify there is no autonomous actionable work before proposals are approved.

**Impact**: Queue remains idle for autonomous execution; no code changes were required and existing proposals remain pending human approval.


---

## 2026-04-01 14:50:02 - Issue #235: inline issue-detail comments

**What Changed**: Added Issue Detail comment input state, UI overlay/help hint, and gh issue comment command wiring.

**Why Changed**: To implement approved enhancement #235 so users can post comments without leaving the TUI.

**Impact**: Issue Detail now supports C to draft/post comments with Enter, Esc cancel behavior, and status feedback.


---

## 2026-04-01 14:56:22 - Autocoder one-pass idle check

**What Changed**: Reviewed GitHub issue queue (no open issues) and ran cargo build/cargo test at regression stage.

**Why Changed**: The autocoder workflow requires regression verification when no prioritized bug issues are available.

**Impact**: Confirmed no actionable work items; current branch remains functionally healthy with build/tests passing.


---

## 2026-04-01 17:18:34 - Autocoder pass: regression check

**What Changed**: Checked GitHub issue queue (none open) and ran cargo test + cargo build as regression/build verification

**Why Changed**: Workflow order requires regression validation when no prioritized bugs are available

**Impact**: Confirmed green baseline for one-pass autocoder run; no code changes or issue actions were needed


---

## 2026-04-01 18:27:58 - Autocoder one-pass queue check

**What Changed**: Read autocoder workflow docs, checked issue queues, attempted plugin regression script (failed due REPORT_DIR parse), then ran cargo build && cargo test successfully (120/120).

**Why Changed**: Workflow-ordered one-pass execution required selecting highest-priority unblocked work; no open issues existed, so regression phase was executed.

**Impact**: Validated repository health with no new actionable issues found; no code changes were required.


---

## 2026-04-02 13:18:38 - Autocoder one-pass idle check

**What Changed**: Reviewed the GitHub issue queue in workflow order, confirmed there are no open issues, attempted the shared regression script (it failed immediately on CLAUDE.md report-dir parsing), then ran cargo test and cargo build successfully.

**Why Changed**: To complete exactly one autocoder workflow pass when no triage, bug, regression-failure issue, or approved enhancement work was available.

**Impact**: Confirmed a green baseline for this repository with no actionable unblocked work items; no code changes or issue actions were required.


---

## 2026-04-02 13:35:12 - Autocoder one-pass queue review and regression check

**What Changed**: Read the autocoder workflow references, confirmed AGENTS.md and the expected helper scripts are not vendored in this repo, checked the open GitHub issue queue (none open), and ran cargo test plus cargo build successfully.

**Why Changed**: The workflow order reached the regression stage because there were no unprioritized issues, bugs, regression-failure issues, approved enhancements, or proposals actionable in this worktree.

**Impact**: Validated a green baseline for one exact autocoder pass with no actionable unblocked work items; no product code changes, commit, or push were required.


---

## 2026-04-02 14:08:52 - Autocoder one-pass regression check

**What Changed**: Read the shared autocoder workflow docs, checked the GitHub issue queue with gh, confirmed there are no open actionable issues, and ran cargo test plus cargo build successfully.

**Why Changed**: The workflow order reached regression verification because there were no unprioritized issues, bugs, regression failures, approved enhancements, or actionable proposals in this repository.

**Impact**: Validated a green baseline for this repository for exactly one autocoder pass; no code changes, commit, or push were needed.


---

## 2026-04-02 14:25:57 - Autocoder one-pass queue check

**What Changed**: Read the autocoder workflow references, confirmed `AGENTS.md` plus the shared autocoder helper scripts are not vendored in this checkout, queried the open GitHub issue queue (`gh issue list`, none open), and ran `cargo test` plus `cargo build` successfully.

**Why Changed**: The required one-pass workflow advanced from triage and bug stages to regression verification because there were no unprioritized issues, open bugs, regression-failure issues, approved enhancements, or proposals actionable in this repository.

**Impact**: Confirmed the repository is currently idle for autonomous work with a green regression baseline; no code changes, commit, or push were required in this pass.

---

## 2026-04-02 14:42:19 - Autocoder single-pass regression sweep

**What Changed**: Checked the GitHub issue queue, found no open issues, then ran cargo test and cargo build successfully in the agents-ui repo.

**Why Changed**: The autocoder workflow requires regression verification when no prioritized bugs or approved enhancements are available.

**Impact**: Confirmed the current codebase passes its Rust test and build gates; no actionable issue was available for this pass.

---

## 2026-04-02 14:59:11 - Autocoder single-pass idle verification

**What Changed**: Read the repo autocoder workflow references, confirmed `AGENTS.md` and the shared helper scripts are not vendored in this checkout, checked the GitHub issue queue (`gh issue list`, none open; `gh label list` hit an API connectivity error), and ran `cargo test` plus `cargo build` successfully.

**Why Changed**: The required one-pass workflow reached regression verification because there were no unprioritized issues, prioritized bugs, regression-failure issues, approved enhancements, or proposals actionable in this environment.

**Impact**: Confirmed a green repository baseline for this pass with no actionable unblocked work items; no product code changes, commit, or push were required.

---

## 2026-04-02 15:16:05 - Autocoder one-pass queue check and regression verification

**What Changed**: Checked the GitHub issue queue for open actionable work, found no open issues, then ran cargo test and cargo build as the repo's regression/build verification steps.

**Why Changed**: The autocoder workflow requires regression verification when no prioritized bugs remain; this repo's native checks are Rust cargo commands rather than the legacy JS-oriented regression helper defaults.

**Impact**: Confirmed the current branch has no actionable queue item and the codebase passes 120 tests plus a clean build.

---

## 2026-04-02 15:32:18 - Autocoder one-pass idle queue verification

**What Changed**: Read the required autocoder workflow references, confirmed `AGENTS.md`, `plugins/autocoder/scripts/regression-test.sh`, and `scripts/append-to-history.sh` are not vendored in this checkout, queried the open GitHub issue queue (`gh issue list`, none open), and ran `cargo test` plus `cargo build` successfully.

**Why Changed**: The one-pass workflow had no unprioritized issues, bugs, regression-failure issues, approved enhancements, or actionable proposals, so it advanced to regression verification using the repo's available Rust checks.

**Impact**: Confirmed the repository is idle for autonomous issue work with a green baseline; no product code changes, commit, or push were required in this pass.

---

## 2026-04-02 15:49:28 - Autocoder one-pass idle verification

**What Changed**: Read the required autocoder workflow references, confirmed `AGENTS.md`, `plugins/autocoder/scripts/regression-test.sh`, and `scripts/append-to-history.sh` are not vendored in this checkout, queried the open GitHub issue queue (`gh issue list`, none open; `gh label list` hit an API connectivity error), and ran `cargo test` plus `cargo build` successfully.

**Why Changed**: The one-pass autocoder workflow requires queue inspection in priority order followed by regression verification when there is no unblocked triage, bug, regression-failure, approved enhancement, or proposal work to execute.

**Impact**: Confirmed a green repository baseline for this pass with no actionable unblocked work items; no product code changes, commit, or push were required.

---

## 2026-04-02 16:05:57 - Autocoder one-pass idle verification

**What Changed**: Read the required autocoder workflow references, used `CLAUDE.md` only as legacy fallback because `AGENTS.md` is not present in this checkout, confirmed the shared helper scripts (`plugins/autocoder/scripts/regression-test.sh` and `scripts/append-to-history.sh`) are not vendored here, checked the live GitHub queue (`gh issue list`, none open), and ran `cargo test` plus `cargo build` successfully.

**Why Changed**: The workflow order reached regression verification because there were no unprioritized issues, prioritized bugs, regression failures, approved enhancements, or actionable proposals available for a single-pass autocoder run.

**Impact**: Confirmed the repository is idle for autonomous work on branch `fix/issue-132-auto` with a green baseline; no code changes, commit, or push were required in this pass.

---

## 2026-04-02 16:39:28 - Autocoder single-pass queue check

**What Changed**: Checked the GitHub issue queue, found no open issues, and ran the configured regression verification with cargo test and cargo build.

**Why Changed**: The autocoder workflow requires regression verification when no prioritized bug or enhancement work is available.

**Impact**: Confirmed the current branch is clean from a workflow perspective: no queue items were available and the Rust test/build gates passed without creating new follow-up work.

---

## 2026-04-02 16:55:50 - Autocoder one-pass idle check

**What Changed**: Read the required autocoder workflow references, used `CLAUDE.md` as the documented legacy fallback because `AGENTS.md` is not present in this checkout, confirmed the shared helper scripts (`plugins/autocoder/scripts/regression-test.sh` and `scripts/append-to-history.sh`) are not vendored here, queried the live GitHub queue (`gh issue list`, none open; `gh issue status` hit an API connectivity error), and ran `cargo test` plus `cargo build` successfully.

**Why Changed**: The one-pass workflow had no unprioritized issues, bugs, regression-failure items, approved enhancements, or actionable proposals, so it advanced to regression verification using the repo's available Rust checks.

**Impact**: Confirmed a green baseline for exactly one autocoder pass with no actionable unblocked work items; no product code changes, commit, or push were needed.

---

## 2026-04-03 07:24:56 - Autocoder one-pass idle verification

**What Changed**: Read the required autocoder workflow references, used `CLAUDE.md` as the documented legacy fallback because `AGENTS.md` is not present in this checkout, confirmed the shared helper scripts (`plugins/autocoder/scripts/regression-test.sh` and `scripts/append-to-history.sh`) are not vendored here, queried the live GitHub queue (`gh issue list --repo laird/agents-ui --state open --limit 100`, none open), and ran `cargo test` plus `cargo build` successfully.

**Why Changed**: The workflow order for exactly one autocoder pass reached regression verification because there were no unprioritized issues, prioritized bugs, regression-failure issues, approved enhancements, or actionable proposals available in the queue.

**Impact**: Confirmed a green baseline for exactly one autocoder pass with no actionable unblocked work; no product code changes, commit, or push were required.

---

## 2026-04-02 17:12:58 - Autocoder one-pass idle verification

**What Changed**: Read the required autocoder workflow references, used `CLAUDE.md` only as legacy fallback because `AGENTS.md` is not present in this checkout, confirmed `plugins/autocoder/scripts/regression-test.sh` and `scripts/append-to-history.sh` are not vendored here, queried the live GitHub queue (`gh issue list --repo laird/agents-ui --state open --limit 100`, none open), and ran `cargo test` plus `cargo build` successfully.

**Why Changed**: The workflow order for exactly one autocoder pass reached regression verification because there were no unprioritized issues, open bugs, regression-failure issues, approved enhancements, or actionable proposals available in the queue.

**Impact**: Confirmed this branch remains idle for autonomous issue work with a green baseline of 120 passing tests and a successful build; no code changes, commit, or push were required.

---

## 2026-04-02 17:29:45 - Autocoder one-pass idle verification

**What Changed**: Read the required autocoder workflow references, used `CLAUDE.md` only as legacy fallback because `AGENTS.md` is not present in this checkout, confirmed `plugins/autocoder/scripts/regression-test.sh` and `scripts/append-to-history.sh` are not vendored here, queried the live GitHub queue (`gh issue list --repo laird/agents-ui --state open --limit 100`, none open), and ran `cargo test` plus `cargo build` successfully.

**Why Changed**: The one-pass workflow had no unprioritized issues, open bugs, regression-failure issues, approved enhancements, or actionable proposals, so it advanced to regression verification using the repo's available Rust checks.

**Impact**: Confirmed a green baseline for exactly one autocoder pass with no actionable unblocked work; no product code changes, commit, or push were required.

---

## 2026-04-02 22:16:19 - Autocoder one-pass idle verification

**What Changed**: Read the required autocoder workflow references, used CLAUDE.md as the documented legacy fallback because AGENTS.md is not present in this checkout, confirmed the live GitHub queue is empty with gh issue list, attempted the shared regression helper (it failed immediately on legacy CLAUDE.md report-dir parsing via mkdir), then ran cargo test and cargo build successfully.

**Why Changed**: The one-pass workflow had no unprioritized issues, bugs, regression-failure items, approved enhancements, or actionable proposals, so it advanced to regression verification using the repo's available Rust checks.

**Impact**: Confirmed a green baseline for exactly one autocoder pass with no actionable unblocked work items; no product code changes, commit, or push were needed.
