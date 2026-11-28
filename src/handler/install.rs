//! `agents-tui handler install` (#576): registers this binary as the Linux
//! desktop handler for the `agents-ui://` scheme, via a `.desktop` file
//! (`xdg-mime default ... x-scheme-handler/agents-ui`). macOS and Windows
//! handlers are future subtasks of laird/agents-ui#549.

use std::io::Write;
use std::path::{Path, PathBuf};

pub const DESKTOP_FILE_NAME: &str = "agents-tui-open.desktop";

/// The `.desktop` file's text. `Exec` names `exe` (this running binary's own
/// path), not bare `agents-tui`, so the installed entry keeps working
/// however `PATH` changes later.
pub fn desktop_file(exe: &Path) -> String {
    format!(
        "[Desktop Entry]\n\
Type=Application\n\
Name=agents-tui link handler\n\
Comment=Open an agents-ui:// link from the village\n\
Exec={} open %u\n\
NoDisplay=true\n\
MimeType=x-scheme-handler/agents-ui;\n",
        exe.display()
    )
}

/// `<data_local_dir>/applications/agents-tui-open.desktop`
/// (`~/.local/share/applications/...` under XDG defaults).
pub fn desktop_file_path(data_local_dir: &Path) -> PathBuf {
    data_local_dir.join("applications").join(DESKTOP_FILE_NAME)
}

/// Write the desktop entry and register it with `xdg-mime`, or just report
/// what that would do under `--dry-run`. `run` executes `xdg-mime` so the
/// tests can record instead of running it for real.
pub fn install(
    exe: &Path,
    data_local_dir: &Path,
    dry_run: bool,
    mut run: impl FnMut(&str, &[&str]) -> Result<(), String>,
    out: &mut dyn Write,
) -> Result<(), String> {
    let path = desktop_file_path(data_local_dir);
    let contents = desktop_file(exe);
    if dry_run {
        writeln!(out, "would write {}:", path.display()).ok();
        writeln!(out, "{contents}").ok();
        writeln!(out, "would run: xdg-mime default {DESKTOP_FILE_NAME} x-scheme-handler/agents-ui").ok();
        return Ok(());
    }
    let dir = path.parent().ok_or("no parent directory for the desktop file")?;
    std::fs::create_dir_all(dir).map_err(|e| format!("could not create {}: {e}", dir.display()))?;
    std::fs::write(&path, &contents).map_err(|e| format!("could not write {}: {e}", path.display()))?;
    writeln!(out, "wrote {}", path.display()).ok();
    run("xdg-mime", &["default", DESKTOP_FILE_NAME, "x-scheme-handler/agents-ui"])?;
    writeln!(out, "registered agents-ui:// with xdg-mime").ok();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_file_points_exec_at_the_given_binary() {
        let text = desktop_file(Path::new("/opt/agents-tui/agents-tui"));
        assert!(text.contains("Exec=/opt/agents-tui/agents-tui open %u"));
        assert!(text.contains("MimeType=x-scheme-handler/agents-ui;"));
    }

    #[test]
    fn desktop_file_path_is_under_applications() {
        assert_eq!(desktop_file_path(Path::new("/home/x/.local/share")), PathBuf::from("/home/x/.local/share/applications/agents-tui-open.desktop"));
    }

    #[test]
    fn dry_run_writes_nothing_and_runs_nothing() {
        let dir = std::env::temp_dir().join(format!("agents-ui-handler-install-dryrun-{}", std::process::id()));
        let mut out = Vec::new();
        let mut ran = false;
        install(Path::new("/opt/agents-tui"), &dir, true, |_, _| {
            ran = true;
            Ok(())
        }, &mut out)
        .unwrap();
        assert!(!ran);
        assert!(!dir.exists());
        let printed = String::from_utf8(out).unwrap();
        assert!(printed.contains("would write"));
        assert!(printed.contains("would run: xdg-mime"));
    }

    #[test]
    fn install_writes_the_file_and_registers_it() {
        let dir = std::env::temp_dir().join(format!("agents-ui-handler-install-{}", std::process::id()));
        let mut out = Vec::new();
        let mut recorded = Vec::new();
        install(
            Path::new("/opt/agents-tui"),
            &dir,
            false,
            |program, args| {
                recorded.push((program.to_string(), args.iter().map(|a| a.to_string()).collect::<Vec<_>>()));
                Ok(())
            },
            &mut out,
        )
        .unwrap();
        let path = desktop_file_path(&dir);
        assert!(path.exists());
        assert!(std::fs::read_to_string(&path).unwrap().contains("Exec=/opt/agents-tui open %u"));
        assert_eq!(recorded, vec![("xdg-mime".to_string(), vec!["default".to_string(), DESKTOP_FILE_NAME.to_string(), "x-scheme-handler/agents-ui".to_string()])]);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_failed_xdg_mime_call_is_reported() {
        let dir = std::env::temp_dir().join(format!("agents-ui-handler-install-fail-{}", std::process::id()));
        let mut out = Vec::new();
        let err = install(Path::new("/opt/agents-tui"), &dir, false, |_, _| Err("no xdg-mime on this system".to_string()), &mut out).unwrap_err();
        assert_eq!(err, "no xdg-mime on this system");
        // The file is still written: a human can register it by hand.
        assert!(desktop_file_path(&dir).exists());
        std::fs::remove_dir_all(&dir).ok();
    }
}
