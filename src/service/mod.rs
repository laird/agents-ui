//! `agents-tui service`: the served agents-tui as a user service (#424).
//! The service file's text is [`unit`]'s (#425). On Linux [`systemd`] writes
//! it and drives systemd (#426); on macOS [`launchd`] writes the agent's
//! plist and drives launchctl (#427). Which one is a [`Backend`], so either
//! is tested on any OS. `--with-herdr` adds the herdr server's own systemd
//! unit, [`herdr`] (#428).
pub mod herdr;
pub mod launchd;
pub mod systemd;
pub mod unit;

use std::io::Write;
use std::path::Path;

use unit::{Probes, ServiceOptions, defaults, suggested_bind};

const USAGE: &str = "usage: agents-tui service <install|uninstall|status|restart>
  install [--web-port N] [--web-bind ADDR[,ADDR]] [--source LAYER=NAME]... [--read-only]
          [--allow-lan] [--linger] [--with-herdr] [--dry-run]
    Write the user service, enable and start it, and wait until the village answers.
    Run it again to change the options: it shows what changed and restarts.
    --linger (Linux) keeps it running after you log out. macOS has no such thing:
    a LaunchAgent runs while you are logged in.
    --with-herdr (Linux) also installs herdr.service for the herdr server. A herdr
    already running is never restarted (that ends every pane): the unit is enabled,
    and takes over at the next login or once that herdr exits.
  uninstall [--with-herdr [--stop-herdr]]
    Stop, disable and remove the service. --with-herdr removes herdr.service too,
    leaving a running herdr running unless --stop-herdr.
  status      The service's state, where it serves, and the binary it runs.
  restart     Restart the service and wait for the village.";

/// What `service` was asked to do.
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Install { set: Overrides, flags: systemd::InstallFlags, with_herdr: bool },
    Uninstall { with_herdr: bool, stop_herdr: bool },
    Status,
    Restart,
}

/// The server options given on the command line, over the defaults.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Overrides {
    pub web_port: Option<u16>,
    pub web_binds: Vec<String>,
    pub sources: Vec<String>,
    pub read_only: bool,
    pub allow_lan: bool,
}

impl Overrides {
    pub fn apply(&self, mut opts: ServiceOptions) -> ServiceOptions {
        if let Some(p) = self.web_port {
            opts.web_port = p;
        }
        if !self.web_binds.is_empty() {
            opts.web_binds = self.web_binds.clone();
        }
        if !self.sources.is_empty() {
            opts.sources = self.sources.clone();
        }
        opts.read_only |= self.read_only;
        opts.allow_lan |= self.allow_lan;
        opts
    }
}

/// Parse the words after `service`.
pub fn parse(args: &[String]) -> Result<Command, String> {
    let (verb, rest) = args.split_first().ok_or_else(|| USAGE.to_string())?;
    let simple = |c: Command| if rest.is_empty() { Ok(c) } else { Err(format!("`service {verb}` takes no options\n{USAGE}")) };
    match verb.as_str() {
        "uninstall" => {
            let (mut with_herdr, mut stop_herdr) = (false, false);
            for arg in rest {
                match arg.as_str() {
                    "--with-herdr" => with_herdr = true,
                    "--stop-herdr" => stop_herdr = true,
                    other => return Err(format!("unknown option {other}\n{USAGE}")),
                }
            }
            if stop_herdr && !with_herdr {
                return Err(format!("--stop-herdr goes with --with-herdr\n{USAGE}"));
            }
            Ok(Command::Uninstall { with_herdr, stop_herdr })
        }
        "status" => simple(Command::Status),
        "restart" => simple(Command::Restart),
        "install" => {
            let (mut set, mut flags, mut with_herdr) = (Overrides::default(), systemd::InstallFlags::default(), false);
            let mut it = rest.iter();
            while let Some(arg) = it.next() {
                let (name, inline) = match arg.split_once('=') {
                    Some((n, v)) if n.starts_with("--") => (n, Some(v.to_string())),
                    _ => (arg.as_str(), None),
                };
                let mut value = || inline.clone().or_else(|| it.next().cloned()).ok_or_else(|| format!("{name} needs a value"));
                match name {
                    "--web-port" => set.web_port = Some(value()?.parse().map_err(|_| "--web-port needs a port number".to_string())?),
                    "--web-bind" => set.web_binds.extend(value()?.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty())),
                    "--source" => set.sources.push(value()?),
                    "--read-only" => set.read_only = true,
                    "--allow-lan" => set.allow_lan = true,
                    "--linger" => flags.linger = true,
                    "--dry-run" => flags.dry_run = true,
                    "--with-herdr" => with_herdr = true,
                    other => return Err(format!("unknown option {other}\n{USAGE}")),
                }
            }
            Ok(Command::Install { set, flags, with_herdr })
        }
        "-h" | "--help" | "help" => Err(USAGE.to_string()),
        other => Err(format!("unknown service command {other}\n{USAGE}")),
    }
}

fn find_on_path(program: &str) -> Option<std::path::PathBuf> {
    std::env::split_paths(&std::env::var_os("PATH")?).map(|d| d.join(program)).find(|p| p.is_file())
}

fn on_path(program: &str) -> bool {
    find_on_path(program).is_some()
}

/// `agents-tui service ...`: run it, print to `out`, and return the exit code.
pub fn run(args: &[String], out: &mut dyn Write) -> i32 {
    let command = match parse(args) {
        Ok(c) => c,
        Err(e) => {
            let _ = writeln!(out, "{e}");
            return 2;
        }
    };
    let Some(home) = dirs::home_dir() else {
        let _ = writeln!(out, "can't find your home directory");
        return 1;
    };
    let mut runner = systemd::SystemRunner;
    let backend = if cfg!(target_os = "macos") {
        let id = systemd::Runner::run(&mut runner, "id", &["-u"]);
        match id.stdout.trim().parse() {
            Ok(uid) => Backend::Launchd { uid },
            Err(_) => {
                let _ = writeln!(out, "can't find your user id (`id -u`)");
                return 1;
            }
        }
    } else if cfg!(target_os = "linux") {
        Backend::Systemd
    } else {
        let _ = writeln!(out, "agents-tui service runs under systemd (Linux) or launchd (macOS).");
        return 1;
    };
    let result = dispatch(command, backend, &home, &mut runner, out);
    match result {
        Ok(()) => 0,
        Err(e) => {
            let _ = writeln!(out, "agents-tui service: {e}");
            1
        }
    }
}

/// The service manager `service` drives.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Backend {
    Systemd,
    /// launchd, in the GUI domain of this user id.
    Launchd { uid: u32 },
}

const HERDR_SYSTEMD_ONLY: &str = "--with-herdr installs a systemd unit; it is for Linux only";

fn dispatch(command: Command, backend: Backend, home: &Path, runner: &mut systemd::SystemRunner, out: &mut dyn Write) -> Result<(), String> {
    match (command, backend) {
        (Command::Install { set, flags, with_herdr }, _) => install(home, &set, flags, with_herdr, backend, runner, out),
        (Command::Uninstall { with_herdr, stop_herdr }, Backend::Systemd) => systemd::uninstall(home, runner, out)
            .and_then(|()| if with_herdr { herdr::uninstall(home, stop_herdr, runner, out) } else { Ok(()) }),
        (Command::Uninstall { with_herdr: true, .. }, Backend::Launchd { .. }) => Err(HERDR_SYSTEMD_ONLY.into()),
        (Command::Uninstall { .. }, Backend::Launchd { uid }) => launchd::uninstall(home, uid, runner, out),
        (Command::Status, Backend::Systemd) => systemd::status(home, runner, out),
        (Command::Status, Backend::Launchd { uid }) => launchd::status(home, uid, runner, out),
        (Command::Restart, Backend::Systemd) => systemd::restart(home, runner, out),
        (Command::Restart, Backend::Launchd { uid }) => launchd::restart(home, uid, runner, out),
    }
}

#[allow(clippy::too_many_arguments)]
fn install(home: &Path, set: &Overrides, flags: systemd::InstallFlags, with_herdr: bool, backend: Backend, runner: &mut systemd::SystemRunner, out: &mut dyn Write) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| format!("can't find this binary: {e}"))?;
    let probes = Probes {
        herdr_on_path: on_path("herdr"),
        tailscale_up: on_path("tailscale") && systemd::Runner::run(runner, "tailscale", &["status"]).ok,
    };
    let opts = set.apply(defaults(home, &exe, probes, |p| p.exists()));
    if set.web_binds.is_empty()
        && let Some((bind, why)) = suggested_bind(probes)
    {
        let _ = writeln!(out, "Tip: {why}. Re-run with --web-bind {bind} to do that.");
    }
    match backend {
        Backend::Systemd => {
            // herdr first: the village's swarms=herdr source reads from it.
            if with_herdr {
                let herdr = find_on_path("herdr").ok_or("--with-herdr: herdr is not on PATH")?;
                herdr::install(home, &herdr, &opts.path, flags.dry_run, runner, out)?;
            }
            let user = std::env::var("USER").unwrap_or_else(|_| "$USER".into());
            systemd::install(&opts, flags, &user, runner, out)
        }
        Backend::Launchd { .. } if with_herdr => Err(HERDR_SYSTEMD_ONLY.into()),
        Backend::Launchd { uid } => launchd::install(&opts, flags, uid, runner, out),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(s: &str) -> Vec<String> {
        s.split_whitespace().map(str::to_string).collect()
    }

    #[test]
    fn parses_install_options() {
        let c = parse(&words("install --web-port 7999 --web-bind 127.0.0.1,tailscale --source swarms=herdr --read-only --linger --dry-run")).unwrap();
        let Command::Install { set, flags, with_herdr } = c else { panic!("{c:?}") };
        assert!(!with_herdr);
        assert_eq!(set.web_port, Some(7999));
        assert_eq!(set.web_binds, vec!["127.0.0.1", "tailscale"]);
        assert_eq!(set.sources, vec!["swarms=herdr"]);
        assert!(set.read_only && !set.allow_lan && flags.linger && flags.dry_run);
        let Command::Install { set, .. } = parse(&words("install --web-port=7111 --web-bind=::1")).unwrap() else { panic!() };
        assert_eq!((set.web_port, set.web_binds), (Some(7111), vec!["::1".to_string()]));
    }

    #[test]
    fn parses_the_other_commands_and_refuses_nonsense() {
        assert_eq!(parse(&words("status")).unwrap(), Command::Status);
        assert_eq!(parse(&words("restart")).unwrap(), Command::Restart);
        assert_eq!(parse(&words("uninstall")).unwrap(), Command::Uninstall { with_herdr: false, stop_herdr: false });
        assert!(parse(&words("status --now")).unwrap_err().contains("takes no options"));
        assert!(parse(&words("install --web-port")).unwrap_err().contains("needs a value"));
        assert!(parse(&words("install --web-port nine")).unwrap_err().contains("port number"));
        assert!(parse(&words("install --verbose")).unwrap_err().contains("unknown option"));
        assert!(parse(&words("frobnicate")).unwrap_err().contains("unknown service command"));
        assert!(parse(&[]).unwrap_err().starts_with("usage:"));
    }

    #[test]
    fn with_herdr_on_install_and_uninstall() {
        let Command::Install { with_herdr, flags, .. } = parse(&words("install --with-herdr --dry-run")).unwrap() else { panic!() };
        assert!(with_herdr && flags.dry_run);
        assert_eq!(parse(&words("uninstall --with-herdr")).unwrap(), Command::Uninstall { with_herdr: true, stop_herdr: false });
        assert_eq!(parse(&words("uninstall --with-herdr --stop-herdr")).unwrap(), Command::Uninstall { with_herdr: true, stop_herdr: true });
        assert!(parse(&words("uninstall --stop-herdr")).unwrap_err().contains("goes with --with-herdr"));
        assert!(parse(&words("uninstall --now")).unwrap_err().contains("unknown option"));
    }

    #[test]
    fn options_given_override_the_defaults() {
        let base = defaults(Path::new("/home/me"), Path::new("/home/me/.cargo/bin/agents-tui"), Probes { herdr_on_path: true, tailscale_up: false }, |_| false);
        let set = Overrides { web_port: Some(7999), web_binds: vec!["tailscale".into()], read_only: true, ..Overrides::default() };
        let o = set.apply(base.clone());
        assert_eq!((o.web_port, o.web_binds.clone(), o.read_only), (7999, vec!["tailscale".to_string()], true));
        assert_eq!(o.sources, vec!["swarms=herdr"], "the herdr default stays unless --source is given");
        assert_eq!(Overrides::default().apply(base.clone()), base);
    }
}
