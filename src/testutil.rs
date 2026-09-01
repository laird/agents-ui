//! Test-only helpers shared across modules.

/// The process-wide lock for tests that mutate environment variables.
///
/// It has to be process-wide, not per-module. Tests run as threads of one
/// binary, so a test in `adapter::supervisor` that sets and then removes
/// `AGENTS_DIR` will pull it out from under a test in `adapter::claude` that
/// is relying on it -- which is exactly what happened: the launch test passed
/// locally and on macOS and failed on Linux CI, purely on thread timing.
///
/// Poisoning is tolerated on purpose. This guards environment variables, not
/// data whose invariants a panic could break, and a poisoned lock turns one
/// real failure into a cascade of unrelated PoisonErrors that bury it.
pub fn env_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| std::sync::Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

use std::path::{Path, PathBuf};

/// Every test artifact starts with this, so a sweep can find them all and a
/// human can tell at a glance what left them behind.
pub const ARTIFACT_PREFIX: &str = "agents-ui-test";

fn artifact_root() -> PathBuf {
    std::env::temp_dir()
}

/// `agents-ui-test-<pid>-<label>-<nanos>` — the pid sits in a fixed position so
/// the reaper can ask whether the run that created this is still alive.
pub fn artifact_name(label: &str) -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!(
        "{ARTIFACT_PREFIX}-{}-{label}-{nanos}",
        std::process::id()
    )
}

/// A scratch directory that is removed even if the test panics.
///
/// Cleanup written as the last statement of a test is skipped by every panic,
/// and a panic is precisely how these tests fail -- so the failing runs, the
/// ones you repeat most, were the ones that leaked.
pub struct TempTree {
    path: PathBuf,
}

impl TempTree {
    pub fn new(label: &str) -> Self {
        let path = artifact_root().join(artifact_name(label));
        std::fs::create_dir_all(&path).expect("create temp tree");
        TempTree { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempTree {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.path).ok();
    }
}

/// True when a process with this id is still running.
fn pid_alive(pid: u32) -> bool {
    // `kill -0` is the portable liveness check: /proc does not exist on macOS.
    std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// The pid embedded in `agents-ui-test-<pid>-...`, if any.
fn owner_pid(name: &str) -> Option<u32> {
    name.strip_prefix(ARTIFACT_PREFIX)?
        .trim_start_matches('-')
        .split('-')
        .next()?
        .parse()
        .ok()
}

/// Remove artifacts left by runs that are no longer alive.
///
/// RAII and end-of-test cleanup both need the process to survive; a test run
/// killed by Ctrl+C, a harness timeout, or a CI cancellation leaks everything
/// it held. Something has to collect that, and the next run is the only thing
/// guaranteed to happen. Runs once per test binary.
pub fn reap_stale_artifacts() {
    static DONE: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    DONE.get_or_init(|| {
        reap_stale_temp_dirs();
        reap_stale_tmux_servers();
        reap_stale_tmux_sessions();
    });
}

fn reap_stale_temp_dirs() {
    let Ok(entries) = std::fs::read_dir(artifact_root()) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.starts_with(ARTIFACT_PREFIX) {
            continue;
        }
        if owner_pid(&name).is_some_and(pid_alive) {
            continue;
        }
        std::fs::remove_dir_all(entry.path()).ok();
    }
}

/// Private tmux servers are addressed by socket file; killing the server is
/// what stops the process, and unlinking alone would leave it running.
fn reap_stale_tmux_servers() {
    let uid = std::process::Command::new("id")
        .arg("-u")
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();
    if uid.is_empty() {
        return;
    }
    let socket_dir = artifact_root().join(format!("tmux-{uid}"));
    let Ok(entries) = std::fs::read_dir(&socket_dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.starts_with(ARTIFACT_PREFIX) {
            continue;
        }
        if owner_pid(&name).is_some_and(pid_alive) {
            continue;
        }
        std::process::Command::new("tmux")
            .args(["-S"])
            .arg(entry.path())
            .arg("kill-server")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .ok();
        std::fs::remove_file(entry.path()).ok();
    }
}

/// Sessions on the DEFAULT server are the dangerous leak: that is where the
/// user's real swarms live, and where `web::discovery` looks for them.
fn reap_stale_tmux_sessions() {
    let Ok(output) = std::process::Command::new("tmux")
        .args(["list-sessions", "-F", "#{session_name}"])
        .output()
    else {
        return;
    };
    for name in String::from_utf8_lossy(&output.stdout).lines() {
        let name = name.trim();
        // Test sessions are named after a temp repo, so the marker can appear
        // anywhere in the name -- `claude-agents-ui-test-123-launch-456`.
        if !name.contains(ARTIFACT_PREFIX) {
            continue;
        }
        let tail = &name[name.find(ARTIFACT_PREFIX).unwrap_or(0)..];
        if owner_pid(tail).is_some_and(pid_alive) {
            continue;
        }
        std::process::Command::new("tmux")
            .args(["kill-session", "-t", name])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .ok();
    }
}
