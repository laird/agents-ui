# agents-ui

A Rust TUI for launching, monitoring, and managing swarms of AI agents working on software repositories.

## Build & Test

```bash
cargo build --release
cargo test
```

## Deploy

```bash
cargo build --release && cargo install --path .
```

## Automated Testing & Issue Management

This section configures the `/fix` command for autonomous issue resolution.

### Regression Test Suite
```bash
cargo test 2>&1
```

### Build Verification
```bash
cargo build --release 2>&1
```

### Test Framework Details

**Unit Tests**:
- Framework: Rust built-in test framework
- Location: Inline `#[cfg(test)]` modules in source files

**Test Reports**:
- Location: `docs/test/regression-reports/`
