//! Choosing and launching a terminal to run a [`Plan`](super::plan::Plan) in
//! (#576).
//!
//! `omarchy-launch-terminal` wins when present: Omarchy's own wrapper
//! around `xdg-terminal-exec`, with its session's `uwsm-app`/`setsid`
//! integration (the same tool `omarchy-launch-terminal-herdr` uses to open
//! herdr's persistent session from the app menu). Else `xdg-terminal-exec`
//! itself, the cross-desktop Default Terminal Specification -- the
//! portable way to run a command in whatever terminal the user already
//! chose, without this binary guessing at terminal emulators. Confirmed
//! live on this machine (laird/agents-ui#576): `omarchy-launch-terminal --
//! sleep 8` opened a real window (`foot`, Omarchy's configured default)
//! that `hyprctl clients` could see while it ran.
//!
//! Only when neither is installed does this fall back to `$TERMINAL` or a
//! short list of common emulators, each run with the `-e` convention.

/// Tried before falling back to guessing at a terminal emulator; both
/// understand `-- <command> <args...>`.
const PORTABLE_LAUNCHERS: &[&str] = &["omarchy-launch-terminal", "xdg-terminal-exec"];
/// Tried in order after `$TERMINAL`, each with the `-e` convention.
const FALLBACK_TERMINALS: &[&str] = &["x-terminal-emulator", "alacritty", "kitty", "foot", "gnome-terminal", "konsole", "xterm"];

/// Which terminal to launch `program`/`args` in, given what's on PATH.
/// Pure: `on_path` is injected so this is testable without a real PATH.
pub fn choose(on_path: impl Fn(&str) -> bool, env_terminal: Option<&str>) -> Option<Launcher> {
    if let Some(launcher) = PORTABLE_LAUNCHERS.iter().find(|l| on_path(l)) {
        return Some(Launcher::Portable(launcher.to_string()));
    }
    if let Some(term) = env_terminal
        && !term.is_empty()
        && on_path(term)
    {
        return Some(Launcher::ExecFlag(term.to_string()));
    }
    FALLBACK_TERMINALS.iter().find(|t| on_path(t)).map(|t| Launcher::ExecFlag(t.to_string()))
}

#[derive(Debug, Clone, PartialEq)]
pub enum Launcher {
    /// `<launcher> -- <program> <args...>`: `omarchy-launch-terminal` or
    /// `xdg-terminal-exec` itself, which both take the command this way.
    Portable(String),
    /// `<terminal> -e <program> <args...>` -- the convention xterm,
    /// alacritty, foot and urxvt share. gnome-terminal and konsole accept
    /// `-e` too, as a compatibility alias for their native `--`.
    ExecFlag(String),
}

impl Launcher {
    /// The full command line to spawn: program plus its arguments.
    pub fn command_for(&self, program: &str, args: &[String]) -> (String, Vec<String>) {
        let (launcher, flag) = match self {
            Launcher::Portable(l) => (l.clone(), "--"),
            Launcher::ExecFlag(l) => (l.clone(), "-e"),
        };
        let mut full = vec![flag.to_string(), program.to_string()];
        full.extend(args.iter().cloned());
        (launcher, full)
    }
}

/// Spawn `program`/`args` in a terminal chosen by [`choose`]. Detached: the
/// caller does not wait on it, matching a desktop link handler (the user's
/// click should return immediately; the terminal stays open on its own).
pub fn spawn(program: &str, args: &[String], on_path: impl Fn(&str) -> bool, env_terminal: Option<&str>) -> Result<(), String> {
    let Some(launcher) = choose(on_path, env_terminal) else {
        return Err(format!(
            "no terminal launcher found on PATH (tried omarchy-launch-terminal, xdg-terminal-exec, $TERMINAL, then {}); install one, set $TERMINAL, or run directly: {program} {}",
            FALLBACK_TERMINALS.join("/"),
            args.join(" "),
        ));
    };
    let (term_program, term_args) = launcher.command_for(program, args);
    std::process::Command::new(&term_program)
        .args(&term_args)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("could not launch {term_program}: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn on_path(names: &'static [&'static str]) -> impl Fn(&str) -> bool {
        move |n| names.contains(&n)
    }

    #[test]
    fn omarchy_launch_terminal_wins_over_everything_else() {
        assert_eq!(
            choose(on_path(&["omarchy-launch-terminal", "xdg-terminal-exec", "alacritty"]), Some("xterm")),
            Some(Launcher::Portable("omarchy-launch-terminal".to_string()))
        );
    }

    #[test]
    fn xdg_terminal_exec_wins_when_omarchys_own_wrapper_is_absent() {
        assert_eq!(choose(on_path(&["xdg-terminal-exec", "alacritty"]), Some("xterm")), Some(Launcher::Portable("xdg-terminal-exec".to_string())));
    }

    #[test]
    fn env_terminal_wins_when_no_portable_launcher_and_omarchy_absent() {
        assert_eq!(choose(on_path(&["kitty", "xterm"]), Some("kitty")), Some(Launcher::ExecFlag("kitty".to_string())));
    }

    #[test]
    fn env_terminal_is_skipped_when_not_on_path() {
        assert_eq!(choose(on_path(&["xterm"]), Some("kitty")), Some(Launcher::ExecFlag("xterm".to_string())));
    }

    #[test]
    fn falls_back_to_the_first_known_terminal_on_path() {
        assert_eq!(choose(on_path(&["foot", "xterm"]), None), Some(Launcher::ExecFlag("foot".to_string())));
    }

    #[test]
    fn none_when_nothing_is_on_path() {
        assert_eq!(choose(on_path(&[]), None), None);
    }

    #[test]
    fn portable_launcher_uses_a_double_dash() {
        let (program, args) = Launcher::Portable("omarchy-launch-terminal".to_string()).command_for("herdr", &["agent".into(), "attach".into(), "w1:p1".into()]);
        assert_eq!(program, "omarchy-launch-terminal");
        assert_eq!(args, vec!["--", "herdr", "agent", "attach", "w1:p1"]);
    }

    #[test]
    fn exec_flag_uses_dash_e() {
        let (program, args) = Launcher::ExecFlag("xterm".to_string()).command_for("tmux", &["attach".into(), "-t".into(), "%3".into()]);
        assert_eq!(program, "xterm");
        assert_eq!(args, vec!["-e", "tmux", "attach", "-t", "%3"]);
    }
}
