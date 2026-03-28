# Test Coverage Report

**Last Full Analysis**: 2026-03-25
**Total Tests**: 56
**Target Coverage**: 80% of pure/testable functions

## Summary

| Priority | Area | Coverage | Files | Last Verified |
|----------|------|----------|-------|---------------|
| P0 | Data Models (status, swarm, issue) | 80% | 3/3 | 2026-03-25 |
| P0 | Config & Persistence | 70% | 1/1 | 2026-03-25 |
| P1 | App Logic (key handling, status) | 50% | 1/1 | 2026-03-25 |
| P1 | GitHub Integration | 90% | 1/1 | 2026-03-25 |
| P1 | Adapter (claude.rs) | 60% | 1/1 | 2026-03-25 |
| P2 | CLI / main.rs | 90% | 1/1 | 2026-03-25 |
| P2 | Transport | 80% | 1/1 | 2026-03-25 |
| P2 | Tmux Session Parsing | 70% | 1/1 | 2026-03-25 |
| P2 | UI Smoke Tests | 40% | 3/6 | 2026-03-25 |
| P3 | Scripts/Launcher | 0% | 0/1 | 2026-03-25 |
| P3 | Theme | 0% | 0/1 | 2026-03-25 |

---

## P0: Critical Priority

### Data Models
<!-- COVERAGE: 80% | FILES: 3/3 | VERIFIED: 2026-03-25 -->

**Coverage**: 80%
**Status**: Good — core parsing and state logic tested
**Last Verified**: 2026-03-25

| File | Tests | Coverage | Notes |
|------|-------|----------|-------|
| `src/model/status.rs` | 3 | 80% | parse_status_line, parse_state, read_status_file tested. Missing: read_json_status_files (new) |
| `src/model/swarm.rs` | 4 | 75% | Runtime-specific launch/loop commands tested. Missing: detect_waiting_for_input |
| `src/model/issue.rs` | 7 | 90% | Blocking, working, priority, filter, display all tested |

### Config & Persistence
<!-- COVERAGE: 70% | FILES: 1/1 | VERIFIED: 2026-03-25 -->

**Coverage**: 70%
**Status**: Core save/load tested
**Last Verified**: 2026-03-25

| File | Tests | Coverage | Notes |
|------|-------|----------|-------|
| `src/config/persistence.rs` | 2 | 70% | repo root finding, save/load round-trip. Missing: list_configs, edge cases |

---

## P1: High Priority

### App Logic
<!-- COVERAGE: 50% | FILES: 1/1 | VERIFIED: 2026-03-25 -->

**Coverage**: 50%
**Status**: Key handling and status inference tested, but large file with many untested paths
**Last Verified**: 2026-03-25

| File | Tests | Coverage | Notes |
|------|-------|----------|-------|
| `src/app.rs` | 5 | 50% | key_event_to_tmux, infer_status_from_pane, extract_issue_from_text tested. Missing: screen state transitions, refresh_statuses, dispatch logic |

### GitHub Integration
<!-- COVERAGE: 90% | FILES: 1/1 | VERIFIED: 2026-03-25 -->

**Coverage**: 90%
**Status**: Well tested
**Last Verified**: 2026-03-25

| File | Tests | Coverage | Notes |
|------|-------|----------|-------|
| `src/github.rs` | 5 | 90% | JSON parsing, error classification all tested |

### Adapter (Claude)
<!-- COVERAGE: 60% | FILES: 1/1 | VERIFIED: 2026-03-25 -->

**Coverage**: 60%
**Status**: Pane state classification tested, async runtime methods need integration tests
**Last Verified**: 2026-03-25

| File | Tests | Coverage | Notes |
|------|-------|----------|-------|
| `src/adapter/claude.rs` | 10 | 60% | Pane detection, bootstrap, branch parsing tested. Missing: launch, discover, add_worker (need tmux) |

---

## P2: Medium Priority

### CLI / Main
<!-- COVERAGE: 90% | FILES: 1/1 | VERIFIED: 2026-03-25 -->

**Coverage**: 90%
**Status**: Well tested
**Last Verified**: 2026-03-25

| File | Tests | Coverage | Notes |
|------|-------|----------|-------|
| `src/main.rs` | 7 | 90% | Flag parsing, runtime selection, conflicts all tested |

### Transport
<!-- COVERAGE: 80% | FILES: 1/1 | VERIFIED: 2026-03-25 -->

**Coverage**: 80%
**Status**: Good
**Last Verified**: 2026-03-25

| File | Tests | Coverage | Notes |
|------|-------|----------|-------|
| `src/transport.rs` | 2 | 80% | Remote command building, transport detection tested |

### Tmux Session Parsing
<!-- COVERAGE: 70% | FILES: 1/1 | VERIFIED: 2026-03-25 -->

**Coverage**: 70%
**Status**: Good
**Last Verified**: 2026-03-25

| File | Tests | Coverage | Notes |
|------|-------|----------|-------|
| `src/tmux/session.rs` | 2 | 70% | Window/pane parsing tested. Missing: edge cases |

### UI Smoke Tests
<!-- COVERAGE: 40% | FILES: 3/6 | VERIFIED: 2026-03-25 -->

**Coverage**: 40%
**Status**: Some views have smoke tests, others missing
**Last Verified**: 2026-03-25

| File | Tests | Coverage | Notes |
|------|-------|----------|-------|
| `src/ui/new_swarm.rs` | 4 | 80% | All dialogs smoke-tested |
| `src/ui/repos_list.rs` | 1 | 60% | Basic render test |
| `src/ui/swarm_view.rs` | 2 | 50% | Render + confirmation detection |
| `src/ui/agent_view.rs` | 0 | 0% | **No tests** — needs smoke test |
| `src/ui/repo_view.rs` | 0 | 0% | **No tests** — needs smoke test |
| `src/ui/theme.rs` | 0 | 0% | Pure style functions, easy to test |

---

## P3: Lower Priority

### Scripts / Launcher
<!-- COVERAGE: 0% | FILES: 0/1 | VERIFIED: 2026-03-25 -->

**Coverage**: 0%
**Status**: No tests — resolve_agents_dir and find_script are testable pure functions
**Last Verified**: 2026-03-25

| File | Tests | Coverage | Notes |
|------|-------|----------|-------|
| `src/scripts/launcher.rs` | 0 | 0% | Path resolution logic, testable with tempdir |

### Untestable (Integration Only)

These files require tmux or terminal I/O and are tested manually:

| File | Reason |
|------|--------|
| `src/tui.rs` | Terminal raw mode init/restore |
| `src/tmux/proxy.rs` | All functions require running tmux |
| `src/event.rs` | Async tokio event channels |

---

## Coverage History

| Date | Tests | Notes |
|------|-------|-------|
| 2026-03-25 | 56 | Initial analysis |

---

*Generated by /improve-test-coverage*
