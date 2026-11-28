//! The `agents-ui://` link handler (#576, laird/agents-ui#549 slice c):
//! `agents-tui open <url>` opens a village agent's pane on this desktop,
//! and `agents-tui handler install` registers this binary as the system's
//! handler for that scheme: a `.desktop` file on Linux ([`install`]), a
//! small handler app on macOS ([`macos`], #582). Windows is future work.
//!
//! Every field from the link is validated before anything runs ([`url`]):
//! `host` must match this machine, a saved herdr machine, or a tailnet peer
//! ([`host`]); `pane` must match its mux's own target shape. Every command
//! built from it ([`plan`]) passes arguments as separate argv entries,
//! never through a shell string, so even a validation gap could not inject
//! a shell command.
//!
//! `herdr agent focus` moves a single, server-wide "last focused" pointer
//! shared by every attached client -- confirmed live against a running
//! herdr 0.9.5 server (see the finding on laird/agents-ui#576), not scoped
//! to the caller. `open` never calls it: both the local and the remote
//! branch instead open a fresh terminal attached to just the target pane
//! (`herdr agent attach`, or the same direct SSH exec its tmux branch
//! already used), so a click here can never move a client someone else is
//! looking at.

mod host;
mod install;
mod macos;
mod plan;
mod terminal;
mod url;

use std::io::Write;

const OPEN_USAGE: &str = "usage: agents-tui open <agents-ui://attach?host=H&mux=herdr|tmux&pane=P[&project=NAME&role=ROLE]>";
const INSTALL_USAGE: &str = "usage: agents-tui handler install [--dry-run]";

/// `agents-tui open <url>`: run it, print to `out`, and return the exit code.
pub fn run_open(args: &[String], out: &mut dyn Write) -> i32 {
    let Some(link) = args.first() else {
        let _ = writeln!(out, "{OPEN_USAGE}");
        return 2;
    };
    let parsed = match url::parse(link) {
        Ok(p) => p,
        Err(e) => {
            let _ = writeln!(out, "agents-tui open: {e}");
            return 2;
        }
    };
    match open(&parsed) {
        Ok(()) => 0,
        Err(e) => {
            let _ = writeln!(out, "agents-tui open: {e}");
            1
        }
    }
}

fn open(link: &url::AttachUrl) -> Result<(), String> {
    let local_names = local_hostnames();
    let known_hosts = known_remote_hosts();
    let kind = host::classify(&link.host, &local_names, &known_hosts)
        .ok_or_else(|| format!("'{}' is not this machine, a saved herdr machine, or a reachable tailnet peer -- refusing to run anything against it", link.host))?;
    let p = plan::plan(link, kind);
    if cfg!(target_os = "macos") {
        return macos::spawn(&p.program, &p.args, find_on_path, dirs::home_dir().as_deref());
    }
    terminal::spawn(&p.program, &p.args, on_path, std::env::var("TERMINAL").ok().as_deref())
}

/// `agents-tui handler install [--dry-run]`: run it, print to `out`, and
/// return the exit code.
pub fn run_install(args: &[String], out: &mut dyn Write) -> i32 {
    if args.first().map(String::as_str) != Some("install") {
        let _ = writeln!(out, "{INSTALL_USAGE}");
        return 2;
    }
    let mut dry_run = false;
    for arg in &args[1..] {
        match arg.as_str() {
            "--dry-run" => dry_run = true,
            other => {
                let _ = writeln!(out, "unknown option {other}\n{INSTALL_USAGE}");
                return 2;
            }
        }
    }
    if cfg!(target_os = "macos") {
        return run_install_macos(dry_run, out);
    }
    if !cfg!(target_os = "linux") {
        let _ = writeln!(out, "agents-tui handler install is for Linux and macOS (Windows is future work; see laird/agents-ui#549).");
        return 1;
    }
    let Some(data_local_dir) = dirs::data_local_dir() else {
        let _ = writeln!(out, "can't find your local data directory (XDG_DATA_HOME)");
        return 1;
    };
    let exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => {
            let _ = writeln!(out, "can't find this binary: {e}");
            return 1;
        }
    };
    let run = |program: &str, run_args: &[&str]| -> Result<(), String> {
        let out = std::process::Command::new(program).args(run_args).output().map_err(|e| format!("could not run {program}: {e}"))?;
        if out.status.success() {
            Ok(())
        } else {
            Err(format!("{program} failed: {}", String::from_utf8_lossy(&out.stderr).trim()))
        }
    };
    match install::install(&exe, &data_local_dir, dry_run, run, out) {
        Ok(()) => 0,
        Err(e) => {
            let _ = writeln!(out, "agents-tui handler install: {e}");
            1
        }
    }
}

fn run_install_macos(dry_run: bool, out: &mut dyn Write) -> i32 {
    let Some(home) = dirs::home_dir() else {
        let _ = writeln!(out, "can't find your home directory");
        return 1;
    };
    let exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => {
            let _ = writeln!(out, "can't find this binary: {e}");
            return 1;
        }
    };
    let run = |program: &str, run_args: &[String]| -> Result<(), String> {
        let out = std::process::Command::new(program).args(run_args).output().map_err(|e| format!("could not run {program}: {e}"))?;
        if out.status.success() {
            Ok(())
        } else {
            Err(format!("{program} failed: {}", String::from_utf8_lossy(&out.stderr).trim()))
        }
    };
    match macos::install(&exe, &home, dry_run, run, out) {
        Ok(()) => 0,
        Err(e) => {
            let _ = writeln!(out, "agents-tui handler install: {e}");
            1
        }
    }
}

/// The Mac App Store and standalone Tailscale apps ship their CLI inside
/// the app bundle, not on `PATH`.
const MACOS_TAILSCALE: &str = "/Applications/Tailscale.app/Contents/MacOS/Tailscale";

fn find_on_path(program: &str) -> Option<std::path::PathBuf> {
    std::env::split_paths(&std::env::var_os("PATH")?).map(|d| d.join(program)).find(|p| p.is_file())
}

fn on_path(program: &str) -> bool {
    find_on_path(program).is_some()
}

/// This machine's own names: `hostname`'s output, plus tailscale's `Self`
/// entry (its short `HostName` and full `DNSName`) when tailscale is up.
fn local_hostnames() -> Vec<String> {
    let mut names = Vec::new();
    if let Ok(out) = std::process::Command::new("hostname").output()
        && out.status.success()
    {
        let h = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !h.is_empty() {
            names.push(h);
        }
    }
    if let Some(json) = run_json("tailscale", &["status", "--json"]) {
        names.extend(tailscale_self_names(&json));
    }
    names
}

/// Every tailnet peer's names (short and FQDN), plus every saved herdr
/// machine's label and target -- the hosts `open` will act on remotely.
fn known_remote_hosts() -> Vec<String> {
    let mut hosts = Vec::new();
    if let Some(json) = run_json("tailscale", &["status", "--json"]) {
        hosts.extend(tailscale_peer_names(&json));
    }
    if let Some(json) = run_json("herdr", &["machine", "list", "--json"]) {
        hosts.extend(herdr_machine_names(&json));
    }
    hosts
}

fn run_json(program: &str, args: &[&str]) -> Option<serde_json::Value> {
    let resolved = match find_on_path(program) {
        Some(p) => p,
        None if cfg!(target_os = "macos") && program == "tailscale" => std::path::PathBuf::from(MACOS_TAILSCALE),
        None => std::path::PathBuf::from(program),
    };
    let out = std::process::Command::new(resolved).args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    serde_json::from_slice(&out.stdout).ok()
}

fn tailscale_self_names(status: &serde_json::Value) -> Vec<String> {
    dns_and_short_names(status.get("Self"))
}

fn tailscale_peer_names(status: &serde_json::Value) -> Vec<String> {
    let Some(peers) = status.get("Peer").and_then(|p| p.as_object()) else { return Vec::new() };
    peers.values().flat_map(|v| dns_and_short_names(Some(v))).collect()
}

fn dns_and_short_names(entry: Option<&serde_json::Value>) -> Vec<String> {
    let Some(entry) = entry else { return Vec::new() };
    let mut names = Vec::new();
    if let Some(dns) = entry.get("DNSName").and_then(|v| v.as_str()) {
        names.extend(host::tailnet_names(dns));
    }
    if let Some(host_name) = entry.get("HostName").and_then(|v| v.as_str())
        && !host_name.is_empty()
    {
        names.push(host_name.to_string());
    }
    names
}

/// `herdr machine list --json`'s `label` and `target` fields: the names a
/// human already chose to identify a saved machine by.
fn herdr_machine_names(machines: &serde_json::Value) -> Vec<String> {
    let Some(list) = machines.as_array() else { return Vec::new() };
    list.iter()
        .flat_map(|m| [m.get("label"), m.get("target")])
        .filter_map(|v| v.and_then(|v| v.as_str()).map(str::to_string))
        .filter(|s| !s.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const TAILSCALE_STATUS: &str = r#"{
        "Self": {"HostName": "nuc", "DNSName": "nuc-1.tail04f2b7.ts.net."},
        "Peer": {
            "a": {"HostName": "workbox", "DNSName": "workbox.tail04f2b7.ts.net."},
            "b": {"HostName": "imac", "DNSName": "lairds-imac.tail04f2b7.ts.net."}
        }
    }"#;

    #[test]
    fn self_names_include_the_short_and_full_tailnet_name() {
        let v: serde_json::Value = serde_json::from_str(TAILSCALE_STATUS).unwrap();
        let names = tailscale_self_names(&v);
        assert!(names.contains(&"nuc-1".to_string()));
        assert!(names.contains(&"nuc-1.tail04f2b7.ts.net".to_string()));
        assert!(names.contains(&"nuc".to_string()), "HostName also counts");
    }

    #[test]
    fn peer_names_cover_every_peer() {
        let v: serde_json::Value = serde_json::from_str(TAILSCALE_STATUS).unwrap();
        let names = tailscale_peer_names(&v);
        assert!(names.contains(&"workbox".to_string()));
        assert!(names.contains(&"lairds-imac".to_string()));
    }

    #[test]
    fn herdr_machine_names_reads_label_and_target() {
        let v: serde_json::Value = serde_json::from_str(r#"[{"label": "Build machine", "target": "workbox", "id": "abc123"}]"#).unwrap();
        let names = herdr_machine_names(&v);
        assert_eq!(names, vec!["Build machine", "workbox"]);
    }

    #[test]
    fn an_empty_or_malformed_herdr_machine_list_is_no_hosts() {
        assert_eq!(herdr_machine_names(&serde_json::json!([])), Vec::<String>::new());
        assert_eq!(herdr_machine_names(&serde_json::json!("not an array")), Vec::<String>::new());
    }

    #[test]
    fn run_open_rejects_a_malformed_link_before_touching_the_network() {
        let mut out = Vec::new();
        assert_eq!(run_open(&["not-a-url".to_string()], &mut out), 2);
        assert!(String::from_utf8(out).unwrap().contains("not an agents-ui"));
    }

    #[test]
    fn run_open_with_no_argument_prints_usage() {
        let mut out = Vec::new();
        assert_eq!(run_open(&[], &mut out), 2);
        assert!(String::from_utf8(out).unwrap().contains(OPEN_USAGE));
    }

    #[test]
    fn run_install_requires_the_install_subcommand() {
        let mut out = Vec::new();
        assert_eq!(run_install(&[], &mut out), 2);
        assert_eq!(run_install(&["uninstall".to_string()], &mut out), 2);
    }

    #[test]
    fn run_install_rejects_an_unknown_flag() {
        let mut out = Vec::new();
        assert_eq!(run_install(&["install".to_string(), "--bogus".to_string()], &mut out), 2);
    }
}
