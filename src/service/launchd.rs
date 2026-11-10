//! `agents-tui service` on macOS: the served agents-tui as a launchd agent
//! (#427). `install` writes [`super::unit::launchd_plist`]'s text to
//! `~/Library/LaunchAgents/com.agents-ui.agents-tui.plist` and bootstraps it
//! into the user's GUI domain (`gui/<uid>`), booting out any older one
//! first, so re-running it is safe. There is no linger: a LaunchAgent runs
//! while the user is logged in.
//!
//! The same [`Runner`] as systemd, so these tests run on any OS.

use std::io::Write;
use std::path::{Path, PathBuf};

use super::systemd::{InstallFlags, Runner, WAIT, line_diff, reach_host, served, url, wait_for_village};
use super::unit::{LAUNCHD_LABEL, ServiceOptions, launchd_plist, validate};

/// `~/Library/LaunchAgents/com.agents-ui.agents-tui.plist`.
pub fn plist_path(home: &Path) -> PathBuf {
    home.join("Library/LaunchAgents").join(format!("{LAUNCHD_LABEL}.plist"))
}

/// Where the plist sends the server's output.
pub fn logs_dir(home: &Path) -> PathBuf {
    home.join("Library/Logs/agents-tui")
}

/// The agent's domain target for launchctl: `gui/501/com.agents-ui.agents-tui`.
fn target(uid: u32) -> String {
    format!("gui/{uid}/{LAUNCHD_LABEL}")
}

const NO_LINGER: &str = "macOS has no lingering: a LaunchAgent runs while you are logged in, and starts again when you log in.";

/// Write the plist, (re)load it, wait for the village, and say where it is.
/// A dry run prints the plist, what would change and the commands only.
pub fn install(opts: &ServiceOptions, flags: InstallFlags, uid: u32, runner: &mut dyn Runner, out: &mut dyn Write) -> Result<(), String> {
    validate(opts)?;
    let text = launchd_plist(opts)?;
    let path = plist_path(&opts.home);
    let old = std::fs::read_to_string(&path).ok();
    let path_str = path.to_string_lossy().into_owned();
    let say = |out: &mut dyn Write, s: String| writeln!(out, "{s}").map_err(|e| e.to_string());
    let diff = |out: &mut dyn Write, heading: String| -> Result<(), String> {
        if let Some(old) = old.as_ref().filter(|o| **o != text) {
            say(out, heading)?;
            for line in line_diff(old, &text) {
                say(out, format!("  {line}"))?;
            }
        }
        Ok(())
    };

    if flags.dry_run {
        say(out, format!("Would write {path_str}:\n"))?;
        say(out, text.clone())?;
        diff(out, "Changes from the installed agent:".into())?;
        say(out, "Would run:".into())?;
        say(out, format!("  launchctl bootout {}", target(uid)))?;
        say(out, format!("  launchctl bootstrap gui/{uid} {path_str}"))?;
        if flags.linger {
            say(out, NO_LINGER.into())?;
        }
        return Ok(());
    }

    match &old {
        Some(o) if *o != text => diff(out, format!("Updating {path_str}:"))?,
        Some(_) => say(out, format!("{path_str} is unchanged."))?,
        None => say(out, format!("Writing {path_str}"))?,
    }
    for dir in [path.parent().map(Path::to_path_buf), Some(logs_dir(&opts.home))].into_iter().flatten() {
        std::fs::create_dir_all(&dir).map_err(|e| format!("can't create {}: {e}", dir.display()))?;
    }
    std::fs::write(&path, &text).map_err(|e| format!("can't write {path_str}: {e}"))?;

    // Boot out whatever is loaded (an older plist, or none: that fails, harmlessly).
    runner.run("launchctl", &["bootout", &target(uid)]);
    let domain = format!("gui/{uid}");
    if !runner.run("launchctl", &["bootstrap", &domain, &path_str]).ok {
        return Err(format!("launchctl bootstrap {domain} {path_str} failed"));
    }
    if flags.linger {
        say(out, NO_LINGER.into())?;
    }

    let host = reach_host(opts, runner);
    if !wait_for_village(runner, &host, opts.web_port) {
        return Err(format!(
            "the agent loaded, but the village didn't answer at {}/api/village within {}s. See {}",
            url(&host, opts.web_port),
            WAIT.as_secs(),
            logs_dir(&opts.home).join("agents-tui.err.log").display()
        ));
    }
    say(out, format!("The village: {}/village", url(&host, opts.web_port)))?;
    say(out, format!("The dashboard: {}/", url(&host, opts.web_port)))?;
    Ok(())
}

/// Boot the agent out and remove its plist.
pub fn uninstall(home: &Path, uid: u32, runner: &mut dyn Runner, out: &mut dyn Write) -> Result<(), String> {
    runner.run("launchctl", &["bootout", &target(uid)]);
    let path = plist_path(home);
    match std::fs::remove_file(&path) {
        Ok(()) => writeln!(out, "Removed {}", path.display()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => writeln!(out, "{} was not installed.", path.display()),
        Err(e) => return Err(format!("can't remove {}: {e}", path.display())),
    }
    .map_err(|e| e.to_string())
}

/// The `<string>`s of the plist's ProgramArguments, unescaped.
pub fn program_arguments(plist: &str) -> Vec<String> {
    let Some(start) = plist.find("<key>ProgramArguments</key>") else { return Vec::new() };
    let rest = &plist[start..];
    let Some(end) = rest.find("</array>") else { return Vec::new() };
    rest[..end]
        .split("<string>")
        .skip(1)
        .filter_map(|s| s.split_once("</string>").map(|(v, _)| v))
        .map(|v| v.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&amp;", "&"))
        .collect()
}

/// The agent's state from `launchctl print`: `state = running`, and its pid.
pub fn print_state(print: &str) -> (Option<String>, Option<u32>) {
    let field = |name: &str| {
        print.lines().find_map(|l| l.trim().strip_prefix(name).and_then(|r| r.trim_start().strip_prefix('=')).map(|v| v.trim().to_string()))
    };
    (field("state"), field("pid").and_then(|p| p.parse().ok()))
}

/// The agent's state, where it serves, whether the village answers, and the
/// version of the binary it runs.
pub fn status(home: &Path, uid: u32, runner: &mut dyn Runner, out: &mut dyn Write) -> Result<(), String> {
    let say = |out: &mut dyn Write, s: String| writeln!(out, "{s}").map_err(|e| e.to_string());
    let path = plist_path(home);
    let Ok(text) = std::fs::read_to_string(&path) else {
        return say(out, format!("{LAUNCHD_LABEL} is not installed (no {}). Install it with: agents-tui service install", path.display()));
    };
    let printed = runner.run("launchctl", &["print", &target(uid)]);
    let loaded = if !printed.ok {
        "not loaded".to_string()
    } else {
        match print_state(&printed.stdout) {
            (Some(state), Some(pid)) => format!("{state}, pid {pid}"),
            (Some(state), None) => state,
            _ => "loaded".into(),
        }
    };
    say(out, format!("{LAUNCHD_LABEL}: {loaded}"))?;
    say(out, format!("Agent file: {}", path.display()))?;
    let args = program_arguments(&text);
    let Some(exe) = args.first().map(PathBuf::from) else {
        return say(out, "The plist has no ProgramArguments agents-tui can read.".into());
    };
    let (port, binds) = served(&args);
    let opts = ServiceOptions { web_port: port, web_binds: binds, sources: vec![], read_only: false, allow_lan: false, exe: exe.clone(), home: home.to_path_buf(), path: vec![] };
    let host = reach_host(&opts, runner);
    let answers = runner.village_answers(&host, port);
    say(out, format!("Village: {}/village ({})", url(&host, port), if answers { "answering" } else { "not answering" }))?;
    let version = runner.run(&exe.to_string_lossy(), &["--version"]);
    let said = match (version.ok, exe.exists()) {
        (true, _) => version.stdout.trim().to_string(),
        (false, true) => "doesn't report a version".into(),
        (false, false) => "missing".into(),
    };
    say(out, format!("Binary: {} ({said})", exe.display()))?;
    say(out, format!("Logs: {}", logs_dir(home).display()))
}

/// Restart the agent (`kickstart -k`) and wait for the village.
pub fn restart(home: &Path, uid: u32, runner: &mut dyn Runner, out: &mut dyn Write) -> Result<(), String> {
    let text = std::fs::read_to_string(plist_path(home)).map_err(|_| format!("{LAUNCHD_LABEL} is not installed. Install it with: agents-tui service install"))?;
    if !runner.run("launchctl", &["kickstart", "-k", &target(uid)]).ok {
        return Err(format!("launchctl kickstart -k {} failed", target(uid)));
    }
    let args = program_arguments(&text);
    let (port, binds) = served(&args);
    let opts = ServiceOptions {
        web_port: port,
        web_binds: binds,
        sources: vec![],
        read_only: false,
        allow_lan: false,
        exe: args.first().map(PathBuf::from).unwrap_or_default(),
        home: home.to_path_buf(),
        path: vec![],
    };
    let host = reach_host(&opts, runner);
    if !wait_for_village(runner, &host, port) {
        return Err(format!("restarted, but the village didn't answer at {}/api/village within {}s", url(&host, port), WAIT.as_secs()));
    }
    writeln!(out, "Restarted. The village: {}/village", url(&host, port)).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::super::systemd::Output;
    use super::*;
    use std::time::Duration;

    /// Records each command; launchctl answers from a script.
    #[derive(Default)]
    struct Fake {
        ran: Vec<String>,
        answers: bool,
        /// `launchctl print`'s output, or None when the agent isn't loaded.
        print: Option<String>,
        bootstrap_fails: bool,
        version: Option<String>,
    }

    impl Runner for Fake {
        fn run(&mut self, program: &str, args: &[&str]) -> Output {
            self.ran.push(format!("{program} {}", args.join(" ")));
            match (program, args) {
                ("launchctl", ["print", ..]) => Output { ok: self.print.is_some(), stdout: self.print.clone().unwrap_or_default() },
                ("launchctl", ["bootstrap", ..]) => Output { ok: !self.bootstrap_fails, stdout: String::new() },
                (_, ["--version"]) => Output { ok: self.version.is_some(), stdout: self.version.clone().unwrap_or_default() },
                _ => Output { ok: true, stdout: String::new() },
            }
        }
        fn village_answers(&mut self, host: &str, port: u16) -> bool {
            self.ran.push(format!("probe {host}:{port}"));
            self.answers
        }
        fn sleep(&mut self, _: Duration) {}
    }

    fn up() -> Fake {
        Fake { answers: true, ..Fake::default() }
    }

    fn home(name: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let dir = std::env::temp_dir().join(format!("agents-ui-launchd-{name}-{}-{nanos}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn opts(home: &Path) -> ServiceOptions {
        ServiceOptions {
            web_port: 7890,
            web_binds: vec!["127.0.0.1".into(), "tailscale".into()],
            sources: vec![],
            read_only: false,
            allow_lan: false,
            exe: home.join(".cargo/bin/agents-tui"),
            home: home.to_path_buf(),
            path: vec![PathBuf::from("/opt/homebrew/bin"), PathBuf::from("/usr/bin")],
        }
    }

    fn printed(out: &[u8]) -> String {
        String::from_utf8_lossy(out).into_owned()
    }

    #[test]
    fn install_writes_the_plist_then_boots_out_and_bootstraps() {
        let h = home("install");
        let (mut fake, mut out) = (up(), Vec::new());
        install(&opts(&h), InstallFlags::default(), 501, &mut fake, &mut out).unwrap();
        let path = plist_path(&h);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), launchd_plist(&opts(&h)).unwrap());
        assert!(logs_dir(&h).is_dir(), "the log directory exists for launchd to write into");
        assert_eq!(
            fake.ran,
            vec![
                "launchctl bootout gui/501/com.agents-ui.agents-tui".to_string(),
                format!("launchctl bootstrap gui/501 {}", path.display()),
                "probe 127.0.0.1:7890".to_string(),
            ]
        );
        assert!(printed(&out).contains("The village: http://127.0.0.1:7890/village"));
    }

    #[test]
    fn reinstalling_shows_a_diff_and_reloads() {
        let h = home("reinstall");
        install(&opts(&h), InstallFlags::default(), 501, &mut up(), &mut Vec::new()).unwrap();
        let (mut fake, mut out) = (up(), Vec::new());
        install(&ServiceOptions { web_port: 7999, read_only: true, ..opts(&h) }, InstallFlags::default(), 501, &mut fake, &mut out).unwrap();
        let said = printed(&out);
        assert!(said.contains("Updating") && said.contains("-\t\t<string>7890</string>") && said.contains("+\t\t<string>7999</string>"), "{said}");
        assert!(said.contains("+\t\t<string>--read-only</string>"), "{said}");
        assert_eq!(fake.ran[0], "launchctl bootout gui/501/com.agents-ui.agents-tui");
        assert!(fake.ran[1].starts_with("launchctl bootstrap gui/501 "));
    }

    #[test]
    fn a_dry_run_changes_nothing_and_says_there_is_no_linger() {
        let h = home("dry");
        let (mut fake, mut out) = (up(), Vec::new());
        install(&opts(&h), InstallFlags { dry_run: true, linger: true }, 501, &mut fake, &mut out).unwrap();
        assert!(fake.ran.is_empty() && !plist_path(&h).exists());
        let said = printed(&out);
        assert!(said.contains("<key>ProgramArguments</key>") && said.contains("launchctl bootstrap gui/501"));
        assert!(said.contains("no lingering"), "{said}");
    }

    #[test]
    fn a_failed_bootstrap_or_a_silent_village_is_an_error() {
        let h = home("fail");
        let err = install(&opts(&h), InstallFlags::default(), 501, &mut Fake { bootstrap_fails: true, answers: true, ..Fake::default() }, &mut Vec::new()).unwrap_err();
        assert!(err.contains("bootstrap"), "{err}");
        let err = install(&opts(&h), InstallFlags::default(), 501, &mut Fake::default(), &mut Vec::new()).unwrap_err();
        assert!(err.contains("didn't answer") && err.contains("agents-tui.err.log"), "{err}");
        let lan = ServiceOptions { web_binds: vec!["0.0.0.0".into()], ..opts(&h) };
        assert!(install(&lan, InstallFlags::default(), 501, &mut up(), &mut Vec::new()).unwrap_err().contains("--allow-lan"));
    }

    #[test]
    fn uninstall_boots_out_and_removes() {
        let h = home("uninstall");
        install(&opts(&h), InstallFlags::default(), 501, &mut up(), &mut Vec::new()).unwrap();
        let mut fake = up();
        uninstall(&h, 501, &mut fake, &mut Vec::new()).unwrap();
        assert_eq!(fake.ran, vec!["launchctl bootout gui/501/com.agents-ui.agents-tui"]);
        assert!(!plist_path(&h).exists());
        let mut out = Vec::new();
        uninstall(&h, 501, &mut up(), &mut out).unwrap();
        assert!(printed(&out).contains("was not installed"));
    }

    #[test]
    fn status_reads_launchctl_print_and_the_plist() {
        let h = home("status");
        install(&opts(&h), InstallFlags::default(), 501, &mut up(), &mut Vec::new()).unwrap();
        let print = "gui/501/com.agents-ui.agents-tui = {\n\tactive count = 1\n\tpath = /Users/me/Library/LaunchAgents/com.agents-ui.agents-tui.plist\n\tstate = running\n\n\tprogram = /Users/me/.cargo/bin/agents-tui\n\tpid = 4242\n}\n";
        let mut fake = Fake { answers: true, print: Some(print.into()), version: Some("agents-tui 0.1.0\n".into()), ..Fake::default() };
        let mut out = Vec::new();
        status(&h, 501, &mut fake, &mut out).unwrap();
        let said = printed(&out);
        assert!(said.contains("com.agents-ui.agents-tui: running, pid 4242"), "{said}");
        assert!(said.contains("http://127.0.0.1:7890/village (answering)") && said.contains("(agents-tui 0.1.0)"), "{said}");
        assert_eq!(fake.ran[0], "launchctl print gui/501/com.agents-ui.agents-tui");

        let mut out = Vec::new();
        status(&h, 501, &mut Fake::default(), &mut out).unwrap();
        let said = printed(&out);
        assert!(said.contains("com.agents-ui.agents-tui: not loaded") && said.contains("(not answering)") && said.contains("(missing)"), "{said}");

        let mut out = Vec::new();
        status(&home("status-none"), 501, &mut up(), &mut out).unwrap();
        assert!(printed(&out).contains("is not installed"));
    }

    #[test]
    fn restart_kickstarts_then_waits() {
        let h = home("restart");
        install(&ServiceOptions { web_port: 7333, ..opts(&h) }, InstallFlags::default(), 501, &mut up(), &mut Vec::new()).unwrap();
        let (mut fake, mut out) = (up(), Vec::new());
        restart(&h, 501, &mut fake, &mut out).unwrap();
        assert_eq!(fake.ran, vec!["launchctl kickstart -k gui/501/com.agents-ui.agents-tui", "probe 127.0.0.1:7333"]);
        assert!(printed(&out).contains("Restarted. The village: http://127.0.0.1:7333/village"));
        assert!(restart(&home("restart-none"), 501, &mut up(), &mut Vec::new()).unwrap_err().contains("not installed"));
    }

    #[test]
    fn reads_the_plist_back() {
        let h = home("read");
        let text = launchd_plist(&ServiceOptions { web_port: 7111, sources: vec!["swarms=a&b".into()], ..opts(&h) }).unwrap();
        let args = program_arguments(&text);
        assert_eq!(args[0], h.join(".cargo/bin/agents-tui").to_string_lossy());
        assert!(args.contains(&"swarms=a&b".to_string()), "unescaped: {args:?}");
        assert_eq!(served(&args), (7111, vec!["127.0.0.1".to_string(), "tailscale".to_string()]));
        assert_eq!(print_state("\tstate = waiting\n"), (Some("waiting".into()), None));
    }
}
