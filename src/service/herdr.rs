//! `agents-tui service install --with-herdr` (#428): the herdr server, which
//! hosts the swarm's panes, as a systemd user unit beside agents-tui's.
//!
//! Starting or restarting herdr ends every pane it hosts, and a herdr server
//! that is already running can't be adopted by systemd. So when one is
//! running, the unit is written and enabled but not started: it takes over at
//! the next login, or once the running herdr exits. Only when no herdr is
//! running is it started now. `uninstall --with-herdr` likewise leaves a
//! running herdr alone unless `--stop-herdr` asks for it.

use std::io::Write;
use std::path::{Path, PathBuf};

use super::systemd::{Runner, line_diff};
use super::unit::{systemd_path, systemd_word};

pub const UNIT: &str = "herdr.service";

/// `~/.config/systemd/user/herdr.service`.
pub fn unit_path(home: &Path) -> PathBuf {
    home.join(".config/systemd/user").join(UNIT)
}

/// The unit's text: `herdr server` with the same PATH as agents-tui's unit.
/// It restarts only after a crash: a clean stop is someone meaning it.
pub fn herdr_unit(home: &Path, herdr: &Path, path: &[PathBuf]) -> String {
    let exe = systemd_path(home, herdr);
    let exe = if exe.starts_with("%h/") { exe } else { systemd_word(&exe) };
    let path: Vec<String> = path.iter().map(|p| systemd_path(home, p)).collect();
    format!(
        "[Unit]
Description=herdr server (hosts the agent swarm panes)
After=default.target

[Service]
WorkingDirectory=%h
Environment=PATH={path}
ExecStart={exe} server
# Restarting herdr ends every pane it hosts, so only restart it after a crash,
# never on a clean stop.
Restart=on-failure
RestartSec=5

[Install]
WantedBy=default.target
",
        path = path.join(":"),
    )
}

/// Whether a herdr server is running now, under systemd or not.
pub fn server_running(runner: &mut dyn Runner) -> bool {
    let out = runner.run("herdr", &["status", "server"]);
    out.ok && out.stdout.lines().any(|l| l.trim() == "status: running")
}

fn unit_active(runner: &mut dyn Runner) -> bool {
    runner.run("systemctl", &["--user", "is-active", UNIT]).stdout.trim() == "active"
}

fn systemctl(runner: &mut dyn Runner, args: &[&str]) -> Result<(), String> {
    let mut all = vec!["--user"];
    all.extend_from_slice(args);
    if runner.run("systemctl", &all).ok { Ok(()) } else { Err(format!("systemctl {} failed", all.join(" "))) }
}

fn say(out: &mut dyn Write, s: impl AsRef<str>) -> Result<(), String> {
    writeln!(out, "{}", s.as_ref()).map_err(|e| e.to_string())
}

/// What `install` does with the unit, decided before anything changes.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Plan {
    /// No herdr is running: enable and start it.
    Start,
    /// herdr.service is already the running herdr: leave it running.
    AlreadyServing,
    /// A herdr outside systemd is running: enable only.
    EnableOnly,
}

fn plan(runner: &mut dyn Runner) -> Plan {
    if unit_active(runner) {
        Plan::AlreadyServing
    } else if server_running(runner) {
        Plan::EnableOnly
    } else {
        Plan::Start
    }
}

/// Write and enable herdr.service; start it only when no herdr is running.
pub fn install(home: &Path, herdr: &Path, path: &[PathBuf], dry_run: bool, runner: &mut dyn Runner, out: &mut dyn Write) -> Result<(), String> {
    let text = herdr_unit(home, herdr, path);
    let file = unit_path(home);
    let old = std::fs::read_to_string(&file).ok();
    let plan = plan(runner);
    let enable = match plan {
        Plan::Start => format!("systemctl --user enable --now {UNIT}"),
        Plan::AlreadyServing | Plan::EnableOnly => format!("systemctl --user enable {UNIT}"),
    };

    if dry_run {
        say(out, format!("Would write {}:\n", file.display()))?;
        say(out, &text)?;
        if let Some(old) = old.as_ref().filter(|o| **o != text) {
            say(out, "Changes from the installed herdr unit:")?;
            for line in line_diff(old, &text) {
                say(out, format!("  {line}"))?;
            }
        }
        say(out, "Would run:")?;
        say(out, "  systemctl --user daemon-reload")?;
        say(out, format!("  {enable}"))?;
        return say(out, explain(plan, old.as_ref().is_some_and(|o| *o != text)));
    }

    match &old {
        Some(old) if *old != text => {
            say(out, format!("Updating {}:", file.display()))?;
            for line in line_diff(old, &text) {
                say(out, format!("  {line}"))?;
            }
        }
        Some(_) => say(out, format!("{} is unchanged.", file.display()))?,
        None => say(out, format!("Writing {}", file.display()))?,
    }
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("can't create {}: {e}", dir.display()))?;
    }
    std::fs::write(&file, &text).map_err(|e| format!("can't write {}: {e}", file.display()))?;
    systemctl(runner, &["daemon-reload"])?;
    match plan {
        Plan::Start => systemctl(runner, &["enable", "--now", UNIT])?,
        Plan::AlreadyServing | Plan::EnableOnly => systemctl(runner, &["enable", UNIT])?,
    }
    say(out, explain(plan, old.as_ref().is_some_and(|o| *o != text)))
}

fn explain(plan: Plan, changed: bool) -> String {
    match plan {
        Plan::Start => format!("No herdr was running: {UNIT} is enabled and started."),
        Plan::AlreadyServing if changed => format!(
            "{UNIT} is enabled and already running herdr; it was not restarted, because that ends every pane. \
             The changes apply the next time herdr starts."
        ),
        Plan::AlreadyServing => format!("{UNIT} is enabled and already running herdr."),
        Plan::EnableOnly => format!(
            "A herdr server is already running outside systemd, so {UNIT} is enabled but not started: \
             starting it would end every pane. It takes over at your next login, or once the running herdr exits \
             (`herdr server stop`, then `systemctl --user start {UNIT}`)."
        ),
    }
}

/// Disable and remove herdr.service. A running herdr keeps running unless
/// `stop` says to stop it, which ends every pane.
pub fn uninstall(home: &Path, stop: bool, runner: &mut dyn Runner, out: &mut dyn Write) -> Result<(), String> {
    let file = unit_path(home);
    let active = unit_active(runner);
    if active && stop {
        runner.run("systemctl", &["--user", "stop", UNIT]);
    }
    // Disabling a unit that's already gone isn't a failure.
    runner.run("systemctl", &["--user", "disable", UNIT]);
    match std::fs::remove_file(&file) {
        Ok(()) => say(out, format!("Removed {}", file.display()))?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => say(out, format!("{} was not installed.", file.display()))?,
        Err(e) => return Err(format!("can't remove {}: {e}", file.display())),
    }
    systemctl(runner, &["daemon-reload"])?;
    if active && !stop {
        say(
            out,
            "herdr is still running (stopping it ends every pane). It won't start under systemd again. \
             Stop it with `herdr server stop`, or re-run with --stop-herdr.",
        )?;
    } else if active {
        say(out, "Stopped herdr.")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::systemd::Output;
    use std::time::Duration;

    /// A fake runner: whether herdr.service is active, and whether any herdr
    /// server answers `herdr status server`.
    #[derive(Default)]
    struct Fake {
        ran: Vec<String>,
        unit_active: bool,
        herdr_running: bool,
    }

    impl Runner for Fake {
        fn run(&mut self, program: &str, args: &[&str]) -> Output {
            self.ran.push(format!("{program} {}", args.join(" ")));
            match (program, args) {
                ("herdr", ["status", "server"]) if self.herdr_running => {
                    Output { ok: true, stdout: "status: running\nversion: 0.9.5\nsocket: /home/me/.config/herdr/herdr.sock\n".into() }
                }
                ("herdr", _) => Output { ok: false, stdout: "status: not running\n".into() },
                ("systemctl", [_, "is-active", _]) => Output { ok: self.unit_active, stdout: if self.unit_active { "active\n" } else { "inactive\n" }.into() },
                _ => Output { ok: true, stdout: String::new() },
            }
        }
        fn village_answers(&mut self, _: &str, _: u16) -> bool {
            true
        }
        fn sleep(&mut self, _: Duration) {}
    }

    fn home(name: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let dir = std::env::temp_dir().join(format!("agents-ui-herdr-{name}-{}-{nanos}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn path(h: &Path) -> Vec<PathBuf> {
        vec![h.join(".local/share/mise/shims"), h.join(".cargo/bin"), h.join(".local/bin"), "/usr/local/bin".into(), "/usr/bin".into(), "/bin".into()]
    }

    fn printed(out: &[u8]) -> String {
        String::from_utf8_lossy(out).into_owned()
    }

    #[test]
    fn the_unit_runs_herdr_server_with_the_services_path() {
        let h = home("unit");
        let text = herdr_unit(&h, Path::new("/usr/bin/herdr"), &path(&h));
        assert!(text.contains("ExecStart=/usr/bin/herdr server\n"), "{text}");
        assert!(text.contains("Environment=PATH=%h/.local/share/mise/shims:%h/.cargo/bin:%h/.local/bin:/usr/local/bin:/usr/bin:/bin\n"), "{text}");
        assert!(text.contains("Restart=on-failure") && !text.contains("Restart=always"), "only after a crash: {text}");
        assert!(text.contains("WantedBy=default.target"));
        // herdr under home is written %h/..., as agents-tui's binary is.
        assert!(herdr_unit(&h, &h.join(".local/bin/herdr"), &path(&h)).contains("ExecStart=%h/.local/bin/herdr server\n"));
    }

    #[test]
    fn with_herdr_running_the_unit_is_enabled_but_not_started() {
        let h = home("running");
        let (mut fake, mut out) = (Fake { herdr_running: true, ..Fake::default() }, Vec::new());
        install(&h, Path::new("/usr/bin/herdr"), &path(&h), false, &mut fake, &mut out).unwrap();
        assert!(unit_path(&h).exists());
        assert!(fake.ran.contains(&"systemctl --user enable herdr.service".to_string()), "{:?}", fake.ran);
        assert!(!fake.ran.iter().any(|c| c.contains("--now") || c.contains(" start ") || c.contains("restart")), "never starts it: {:?}", fake.ran);
        let said = printed(&out);
        assert!(said.contains("enabled but not started") && said.contains("next login") && said.contains("running herdr exits"), "{said}");
    }

    #[test]
    fn with_no_herdr_running_it_is_enabled_and_started() {
        let h = home("idle");
        let (mut fake, mut out) = (Fake::default(), Vec::new());
        install(&h, Path::new("/usr/bin/herdr"), &path(&h), false, &mut fake, &mut out).unwrap();
        assert_eq!(
            fake.ran,
            vec![
                "systemctl --user is-active herdr.service",
                "herdr status server",
                "systemctl --user daemon-reload",
                "systemctl --user enable --now herdr.service",
            ]
        );
        assert!(printed(&out).contains("enabled and started"));
    }

    #[test]
    fn a_herdr_already_under_systemd_is_never_restarted() {
        let h = home("active");
        install(&h, Path::new("/usr/bin/herdr"), &path(&h), false, &mut Fake::default(), &mut Vec::new()).unwrap();
        // A changed unit over the running service: written, enabled, not restarted.
        let (mut fake, mut out) = (Fake { unit_active: true, herdr_running: true, ..Fake::default() }, Vec::new());
        install(&h, Path::new("/opt/herdr/herdr"), &path(&h), false, &mut fake, &mut out).unwrap();
        assert!(!fake.ran.iter().any(|c| c.contains("restart") || c.contains("--now") || c.contains(" stop ")), "{:?}", fake.ran);
        let said = printed(&out);
        assert!(said.contains("-ExecStart=/usr/bin/herdr server") && said.contains("+ExecStart=/opt/herdr/herdr server"), "{said}");
        assert!(said.contains("was not restarted") && said.contains("next time herdr starts"), "{said}");
    }

    #[test]
    fn a_dry_run_says_what_it_would_do_and_changes_nothing() {
        let h = home("dry");
        let (mut fake, mut out) = (Fake { herdr_running: true, ..Fake::default() }, Vec::new());
        install(&h, Path::new("/usr/bin/herdr"), &path(&h), true, &mut fake, &mut out).unwrap();
        assert!(!unit_path(&h).exists());
        assert!(fake.ran.iter().all(|c| c.contains("is-active") || c.starts_with("herdr status")), "only probes: {:?}", fake.ran);
        let said = printed(&out);
        assert!(said.contains("ExecStart=/usr/bin/herdr server") && said.contains("  systemctl --user enable herdr.service\n"), "{said}");
        assert!(said.contains("enabled but not started"), "{said}");
        let mut out = Vec::new();
        install(&h, Path::new("/usr/bin/herdr"), &path(&h), true, &mut Fake::default(), &mut out).unwrap();
        assert!(printed(&out).contains("  systemctl --user enable --now herdr.service\n"));
    }

    #[test]
    fn uninstall_leaves_a_running_herdr_alone_unless_asked() {
        let h = home("uninstall");
        install(&h, Path::new("/usr/bin/herdr"), &path(&h), false, &mut Fake::default(), &mut Vec::new()).unwrap();
        let (mut fake, mut out) = (Fake { unit_active: true, herdr_running: true, ..Fake::default() }, Vec::new());
        uninstall(&h, false, &mut fake, &mut out).unwrap();
        assert!(!unit_path(&h).exists());
        assert!(!fake.ran.iter().any(|c| c.contains(" stop ")), "{:?}", fake.ran);
        assert!(fake.ran.contains(&"systemctl --user disable herdr.service".to_string()));
        assert!(printed(&out).contains("herdr is still running") && printed(&out).contains("--stop-herdr"));

        install(&h, Path::new("/usr/bin/herdr"), &path(&h), false, &mut Fake::default(), &mut Vec::new()).unwrap();
        let (mut fake, mut out) = (Fake { unit_active: true, herdr_running: true, ..Fake::default() }, Vec::new());
        uninstall(&h, true, &mut fake, &mut out).unwrap();
        assert!(fake.ran.contains(&"systemctl --user stop herdr.service".to_string()));
        assert!(printed(&out).contains("Stopped herdr."));

        let mut out = Vec::new();
        uninstall(&home("uninstall-none"), false, &mut Fake::default(), &mut out).unwrap();
        assert!(printed(&out).contains("was not installed"));
    }
}
