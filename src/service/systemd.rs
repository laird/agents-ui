//! `agents-tui service` on Linux: the served agents-tui as a systemd user
//! unit (#426). `install` writes [`super::unit::systemd_unit`]'s text to
//! `~/.config/systemd/user/agents-tui.service`, enables and starts it, and
//! waits until the village answers; `uninstall`, `status` and `restart` do
//! what they say.
//!
//! Every command goes through a [`Runner`] and every file under the home
//! directory given, so the tests drive no real service.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::unit::{ServiceOptions, systemd_unit, validate};

pub const UNIT: &str = "agents-tui.service";

/// How long `install` and `restart` wait for the village to answer.
pub(crate) const WAIT: Duration = Duration::from_secs(30);
const WAIT_STEP: Duration = Duration::from_secs(1);

/// What a command printed, and whether it succeeded.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Output {
    pub ok: bool,
    pub stdout: String,
}

/// Runs commands and probes the village; the tests record instead.
pub trait Runner {
    fn run(&mut self, program: &str, args: &[&str]) -> Output;
    /// Whether `GET <base_path>/api/village` at `host:port` answers 200.
    fn village_answers(&mut self, host: &str, port: u16, base_path: &str) -> bool;
    fn sleep(&mut self, d: Duration);
}

/// The real thing: processes, a TCP request, and the clock.
pub struct SystemRunner;

impl Runner for SystemRunner {
    fn run(&mut self, program: &str, args: &[&str]) -> Output {
        match std::process::Command::new(program).args(args).output() {
            Ok(out) => Output { ok: out.status.success(), stdout: String::from_utf8_lossy(&out.stdout).into_owned() },
            Err(_) => Output::default(),
        }
    }
    fn village_answers(&mut self, host: &str, port: u16, base_path: &str) -> bool {
        use std::io::Read;
        let addr = if host.contains(':') { format!("[{host}]:{port}") } else { format!("{host}:{port}") };
        let Ok(mut addrs) = std::net::ToSocketAddrs::to_socket_addrs(&addr) else { return false };
        let Some(addr) = addrs.next() else { return false };
        let Ok(mut s) = std::net::TcpStream::connect_timeout(&addr, Duration::from_secs(3)) else { return false };
        let _ = s.set_read_timeout(Some(Duration::from_secs(5)));
        if s.write_all(format!("GET {base_path}/api/village HTTP/1.0\r\nHost: {host}\r\n\r\n").as_bytes()).is_err() {
            return false;
        }
        let mut head = [0u8; 12];
        s.read_exact(&mut head).is_ok() && head.starts_with(b"HTTP/1.") && &head[9..12] == b"200"
    }
    fn sleep(&mut self, d: Duration) {
        std::thread::sleep(d);
    }
}

/// `~/.config/systemd/user/agents-tui.service`.
pub fn unit_path(home: &Path) -> PathBuf {
    home.join(".config/systemd/user").join(UNIT)
}

/// `install`'s switches that aren't the server's own options.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct InstallFlags {
    /// Keep the user's services running after logout (`loginctl enable-linger`).
    pub linger: bool,
    /// Print the unit and the commands, change nothing.
    pub dry_run: bool,
}

/// The lines that differ between two texts: `-` removed, `+` added, in order.
/// Enough to show what a re-install changed in a short unit file.
pub fn line_diff(old: &str, new: &str) -> Vec<String> {
    let (a, b): (Vec<&str>, Vec<&str>) = (old.lines().collect(), new.lines().collect());
    // Longest common subsequence; a changed line shows as - then + in place.
    let mut lcs = vec![vec![0usize; b.len() + 1]; a.len() + 1];
    for i in (0..a.len()).rev() {
        for j in (0..b.len()).rev() {
            lcs[i][j] = if a[i] == b[j] { lcs[i + 1][j + 1] + 1 } else { lcs[i + 1][j].max(lcs[i][j + 1]) };
        }
    }
    let (mut i, mut j, mut out) = (0, 0, Vec::new());
    while i < a.len() || j < b.len() {
        if i < a.len() && j < b.len() && a[i] == b[j] {
            i += 1;
            j += 1;
        } else if i < a.len() && (j == b.len() || lcs[i + 1][j] >= lcs[i][j + 1]) {
            out.push(format!("-{}", a[i]));
            i += 1;
        } else {
            out.push(format!("+{}", b[j]));
            j += 1;
        }
    }
    out
}

/// Where to reach the service from this machine: 127.0.0.1 when it binds
/// there (or binds nothing literal), else its first literal address, else
/// the tailnet address `tailscale ip -4` gives.
pub fn reach_host(opts: &ServiceOptions, runner: &mut dyn Runner) -> String {
    let binds: Vec<&str> = opts.web_binds.iter().map(|b| b.trim()).collect();
    // Listening on localhost, or on every interface, which includes it.
    if binds.is_empty() || binds.iter().any(|b| matches!(*b, "127.0.0.1" | "0.0.0.0" | "::" | "[::]")) {
        return "127.0.0.1".into();
    }
    if let Some(b) = binds.iter().find(|b| **b != "tailscale") {
        return b.trim_matches(['[', ']']).to_string();
    }
    let ip = runner.run("tailscale", &["ip", "-4"]);
    ip.stdout.lines().next().map(str::trim).filter(|s| !s.is_empty()).unwrap_or("127.0.0.1").to_string()
}

/// The service's root at `host`, base path included: `http://127.0.0.1:7890/ops/village`.
pub(crate) fn url(host: &str, opts: &ServiceOptions) -> String {
    let (port, base) = (opts.web_port, &opts.base_path);
    if host.contains(':') { format!("http://[{host}]:{port}{base}") } else { format!("http://{host}:{port}{base}") }
}

/// Wait until the village answers, up to [`WAIT`].
pub(crate) fn wait_for_village(runner: &mut dyn Runner, host: &str, opts: &ServiceOptions) -> bool {
    let mut waited = Duration::ZERO;
    loop {
        if runner.village_answers(host, opts.web_port, &opts.base_path) {
            return true;
        }
        if waited >= WAIT {
            return false;
        }
        runner.sleep(WAIT_STEP);
        waited += WAIT_STEP;
    }
}

fn systemctl(runner: &mut dyn Runner, args: &[&str]) -> Result<(), String> {
    let mut all = vec!["--user"];
    all.extend_from_slice(args);
    if runner.run("systemctl", &all).ok { Ok(()) } else { Err(format!("systemctl {} failed", all.join(" "))) }
}

fn lingering(runner: &mut dyn Runner, user: &str) -> bool {
    runner.run("loginctl", &["show-user", user, "-p", "Linger", "--value"]).stdout.trim() == "yes"
}

/// Write the unit, enable and (re)start it, wait for the village, and say
/// where it is. A dry run prints the unit and the commands only.
pub fn install(opts: &ServiceOptions, flags: InstallFlags, user: &str, runner: &mut dyn Runner, out: &mut dyn Write) -> Result<(), String> {
    validate(opts)?;
    let text = systemd_unit(opts)?;
    let path = unit_path(&opts.home);
    let old = std::fs::read_to_string(&path).ok();
    let mut commands = vec![format!("systemctl --user daemon-reload"), format!("systemctl --user enable --now {UNIT}")];
    if old.is_some() {
        commands.push(format!("systemctl --user restart {UNIT}"));
    }
    if flags.linger {
        commands.push(format!("loginctl enable-linger {user}"));
    }
    let say = |out: &mut dyn Write, s: String| writeln!(out, "{s}").map_err(|e| e.to_string());

    if flags.dry_run {
        say(out, format!("Would write {}:\n", path.display()))?;
        say(out, text.clone())?;
        if let Some(old) = old.as_ref().filter(|o| **o != text) {
            say(out, "Changes from the installed unit:".into())?;
            for line in line_diff(old, &text) {
                say(out, format!("  {line}"))?;
            }
        }
        say(out, "Would run:".into())?;
        for c in &commands {
            say(out, format!("  {c}"))?;
        }
        return say(out, format!("Would wait for the village at {}/api/village", url(&reach_host(opts, runner), opts)));
    }

    match &old {
        Some(old) if *old != text => {
            say(out, format!("Updating {}:", path.display()))?;
            for line in line_diff(old, &text) {
                say(out, format!("  {line}"))?;
            }
        }
        Some(_) => say(out, format!("{} is unchanged.", path.display()))?,
        None => say(out, format!("Writing {}", path.display()))?,
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("can't create {}: {e}", dir.display()))?;
    }
    std::fs::write(&path, &text).map_err(|e| format!("can't write {}: {e}", path.display()))?;

    systemctl(runner, &["daemon-reload"])?;
    systemctl(runner, &["enable", "--now", UNIT])?;
    if old.is_some() {
        systemctl(runner, &["restart", UNIT])?;
    }
    if flags.linger {
        if !runner.run("loginctl", &["enable-linger", user]).ok {
            return Err(format!("loginctl enable-linger {user} failed"));
        }
        say(out, "Lingering on: the service keeps running after you log out.".into())?;
    } else if !lingering(runner, user) {
        say(out, format!(
            "Note: without lingering, systemd stops your user services when you log out, and the village with them. \
             Run `loginctl enable-linger {user}` (or re-run with --linger) to keep it serving."
        ))?;
    }

    let host = reach_host(opts, runner);
    if !wait_for_village(runner, &host, opts) {
        return Err(format!(
            "{UNIT} started, but the village didn't answer at {}/api/village within {}s. See: journalctl --user -u {UNIT}",
            url(&host, opts),
            WAIT.as_secs()
        ));
    }
    say(out, format!("The village: {}/village", url(&host, opts)))?;
    say(out, format!("The dashboard: {}/", url(&host, opts)))?;
    Ok(())
}

/// Stop, disable and remove the unit, then reload systemd.
pub fn uninstall(home: &Path, runner: &mut dyn Runner, out: &mut dyn Write) -> Result<(), String> {
    let path = unit_path(home);
    // Stopping or disabling a unit that's already gone isn't a failure.
    runner.run("systemctl", &["--user", "stop", UNIT]);
    runner.run("systemctl", &["--user", "disable", UNIT]);
    match std::fs::remove_file(&path) {
        Ok(()) => writeln!(out, "Removed {}", path.display()).map_err(|e| e.to_string())?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => writeln!(out, "{} was not installed.", path.display()).map_err(|e| e.to_string())?,
        Err(e) => return Err(format!("can't remove {}: {e}", path.display())),
    }
    systemctl(runner, &["daemon-reload"])
}

/// The installed unit's binary, port, binds and base path, read back from its ExecStart.
pub fn installed(text: &str, home: &Path) -> Option<ServiceOptions> {
    let line = text.lines().find_map(|l| l.strip_prefix("ExecStart="))?;
    let words = shell_words(line);
    let exe = words.first()?;
    let exe = match exe.strip_prefix("%h/") {
        Some(rest) => home.join(rest),
        None => PathBuf::from(exe),
    };
    Some(served(exe, &words, home))
}

/// What `status` and `restart` need from the server's argument words:
/// `--web-port`, `--web-bind` and `--base-path`.
pub(crate) fn served(exe: PathBuf, words: &[String], home: &Path) -> ServiceOptions {
    let after = |flag: &str| words.iter().position(|w| w == flag).and_then(|i| words.get(i + 1)).cloned();
    ServiceOptions {
        web_port: after("--web-port").and_then(|p| p.parse().ok()).unwrap_or(super::unit::DEFAULT_PORT),
        web_binds: after("--web-bind").map(|b| b.split(',').map(str::to_string).collect()).unwrap_or_else(|| vec!["127.0.0.1".into()]),
        sources: vec![],
        read_only: false,
        allow_lan: false,
        base_path: after("--base-path").unwrap_or_default(),
        frame_ancestors: None,
        exe,
        home: home.to_path_buf(),
        path: vec![],
    }
}

/// Split an ExecStart line into words: double quotes group, `%%` is `%`.
fn shell_words(line: &str) -> Vec<String> {
    let (mut words, mut cur, mut quoted, mut any) = (Vec::new(), String::new(), false, false);
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                quoted = !quoted;
                any = true;
            }
            '\\' if quoted => {
                if let Some(n) = chars.next() {
                    cur.push(n);
                }
            }
            c if c.is_whitespace() && !quoted => {
                if any || !cur.is_empty() {
                    words.push(std::mem::take(&mut cur));
                    any = false;
                }
            }
            _ => cur.push(c),
        }
    }
    if any || !cur.is_empty() {
        words.push(cur);
    }
    words.into_iter().map(|w| w.replace("%%", "%")).collect()
}

/// The unit's state, where it serves, whether the village answers, and the
/// version of the binary it runs.
pub fn status(home: &Path, runner: &mut dyn Runner, out: &mut dyn Write) -> Result<(), String> {
    let say = |out: &mut dyn Write, s: String| writeln!(out, "{s}").map_err(|e| e.to_string());
    let path = unit_path(home);
    let Ok(text) = std::fs::read_to_string(&path) else {
        return say(out, format!("{UNIT} is not installed (no {}). Install it with: agents-tui service install", path.display()));
    };
    let state = runner.run("systemctl", &["--user", "is-active", UNIT]).stdout.trim().to_string();
    let enabled = runner.run("systemctl", &["--user", "is-enabled", UNIT]).stdout.trim().to_string();
    say(out, format!("{UNIT}: {} ({})", if state.is_empty() { "unknown" } else { &state }, if enabled.is_empty() { "unknown" } else { &enabled }))?;
    say(out, format!("Unit file: {}", path.display()))?;
    let Some(opts) = installed(&text, home) else {
        return say(out, "The unit has no ExecStart line agents-tui can read.".into());
    };
    let host = reach_host(&opts, runner);
    let answers = runner.village_answers(&host, opts.web_port, &opts.base_path);
    say(out, format!("Village: {}/village ({})", url(&host, &opts), if answers { "answering" } else { "not answering" }))?;
    let exe = opts.exe;
    let exe_str = exe.to_string_lossy().into_owned();
    let version = runner.run(&exe_str, &["--version"]);
    // A binary built before `--version` existed runs fine but can't say.
    let said = match (version.ok, exe.exists()) {
        (true, _) => version.stdout.trim().to_string(),
        (false, true) => "doesn't report a version".into(),
        (false, false) => "missing".into(),
    };
    say(out, format!("Binary: {} ({said})", exe.display()))
}

/// Restart the unit and wait for the village.
pub fn restart(home: &Path, runner: &mut dyn Runner, out: &mut dyn Write) -> Result<(), String> {
    let text = std::fs::read_to_string(unit_path(home)).map_err(|_| format!("{UNIT} is not installed. Install it with: agents-tui service install"))?;
    systemctl(runner, &["restart", UNIT])?;
    let opts = installed(&text, home).unwrap_or_else(|| served(PathBuf::new(), &[], home));
    let host = reach_host(&opts, runner);
    if !wait_for_village(runner, &host, &opts) {
        return Err(format!("restarted, but the village didn't answer at {}/api/village within {}s", url(&host, &opts), WAIT.as_secs()));
    }
    writeln!(out, "Restarted. The village: {}/village", url(&host, &opts)).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Records each command; answers from a script.
    #[derive(Default)]
    struct Fake {
        ran: Vec<String>,
        /// The village answers after this many probes (None: never).
        answers_after: Option<usize>,
        probes: usize,
        linger: bool,
        tailnet: Option<String>,
        active: String,
        version: Option<String>,
        slept: Duration,
    }

    impl Runner for Fake {
        fn run(&mut self, program: &str, args: &[&str]) -> Output {
            self.ran.push(format!("{program} {}", args.join(" ")));
            let stdout = match (program, args) {
                ("loginctl", [.., "Linger", "--value"]) => if self.linger { "yes\n" } else { "no\n" }.to_string(),
                ("tailscale", _) => return Output { ok: self.tailnet.is_some(), stdout: self.tailnet.clone().unwrap_or_default() },
                ("systemctl", [_, "is-active", _]) => format!("{}\n", self.active),
                ("systemctl", [_, "is-enabled", _]) => "enabled\n".into(),
                (_, ["--version"]) => return Output { ok: self.version.is_some(), stdout: self.version.clone().unwrap_or_default() },
                _ => String::new(),
            };
            Output { ok: true, stdout }
        }
        fn village_answers(&mut self, host: &str, port: u16, base_path: &str) -> bool {
            self.ran.push(format!("probe {host}:{port}{base_path}"));
            self.probes += 1;
            self.answers_after.is_some_and(|n| self.probes > n)
        }
        fn sleep(&mut self, d: Duration) {
            self.slept += d;
        }
    }

    fn home(name: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let dir = std::env::temp_dir().join(format!("agents-ui-service-{name}-{}-{nanos}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn opts(home: &Path) -> ServiceOptions {
        ServiceOptions {
            web_port: 7890,
            web_binds: vec!["127.0.0.1".into(), "tailscale".into()],
            sources: vec!["swarms=herdr".into()],
            read_only: false,
            allow_lan: false,
            base_path: String::new(),
            frame_ancestors: None,
            exe: home.join(".cargo/bin/agents-tui"),
            home: home.to_path_buf(),
            path: vec![home.join(".cargo/bin"), PathBuf::from("/usr/bin")],
        }
    }

    fn up() -> Fake {
        Fake { answers_after: Some(0), ..Fake::default() }
    }

    fn printed(out: &[u8]) -> String {
        String::from_utf8_lossy(out).into_owned()
    }

    #[test]
    fn install_writes_the_unit_and_enables_it_in_order() {
        let h = home("install");
        let (mut fake, mut out) = (up(), Vec::new());
        install(&opts(&h), InstallFlags::default(), "me", &mut fake, &mut out).unwrap();
        let text = std::fs::read_to_string(unit_path(&h)).unwrap();
        assert_eq!(text, systemd_unit(&opts(&h)).unwrap());
        assert_eq!(
            fake.ran,
            vec![
                "systemctl --user daemon-reload",
                "systemctl --user enable --now agents-tui.service",
                "loginctl show-user me -p Linger --value",
                "probe 127.0.0.1:7890",
            ]
        );
        let said = printed(&out);
        assert!(said.contains("loginctl enable-linger me"), "says how to linger: {said}");
        assert!(said.contains("The village: http://127.0.0.1:7890/village"));
        assert!(said.contains("The dashboard: http://127.0.0.1:7890/"));
    }

    #[test]
    fn reinstalling_with_changed_options_shows_a_diff_and_restarts() {
        let h = home("reinstall");
        install(&opts(&h), InstallFlags::default(), "me", &mut up(), &mut Vec::new()).unwrap();
        let changed = ServiceOptions { web_port: 7999, read_only: true, ..opts(&h) };
        let (mut fake, mut out) = (Fake { linger: true, ..up() }, Vec::new());
        install(&changed, InstallFlags::default(), "me", &mut fake, &mut out).unwrap();
        let said = printed(&out);
        assert!(said.contains("Updating"), "{said}");
        assert!(said.contains("-ExecStart=") && said.contains("+ExecStart="), "{said}");
        assert!(said.contains("--web-port 7999") && said.contains("--read-only"), "{said}");
        assert!(fake.ran.contains(&"systemctl --user restart agents-tui.service".to_string()));
        // Lingering already: no note about it.
        assert!(!said.contains("Note: without lingering"));
        assert_eq!(fake.ran.last().unwrap(), "probe 127.0.0.1:7999");
    }

    #[test]
    fn a_dry_run_writes_and_runs_nothing() {
        let h = home("dry");
        let (mut fake, mut out) = (up(), Vec::new());
        install(&opts(&h), InstallFlags { dry_run: true, linger: true }, "me", &mut fake, &mut out).unwrap();
        assert!(fake.ran.is_empty(), "{:?}", fake.ran);
        assert!(!unit_path(&h).exists());
        let said = printed(&out);
        assert!(said.contains("[Service]") && said.contains("systemctl --user enable --now agents-tui.service"));
        assert!(said.contains("loginctl enable-linger me"));
        // Over an installed unit, a dry run shows what would change.
        install(&opts(&h), InstallFlags::default(), "me", &mut up(), &mut Vec::new()).unwrap();
        let before = std::fs::read_to_string(unit_path(&h)).unwrap();
        let mut out = Vec::new();
        install(&ServiceOptions { web_port: 7222, ..opts(&h) }, InstallFlags { dry_run: true, linger: false }, "me", &mut up(), &mut out).unwrap();
        assert!(printed(&out).contains("Changes from the installed unit:") && printed(&out).contains("--web-port 7222"));
        assert_eq!(std::fs::read_to_string(unit_path(&h)).unwrap(), before, "a dry run leaves the unit alone");
    }

    #[test]
    fn linger_only_with_the_flag() {
        let h = home("linger");
        let mut fake = up();
        install(&opts(&h), InstallFlags { linger: true, dry_run: false }, "me", &mut fake, &mut Vec::new()).unwrap();
        assert!(fake.ran.contains(&"loginctl enable-linger me".to_string()));
        let mut fake = up();
        install(&opts(&h), InstallFlags::default(), "me", &mut fake, &mut Vec::new()).unwrap();
        assert!(!fake.ran.iter().any(|c| c.starts_with("loginctl enable-linger")));
    }

    #[test]
    fn every_interface_is_refused_without_allow_lan() {
        let h = home("lan");
        let lan = ServiceOptions { web_binds: vec!["0.0.0.0".into()], ..opts(&h) };
        let mut fake = up();
        let err = install(&lan, InstallFlags::default(), "me", &mut fake, &mut Vec::new()).unwrap_err();
        assert!(err.contains("--allow-lan"), "{err}");
        assert!(fake.ran.is_empty() && !unit_path(&h).exists());
    }

    #[test]
    fn install_waits_for_the_village_and_says_so_when_it_never_answers() {
        let h = home("wait");
        let mut fake = Fake { answers_after: Some(3), ..Fake::default() };
        install(&opts(&h), InstallFlags::default(), "me", &mut fake, &mut Vec::new()).unwrap();
        assert_eq!(fake.slept, Duration::from_secs(3));
        let mut fake = Fake::default();
        let err = install(&opts(&h), InstallFlags::default(), "me", &mut fake, &mut Vec::new()).unwrap_err();
        assert!(err.contains("didn't answer") && err.contains("journalctl --user -u agents-tui.service"), "{err}");
        assert_eq!(fake.slept, WAIT);
    }

    #[test]
    fn uninstall_stops_disables_removes_and_reloads() {
        let h = home("uninstall");
        install(&opts(&h), InstallFlags::default(), "me", &mut up(), &mut Vec::new()).unwrap();
        let mut fake = up();
        uninstall(&h, &mut fake, &mut Vec::new()).unwrap();
        assert_eq!(
            fake.ran,
            vec!["systemctl --user stop agents-tui.service", "systemctl --user disable agents-tui.service", "systemctl --user daemon-reload"]
        );
        assert!(!unit_path(&h).exists());
        // Again, with nothing installed: still not an error.
        let mut out = Vec::new();
        uninstall(&h, &mut up(), &mut out).unwrap();
        assert!(printed(&out).contains("was not installed"));
    }

    #[test]
    fn status_with_the_village_up_and_down() {
        let h = home("status");
        install(&opts(&h), InstallFlags::default(), "me", &mut up(), &mut Vec::new()).unwrap();
        let mut fake = Fake { active: "active".into(), version: Some("agents-tui 0.1.0\n".into()), ..up() };
        let mut out = Vec::new();
        status(&h, &mut fake, &mut out).unwrap();
        let said = printed(&out);
        assert!(said.contains("agents-tui.service: active (enabled)"), "{said}");
        assert!(said.contains("http://127.0.0.1:7890/village (answering)"), "{said}");
        assert!(said.contains(&format!("Binary: {} (agents-tui 0.1.0)", h.join(".cargo/bin/agents-tui").display())), "{said}");

        let mut fake = Fake { active: "failed".into(), ..Fake::default() };
        let mut out = Vec::new();
        status(&h, &mut fake, &mut out).unwrap();
        let said = printed(&out);
        // The installed binary is missing here (a temp home): it says so.
        assert!(said.contains("agents-tui.service: failed") && said.contains("(not answering)") && said.contains("(missing)"), "{said}");

        let mut out = Vec::new();
        status(&home("status-none"), &mut up(), &mut out).unwrap();
        assert!(printed(&out).contains("is not installed"));
    }

    #[test]
    fn restart_restarts_then_waits() {
        let h = home("restart");
        install(&opts(&h), InstallFlags::default(), "me", &mut up(), &mut Vec::new()).unwrap();
        let mut fake = Fake { answers_after: Some(1), ..Fake::default() };
        let mut out = Vec::new();
        restart(&h, &mut fake, &mut out).unwrap();
        assert_eq!(fake.ran[0], "systemctl --user restart agents-tui.service");
        assert_eq!(fake.slept, Duration::from_secs(1));
        assert!(printed(&out).contains("Restarted. The village: http://127.0.0.1:7890/village"));
        assert!(restart(&home("restart-none"), &mut up(), &mut Vec::new()).unwrap_err().contains("not installed"));
    }

    #[test]
    fn reaches_the_service_where_it_binds() {
        let h = home("reach");
        let mut fake = Fake { tailnet: Some("100.64.0.2\n".into()), ..Fake::default() };
        let with = |binds: &[&str]| ServiceOptions { web_binds: binds.iter().map(|b| b.to_string()).collect(), ..opts(&h) };
        assert_eq!(reach_host(&with(&["127.0.0.1", "tailscale"]), &mut fake), "127.0.0.1");
        assert_eq!(reach_host(&with(&["tailscale"]), &mut fake), "100.64.0.2");
        assert_eq!(reach_host(&with(&["192.168.1.5"]), &mut fake), "192.168.1.5");
        assert_eq!(reach_host(&with(&["::1"]), &mut fake), "::1");
    }

    #[test]
    fn reads_the_installed_unit_back() {
        let h = home("read");
        let text = systemd_unit(&ServiceOptions { web_port: 7111, read_only: true, ..opts(&h) }).unwrap();
        let o = installed(&text, &h).unwrap();
        assert_eq!((o.exe, o.web_port, o.web_binds, o.base_path), (h.join(".cargo/bin/agents-tui"), 7111, vec!["127.0.0.1".to_string(), "tailscale".to_string()], String::new()));
        assert_eq!(shell_words(r#""/opt/my apps/agents-tui" --source "a=b" 100%%"#), vec!["/opt/my apps/agents-tui", "--source", "a=b", "100%"]);
    }

    #[test]
    fn under_a_base_path_install_status_and_restart_ask_for_the_village_under_it() {
        let h = home("base-path");
        let mounted = ServiceOptions { base_path: "/x/y".into(), frame_ancestors: Some("'self' https://example.test".into()), ..opts(&h) };
        let mut out = Vec::new();
        install(&mounted, InstallFlags { dry_run: true, linger: false }, "me", &mut up(), &mut out).unwrap();
        assert!(printed(&out).contains("Would wait for the village at http://127.0.0.1:7890/x/y/api/village"), "{}", printed(&out));

        let (mut fake, mut out) = (up(), Vec::new());
        install(&mounted, InstallFlags::default(), "me", &mut fake, &mut out).unwrap();
        assert_eq!(fake.ran.last().unwrap(), "probe 127.0.0.1:7890/x/y");
        assert!(printed(&out).contains("The village: http://127.0.0.1:7890/x/y/village"));
        assert!(printed(&out).contains("The dashboard: http://127.0.0.1:7890/x/y/"));

        let (mut fake, mut out) = (Fake { active: "active".into(), ..up() }, Vec::new());
        status(&h, &mut fake, &mut out).unwrap();
        assert!(fake.ran.contains(&"probe 127.0.0.1:7890/x/y".to_string()), "{:?}", fake.ran);
        assert!(printed(&out).contains("Village: http://127.0.0.1:7890/x/y/village (answering)"), "{}", printed(&out));

        let (mut fake, mut out) = (up(), Vec::new());
        restart(&h, &mut fake, &mut out).unwrap();
        assert_eq!(fake.ran.last().unwrap(), "probe 127.0.0.1:7890/x/y");
        assert!(printed(&out).contains("Restarted. The village: http://127.0.0.1:7890/x/y/village"));

        // Each install writes the options it is given: without the flags, the prefix goes.
        let (mut fake, mut out) = (up(), Vec::new());
        install(&opts(&h), InstallFlags::default(), "me", &mut fake, &mut out).unwrap();
        assert!(printed(&out).contains("--base-path /x/y --frame-ancestors"), "the diff shows them removed: {}", printed(&out));
        assert_eq!(fake.ran.last().unwrap(), "probe 127.0.0.1:7890");
    }

    /// The real probe sends `GET <base path>/api/village` and wants a 200.
    #[test]
    fn the_probe_asks_for_the_api_under_the_base_path() {
        use std::io::{BufRead, BufReader};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            let mut asked = Vec::new();
            for _ in 0..2 {
                let (mut s, _) = listener.accept().unwrap();
                let mut line = String::new();
                BufReader::new(&s).read_line(&mut line).unwrap();
                let ok = line.starts_with("GET /x/y/api/village ");
                s.write_all(if ok { b"HTTP/1.0 200 OK\r\n\r\n" } else { b"HTTP/1.0 404 Not Found\r\n\r\n" }).unwrap();
                asked.push(line.trim().to_string());
            }
            asked
        });
        assert!(SystemRunner.village_answers("127.0.0.1", port, "/x/y"));
        assert!(!SystemRunner.village_answers("127.0.0.1", port, ""));
        assert_eq!(server.join().unwrap(), ["GET /x/y/api/village HTTP/1.0", "GET /api/village HTTP/1.0"]);
    }

    #[test]
    fn diffs_by_line() {
        assert_eq!(line_diff("a\nb\nc\n", "a\nB\nc\nd\n"), vec!["-b", "+B", "+d"]);
        assert!(line_diff("same\n", "same\n").is_empty());
    }
}
