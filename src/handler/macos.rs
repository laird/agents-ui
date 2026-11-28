//! The macOS side of the `agents-ui://` handler (#582, laird/agents-ui#549
//! slice d): `agents-tui handler install` builds a small handler app, and
//! `agents-tui open` runs its plan in Ghostty or Terminal.
//!
//! macOS hands a URL to an app as an Apple Event, not as an argument, so the
//! handler is an AppleScript applet (`osacompile`) whose `on open location`
//! runs `agents-tui open <url>`. Its `Info.plist` declares the scheme
//! (`CFBundleURLTypes`), the edited bundle is re-signed ad hoc (an edited
//! bundle's old signature no longer matches, and Apple silicon won't run
//! it), and `lsregister` tells LaunchServices about it.
//!
//! Everything that runs a program takes it as an injected `run`, and the
//! terminal choice takes `app_exists`, so both are tested on any OS.

use std::io::Write;
use std::path::{Path, PathBuf};

pub const APP_NAME: &str = "AgentsUILinkHandler.app";
pub const BUNDLE_ID: &str = "com.github.laird.agents-ui.link-handler";
const LSREGISTER: &str = "/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister";
const URL_TYPES: &str = "<array><dict><key>CFBundleURLName</key><string>agents-ui link</string><key>CFBundleURLSchemes</key><array><string>agents-ui</string></array></dict></array>";
/// Apps LaunchServices starts get only `/usr/bin:/bin:/usr/sbin:/sbin`, so
/// the applet adds where Homebrew puts `herdr` and `tailscale`.
const EXTRA_PATH: &str = "/opt/homebrew/bin:/usr/local/bin";

/// `~/Applications/AgentsUILinkHandler.app`: the user's own Applications
/// folder, so installing needs no admin rights.
pub fn app_path(home: &Path) -> PathBuf {
    home.join("Applications").join(APP_NAME)
}

/// Where the applet's source is kept, so a human can read what it runs.
pub fn source_path(home: &Path) -> PathBuf {
    home.join("Library/Application Support/agents-tui/link-handler.applescript")
}

/// `s` as one POSIX shell word: single-quoted, with each `'` closed,
/// escaped and reopened. Every word a command line is built from goes
/// through this, so no field can be read as shell syntax.
pub fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

/// `program args...` as one shell command line, each word quoted.
pub fn shell_line(program: &str, args: &[String]) -> String {
    std::iter::once(program).chain(args.iter().map(String::as_str)).map(shell_quote).collect::<Vec<_>>().join(" ")
}

/// `s` inside an AppleScript string literal.
fn applescript_string(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', r"\\").replace('"', "\\\""))
}

/// The applet's source. `exe` is this running binary's own path, as on
/// Linux, so the handler keeps working however `PATH` changes. The URL goes
/// to the shell through AppleScript's own `quoted form of`; `open`'s
/// message goes to stderr, which is what `do shell script` reports, so a
/// refused link shows an alert saying why.
pub fn applescript(exe: &Path) -> String {
    let command = format!("PATH={EXTRA_PATH}:$PATH {} open ", shell_quote(&exe.display().to_string()));
    format!(
        "-- The agents-ui:// handler that `agents-tui handler install` built (laird/agents-ui#582).\n\
on open location theURL\n\
\ttry\n\
\t\tdo shell script {} & quoted form of theURL & \" 1>&2\"\n\
\ton error msg\n\
\t\tdisplay alert \"agents-tui open\" message msg as critical\n\
\tend try\n\
end open location\n\
\n\
on run\n\
\tdisplay alert \"agents-tui link handler\" message \"This app opens agents-ui:// links: click Open in herdr in an agent's dialog in the village.\"\n\
end run\n",
        applescript_string(&command)
    )
}

/// The commands that turn the applet's source into a registered handler,
/// in order.
pub fn install_commands(home: &Path) -> Vec<(String, Vec<String>)> {
    let app = app_path(home).display().to_string();
    let plist = format!("{app}/Contents/Info.plist");
    let cmd = |p: &str, a: &[&str]| (p.to_string(), a.iter().map(|s| s.to_string()).collect());
    vec![
        cmd("osacompile", &["-o", &app, &source_path(home).display().to_string()]),
        cmd("plutil", &["-replace", "CFBundleIdentifier", "-string", BUNDLE_ID, &plist]),
        cmd("plutil", &["-replace", "LSUIElement", "-bool", "true", &plist]),
        cmd("plutil", &["-replace", "CFBundleURLTypes", "-xml", URL_TYPES, &plist]),
        cmd("codesign", &["--force", "--sign", "-", &app]),
        cmd(LSREGISTER, &["-f", &app]),
    ]
}

/// Write the applet's source, build and register the app, or just report
/// what that would do under `--dry-run`. A handler app from an earlier
/// install is replaced, not added to.
pub fn install(
    exe: &Path,
    home: &Path,
    dry_run: bool,
    mut run: impl FnMut(&str, &[String]) -> Result<(), String>,
    out: &mut dyn Write,
) -> Result<(), String> {
    let source = source_path(home);
    let app = app_path(home);
    let text = applescript(exe);
    let commands = install_commands(home);
    if dry_run {
        writeln!(out, "would write {}:", source.display()).ok();
        writeln!(out, "{text}").ok();
        if app.exists() {
            writeln!(out, "would replace {}", app.display()).ok();
        }
        for (program, args) in &commands {
            writeln!(out, "would run: {}", shell_line(program, args)).ok();
        }
        return Ok(());
    }
    let dir = source.parent().ok_or("no parent directory for the applet source")?;
    std::fs::create_dir_all(dir).map_err(|e| format!("could not create {}: {e}", dir.display()))?;
    std::fs::write(&source, &text).map_err(|e| format!("could not write {}: {e}", source.display()))?;
    writeln!(out, "wrote {}", source.display()).ok();
    if let Some(apps) = app.parent() {
        std::fs::create_dir_all(apps).map_err(|e| format!("could not create {}: {e}", apps.display()))?;
    }
    if app.exists() {
        std::fs::remove_dir_all(&app).map_err(|e| format!("could not replace {}: {e}", app.display()))?;
    }
    for (program, args) in &commands {
        run(program, args)?;
    }
    writeln!(out, "built {} and registered agents-ui:// with LaunchServices", app.display()).ok();
    Ok(())
}

/// The terminal a plan opens in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MacTerminal {
    Ghostty,
    Terminal,
}

/// Ghostty when it is installed (in `/Applications` or `~/Applications`),
/// else Terminal, which every Mac has.
pub fn choose(app_exists: impl Fn(&str) -> bool) -> MacTerminal {
    if app_exists("Ghostty.app") { MacTerminal::Ghostty } else { MacTerminal::Terminal }
}

/// The command that opens `program args...` in `term`. Ghostty takes the
/// command as separate words after `-e`; Terminal's `do script` takes one
/// shell line, which arrives as `osascript`'s argument, so it is never part
/// of the AppleScript source.
pub fn command_for(term: MacTerminal, program: &str, args: &[String]) -> (String, Vec<String>) {
    match term {
        MacTerminal::Ghostty => {
            let mut full = vec!["-na".to_string(), "Ghostty".to_string(), "--args".to_string(), "-e".to_string(), program.to_string()];
            full.extend(args.iter().cloned());
            ("open".to_string(), full)
        }
        MacTerminal::Terminal => {
            let script = ["on run argv", "tell application \"Terminal\"", "activate", "do script (item 1 of argv)", "end tell", "end run"];
            let mut full = Vec::new();
            for line in script {
                full.push("-e".to_string());
                full.push(line.to_string());
            }
            full.push(shell_line(program, args));
            ("osascript".to_string(), full)
        }
    }
}

/// Open `program args...` in Ghostty or Terminal. `program` is resolved to
/// its full path first: neither terminal is promised to start it with this
/// process's `PATH`.
pub fn spawn(program: &str, args: &[String], resolve: impl Fn(&str) -> Option<PathBuf>, home: Option<&Path>) -> Result<(), String> {
    let full = resolve(program).ok_or_else(|| format!("{program} is not on PATH; run directly: {}", shell_line(program, args)))?;
    let app_exists = |name: &str| Path::new("/Applications").join(name).exists() || home.is_some_and(|h| h.join("Applications").join(name).exists());
    let (launcher, launcher_args) = command_for(choose(app_exists), &full.display().to_string(), args);
    std::process::Command::new(&launcher)
        .args(&launcher_args)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("could not launch {launcher}: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn shell_quote_keeps_every_word_literal() {
        assert_eq!(shell_quote("w1:p1"), "'w1:p1'");
        assert_eq!(shell_quote("Build machine"), "'Build machine'");
        assert_eq!(shell_quote("it's; rm -rf /"), r"'it'\''s; rm -rf /'");
        assert_eq!(shell_line("ssh", &strings(&["-t", "a b", "$(x)"])), "'ssh' '-t' 'a b' '$(x)'");
    }

    #[test]
    fn the_applet_runs_this_binary_with_the_url_quoted_by_applescript() {
        let text = applescript(Path::new("/Users/x/.cargo/bin/agents-tui"));
        assert!(text.contains("on open location theURL"));
        assert!(text.contains(r#"do shell script "PATH=/opt/homebrew/bin:/usr/local/bin:$PATH '/Users/x/.cargo/bin/agents-tui' open " & quoted form of theURL"#), "{text}");
        assert!(text.contains("display alert"), "a refused link says why");
    }

    #[test]
    fn a_quote_or_backslash_in_the_binary_path_cannot_end_the_applescript_string() {
        let text = applescript(Path::new(r#"/tmp/a"b\c/agents-tui"#));
        assert!(text.contains(r#"'/tmp/a\"b\\c/agents-tui'"#), "{text}");
    }

    #[test]
    fn install_commands_build_declare_sign_and_register_in_that_order() {
        let home = Path::new("/Users/x");
        let cmds = install_commands(home);
        let programs: Vec<&str> = cmds.iter().map(|(p, _)| p.as_str()).collect();
        assert_eq!(programs, vec!["osacompile", "plutil", "plutil", "plutil", "codesign", LSREGISTER]);
        assert_eq!(cmds[0].1, strings(&["-o", "/Users/x/Applications/AgentsUILinkHandler.app", "/Users/x/Library/Application Support/agents-tui/link-handler.applescript"]));
        let url_types = &cmds[3].1;
        assert_eq!(url_types[1], "CFBundleURLTypes");
        assert!(url_types[3].contains("<string>agents-ui</string>"));
        assert_eq!(url_types[4], "/Users/x/Applications/AgentsUILinkHandler.app/Contents/Info.plist");
        assert_eq!(cmds[4].1, strings(&["--force", "--sign", "-", "/Users/x/Applications/AgentsUILinkHandler.app"]));
    }

    fn temp_home(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!("agents-ui-macos-handler-{tag}-{}", std::process::id()))
    }

    #[test]
    fn dry_run_writes_nothing_and_runs_nothing() {
        let home = temp_home("dryrun");
        let mut out = Vec::new();
        let mut ran = false;
        install(Path::new("/opt/agents-tui"), &home, true, |_, _| {
            ran = true;
            Ok(())
        }, &mut out)
        .unwrap();
        assert!(!ran);
        assert!(!home.exists());
        let printed = String::from_utf8(out).unwrap();
        assert!(printed.contains("would write"));
        assert!(printed.contains("would run: 'osacompile'"));
        assert!(printed.contains("would run: 'codesign'"));
    }

    #[test]
    fn install_writes_the_source_replaces_an_old_app_and_runs_every_step() {
        let home = temp_home("install");
        let old = app_path(&home).join("Contents");
        std::fs::create_dir_all(&old).unwrap();
        std::fs::write(old.join("stale"), "x").unwrap();
        let mut out = Vec::new();
        let mut recorded = Vec::new();
        install(Path::new("/opt/agents-tui"), &home, false, |p, a| {
            recorded.push((p.to_string(), a.to_vec()));
            Ok(())
        }, &mut out)
        .unwrap();
        assert!(std::fs::read_to_string(source_path(&home)).unwrap().contains("'/opt/agents-tui' open"));
        assert!(!app_path(&home).exists(), "the old app was removed before osacompile rebuilds it");
        assert_eq!(recorded, install_commands(&home));
        std::fs::remove_dir_all(&home).ok();
    }

    #[test]
    fn a_failed_step_stops_the_install_and_is_reported() {
        let home = temp_home("fail");
        let mut out = Vec::new();
        let mut count = 0;
        let err = install(Path::new("/opt/agents-tui"), &home, false, |p, _| {
            count += 1;
            if p == "codesign" { Err("codesign failed".to_string()) } else { Ok(()) }
        }, &mut out)
        .unwrap_err();
        assert_eq!(err, "codesign failed");
        assert_eq!(count, 5, "lsregister is not run for an unsigned app");
        std::fs::remove_dir_all(&home).ok();
    }

    #[test]
    fn ghostty_wins_when_installed() {
        assert_eq!(choose(|n| n == "Ghostty.app"), MacTerminal::Ghostty);
        assert_eq!(choose(|_| false), MacTerminal::Terminal);
    }

    #[test]
    fn ghostty_gets_the_command_as_separate_words() {
        let (program, args) = command_for(MacTerminal::Ghostty, "/usr/bin/ssh", &strings(&["-t", "nuc-1", "herdr", "agent", "attach", "w1:p1"]));
        assert_eq!(program, "open");
        assert_eq!(args, strings(&["-na", "Ghostty", "--args", "-e", "/usr/bin/ssh", "-t", "nuc-1", "herdr", "agent", "attach", "w1:p1"]));
    }

    #[test]
    fn terminal_gets_one_quoted_line_as_an_argument_not_as_script_source() {
        let (program, args) = command_for(MacTerminal::Terminal, "/usr/bin/ssh", &strings(&["-t", "Build machine", "tmux", "attach", "-t", "%3"]));
        assert_eq!(program, "osascript");
        assert_eq!(args.last().unwrap(), "'/usr/bin/ssh' '-t' 'Build machine' 'tmux' 'attach' '-t' '%3'");
        assert!(args.contains(&"do script (item 1 of argv)".to_string()));
        assert!(!args[..args.len() - 1].iter().any(|a| a.contains("ssh")), "the command is never in the script lines");
    }

    #[test]
    fn spawn_refuses_a_program_it_cannot_find() {
        let err = spawn("herdr", &strings(&["agent", "attach", "w1:p1"]), |_| None, None).unwrap_err();
        assert!(err.contains("herdr is not on PATH"));
        assert!(err.contains("'herdr' 'agent' 'attach' 'w1:p1'"));
    }
}
