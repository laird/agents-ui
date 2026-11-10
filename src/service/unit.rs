//! The service file for the served agents-tui (#424, #425): a systemd user
//! unit on Linux, a launchd agent on macOS. Pure functions of the options, so
//! `agents-tui service install` (#426, #427) only writes what these return and
//! the tests need no service manager. `docs/agents-tui.service` is what
//! [`systemd_unit`] gives for nuc-1's options; a test keeps the two equal.

use std::path::{Path, PathBuf};

/// The launchd label, and the plist's name under `~/Library/LaunchAgents`.
pub const LAUNCHD_LABEL: &str = "com.agents-ui.agents-tui";
pub const DEFAULT_PORT: u16 = 7890;

/// What the service runs: the server flags, and where.
#[derive(Debug, Clone, PartialEq)]
pub struct ServiceOptions {
    pub web_port: u16,
    /// `--web-bind`, one listener each; `tailscale` expands to this machine's tailnet addresses.
    pub web_binds: Vec<String>,
    /// `--source <layer>=<name>`, each as given, e.g. `swarms=herdr`.
    pub sources: Vec<String>,
    pub read_only: bool,
    /// Serve on every interface (0.0.0.0, ::), which has no authentication.
    pub allow_lan: bool,
    /// The agents-tui binary: `std::env::current_exe()` at the call site.
    pub exe: PathBuf,
    pub home: PathBuf,
    /// The service's PATH, in order ([`default_path`]).
    pub path: Vec<PathBuf>,
}

/// What the installer can find out about this machine, as plain answers so tests can set them.
#[derive(Debug, Clone, Copy, Default)]
pub struct Probes {
    /// `herdr` is on PATH: the swarms most likely run under it.
    pub herdr_on_path: bool,
    /// `tailscale` is installed and up.
    pub tailscale_up: bool,
}

/// The options `service install` starts from when given none.
pub fn defaults(home: &Path, exe: &Path, probes: Probes, exists: impl Fn(&Path) -> bool) -> ServiceOptions {
    ServiceOptions {
        web_port: DEFAULT_PORT,
        web_binds: vec!["127.0.0.1".into()],
        sources: if probes.herdr_on_path { vec!["swarms=herdr".into()] } else { vec![] },
        read_only: false,
        allow_lan: false,
        exe: exe.to_path_buf(),
        home: home.to_path_buf(),
        path: default_path(home, exists),
    }
}

/// A bind worth suggesting over the default, with why, when tailscale is up.
pub fn suggested_bind(probes: Probes) -> Option<(&'static str, &'static str)> {
    probes.tailscale_up.then_some((
        "127.0.0.1,tailscale",
        "tailscale is up: --web-bind 127.0.0.1,tailscale also serves your tailnet devices (not the LAN)",
    ))
}

/// PATH for the service, so `gh`, `git`, `herdr` and `claude` resolve as in a
/// login shell: mise shims, `~/.cargo/bin`, `~/.local/bin` and Homebrew when
/// present, then the system directories.
pub fn default_path(home: &Path, exists: impl Fn(&Path) -> bool) -> Vec<PathBuf> {
    let optional = [
        home.join(".local/share/mise/shims"),
        home.join(".cargo/bin"),
        home.join(".local/bin"),
        PathBuf::from("/opt/homebrew/bin"),
    ];
    let mut path: Vec<PathBuf> = optional.into_iter().filter(|p| exists(p)).collect();
    path.extend(["/usr/local/bin", "/usr/bin", "/bin"].map(PathBuf::from));
    path
}

/// Refuse what would expose the unauthenticated server beyond this machine and the tailnet.
pub fn validate(opts: &ServiceOptions) -> Result<(), String> {
    if opts.web_binds.is_empty() {
        return Err("--web-bind needs at least one address".into());
    }
    let every = opts.web_binds.iter().find(|b| matches!(b.trim(), "0.0.0.0" | "::" | "[::]"));
    if let (Some(b), false) = (every, opts.allow_lan) {
        return Err(format!(
            "--web-bind {b} serves every interface, the LAN included, and agents-tui has no authentication. \
             Use 127.0.0.1 and tailscale instead, or pass --allow-lan if you mean it."
        ));
    }
    Ok(())
}

/// The server's arguments, after the binary.
pub fn exec_args(opts: &ServiceOptions) -> Vec<String> {
    let mut args = vec!["--headless".into(), "--web-port".into(), opts.web_port.to_string(), "--web-bind".into(), opts.web_binds.join(",")];
    for s in &opts.sources {
        args.push("--source".into());
        args.push(s.clone());
    }
    if opts.read_only {
        args.push("--read-only".into());
    }
    args
}

/// A path as systemd reads it in a user unit: under the home directory, `%h/...`.
fn systemd_path(home: &Path, p: &Path) -> String {
    match p.strip_prefix(home) {
        Ok(rest) if !home.as_os_str().is_empty() => format!("%h/{}", rest.display()),
        _ => p.display().to_string(),
    }
}

/// One ExecStart word: quoted when it has a space or a quote; `%` doubled, as systemd expands it.
fn systemd_word(w: &str) -> String {
    let w = w.replace('%', "%%");
    if w.chars().any(|c| c.is_whitespace() || c == '"' || c == '\'' || c == '\\') {
        format!("\"{}\"", w.replace('\\', "\\\\").replace('"', "\\\""))
    } else {
        w
    }
}

fn reach(opts: &ServiceOptions) -> &'static str {
    let tailnet = opts.web_binds.iter().any(|b| b.eq_ignore_ascii_case("tailscale"));
    let local = opts.web_binds.iter().any(|b| matches!(b.as_str(), "127.0.0.1" | "localhost" | "::1"));
    match (local, tailnet) {
        (true, true) => " (localhost + tailnet)",
        (true, false) => " (localhost)",
        (false, true) => " (tailnet)",
        _ => "",
    }
}

/// `~/.config/systemd/user/agents-tui.service`.
pub fn systemd_unit(opts: &ServiceOptions) -> Result<String, String> {
    validate(opts)?;
    // The binary under home is written %h/..., as the reference does; anywhere else, as is.
    let exe = systemd_path(&opts.home, &opts.exe);
    let exe = if exe.starts_with("%h/") { exe } else { systemd_word(&exe) };
    let args: Vec<String> = exec_args(opts).iter().map(|a| systemd_word(a)).collect();
    let path: Vec<String> = opts.path.iter().map(|p| systemd_path(&opts.home, p)).collect();
    Ok(format!(
        "[Unit]
Description=agents-tui village and dashboard{reach}
After=network-online.target

[Service]
WorkingDirectory=%h
Environment=PATH={path}
Environment=RUST_LOG=info
# Localhost plus this machine's tailnet addresses (`tailscale` expands to them):
# reachable from here and your tailscale devices, not the LAN. Keep 127.0.0.1 in
# the list; deploy.sh checks the village there. Add --read-only to serve without
# input, and --source swarms=herdr when the swarms run under herdr.
ExecStart={exe} {args}
# tailscale may not have its address yet at login; keep retrying.
Restart=always
RestartSec=5

[Install]
WantedBy=default.target
",
        reach = reach(opts),
        path = path.join(":"),
        args = args.join(" "),
    ))
}

fn xml(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// `~/Library/LaunchAgents/com.agents-ui.agents-tui.plist`. launchd expands
/// neither `~` nor `%h`, so every path is absolute.
pub fn launchd_plist(opts: &ServiceOptions) -> Result<String, String> {
    validate(opts)?;
    let mut program = vec![opts.exe.display().to_string()];
    program.extend(exec_args(opts));
    let program: String = program.iter().map(|a| format!("\n\t\t<string>{}</string>", xml(a))).collect();
    let path: Vec<String> = opts.path.iter().map(|p| p.display().to_string()).collect();
    let logs = opts.home.join("Library/Logs/agents-tui");
    Ok(format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>Label</key>
	<string>{label}</string>
	<key>ProgramArguments</key>
	<array>{program}
	</array>
	<key>WorkingDirectory</key>
	<string>{home}</string>
	<key>EnvironmentVariables</key>
	<dict>
		<key>PATH</key>
		<string>{path}</string>
		<key>RUST_LOG</key>
		<string>info</string>
	</dict>
	<key>RunAtLoad</key>
	<true/>
	<key>KeepAlive</key>
	<true/>
	<key>StandardOutPath</key>
	<string>{out}</string>
	<key>StandardErrorPath</key>
	<string>{err}</string>
</dict>
</plist>
"#,
        label = LAUNCHD_LABEL,
        home = xml(&opts.home.display().to_string()),
        path = xml(&path.join(":")),
        out = xml(&logs.join("agents-tui.log").display().to_string()),
        err = xml(&logs.join("agents-tui.err.log").display().to_string()),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOME: &str = "/home/u";
    fn all_home_dirs(p: &Path) -> bool {
        p.starts_with(HOME)
    }
    fn opts() -> ServiceOptions {
        defaults(Path::new(HOME), Path::new("/home/u/.cargo/bin/agents-tui"), Probes::default(), all_home_dirs)
    }

    /// docs/agents-tui.service is the unit for nuc-1's options, from `[Unit]`
    /// on (above it are the doc's install notes). They cannot drift apart.
    #[test]
    fn the_reference_unit_is_what_the_generator_gives_for_nuc_1() {
        let o = ServiceOptions {
            web_binds: vec!["127.0.0.1".into(), "tailscale".into()],
            sources: vec!["swarms=herdr".into()],
            ..opts()
        };
        let reference = include_str!("../../docs/agents-tui.service");
        let body = &reference[reference.find("[Unit]").expect("the reference has a [Unit] section")..];
        assert_eq!(systemd_unit(&o).unwrap(), body);
    }

    #[test]
    fn defaults_are_localhost_on_7890_and_herdr_when_it_is_there() {
        let o = opts();
        assert_eq!((o.web_port, o.web_binds.clone(), o.sources.clone(), o.read_only), (7890, vec!["127.0.0.1".to_string()], vec![], false));
        let h = defaults(Path::new(HOME), &o.exe, Probes { herdr_on_path: true, tailscale_up: false }, all_home_dirs);
        assert_eq!(h.sources, vec!["swarms=herdr"]);
        let unit = systemd_unit(&o).unwrap();
        assert!(unit.contains("Description=agents-tui village and dashboard (localhost)\n"));
        assert!(unit.contains("ExecStart=%h/.cargo/bin/agents-tui --headless --web-port 7890 --web-bind 127.0.0.1\n"), "{unit}");
    }

    #[test]
    fn tailscale_is_suggested_not_assumed() {
        assert_eq!(suggested_bind(Probes::default()), None);
        let (bind, why) = suggested_bind(Probes { tailscale_up: true, herdr_on_path: false }).unwrap();
        assert_eq!(bind, "127.0.0.1,tailscale");
        assert!(why.contains("not the LAN"));
        let o = defaults(Path::new(HOME), Path::new("/x/agents-tui"), Probes { tailscale_up: true, herdr_on_path: false }, all_home_dirs);
        assert_eq!(o.web_binds, vec!["127.0.0.1"]);
    }

    #[test]
    fn read_only_and_sources_reach_the_command_line() {
        let o = ServiceOptions { read_only: true, sources: vec!["swarms=herdr".into(), "issues=none".into()], web_port: 9000, ..opts() };
        assert_eq!(
            exec_args(&o),
            ["--headless", "--web-port", "9000", "--web-bind", "127.0.0.1", "--source", "swarms=herdr", "--source", "issues=none", "--read-only"]
        );
        assert!(systemd_unit(&o).unwrap().contains("--source swarms=herdr --source issues=none --read-only\n"));
        assert!(launchd_plist(&o).unwrap().contains("<string>--read-only</string>"));
    }

    #[test]
    fn path_has_the_login_shell_dirs_that_exist_then_the_system_ones() {
        let all = default_path(Path::new(HOME), all_home_dirs);
        let s: Vec<String> = all.iter().map(|p| p.display().to_string()).collect();
        assert_eq!(s, ["/home/u/.local/share/mise/shims", "/home/u/.cargo/bin", "/home/u/.local/bin", "/usr/local/bin", "/usr/bin", "/bin"]);
        let none = default_path(Path::new(HOME), |_| false);
        assert_eq!(none, ["/usr/local/bin", "/usr/bin", "/bin"].map(PathBuf::from));
        let cargo_only = default_path(Path::new(HOME), |p| p.ends_with(".cargo/bin"));
        assert_eq!(cargo_only[0], PathBuf::from("/home/u/.cargo/bin"));
        assert_eq!(cargo_only.len(), 4);
        let brew = default_path(Path::new("/Users/u"), |p| p == Path::new("/opt/homebrew/bin"));
        assert_eq!(brew[0], PathBuf::from("/opt/homebrew/bin"));
        let unit = systemd_unit(&ServiceOptions { path: none, ..opts() }).unwrap();
        assert!(unit.contains("Environment=PATH=/usr/local/bin:/usr/bin:/bin\n"));
    }

    #[test]
    fn every_interface_is_refused_without_allow_lan() {
        for bind in ["0.0.0.0", "::", "[::]"] {
            let o = ServiceOptions { web_binds: vec!["127.0.0.1".into(), bind.into()], ..opts() };
            let e = systemd_unit(&o).unwrap_err();
            assert!(e.contains("no authentication") && e.contains("--allow-lan"), "{e}");
            assert!(launchd_plist(&o).is_err());
            assert!(systemd_unit(&ServiceOptions { allow_lan: true, ..o }).is_ok());
        }
        assert!(validate(&ServiceOptions { web_binds: vec![], ..opts() }).is_err());
    }

    #[test]
    fn a_binary_outside_home_keeps_its_absolute_path() {
        let o = ServiceOptions { exe: "/opt/agents tui/agents-tui".into(), ..opts() };
        assert!(systemd_unit(&o).unwrap().contains("ExecStart=\"/opt/agents tui/agents-tui\" --headless"));
        assert_eq!(systemd_word("50%"), "50%%");
    }

    #[test]
    fn the_launchd_agent_runs_at_load_keeps_alive_and_uses_absolute_paths() {
        let o = ServiceOptions {
            home: "/Users/u".into(),
            exe: "/Users/u/.cargo/bin/agents-tui".into(),
            path: default_path(Path::new("/Users/u"), |p| p.starts_with("/Users/u")),
            web_binds: vec!["127.0.0.1".into(), "tailscale".into()],
            ..opts()
        };
        let p = launchd_plist(&o).unwrap();
        assert!(p.contains("<string>com.agents-ui.agents-tui</string>"));
        assert!(p.contains("<key>RunAtLoad</key>\n\t<true/>") && p.contains("<key>KeepAlive</key>\n\t<true/>"));
        assert!(p.contains("\n\t\t<string>/Users/u/.cargo/bin/agents-tui</string>\n\t\t<string>--headless</string>"));
        assert!(p.contains("<string>127.0.0.1,tailscale</string>"));
        assert!(p.contains("<string>/Users/u/.local/share/mise/shims:/Users/u/.cargo/bin:/Users/u/.local/bin:/usr/local/bin:/usr/bin:/bin</string>"));
        assert!(p.contains("<string>/Users/u/Library/Logs/agents-tui/agents-tui.log</string>"));
        assert!(!p.contains("%h") && !p.contains('~'), "launchd expands neither");
        let amp = launchd_plist(&ServiceOptions { sources: vec!["swarms=a&b".into()], ..o }).unwrap();
        assert!(amp.contains("<string>swarms=a&amp;b</string>"));
    }
}
