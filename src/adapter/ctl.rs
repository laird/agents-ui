//! Swarm actions through `autocoder-ctl.sh` (#544): `start`, `add-worker` and
//! `stop-swarm`, called with plain arguments and read back as the one JSON
//! object its result contract promises (laird/agents `docs/autocoder-ctl.md`,
//! `docs/autocoder-ctl/swarm.md`).
//!
//! An autocoder too old to have the verb (no `autocoder-ctl.sh`, or exit 3)
//! is [`CtlRun::Unsupported`], and the caller falls back to the TUI's own
//! launcher in `claude.rs`. A trust or consent prompt the verb found
//! (`detail.prompts`) is shown to the user, never answered: its text is the
//! pane's, so it is shown with control characters removed.

use std::path::{Path, PathBuf};

use crate::model::swarm::AgentType;

/// A verb that has not answered by then is killed (a `start` that installs
/// the plugin and lays out a swarm takes well under this).
const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(600);
/// How much of the JSON result the launch dialog shows.
const RESULT_SHOWN: usize = 4000;

/// A prompt an agent is waiting on, as `detail.prompts` reports it.
#[derive(Debug, Clone, PartialEq)]
pub struct Prompt {
    pub role: String,
    pub worker: Option<u64>,
    pub pane: String,
    pub kind: String,
    pub text: String,
}

/// What one verb answered.
#[derive(Debug, Clone, PartialEq)]
pub struct CtlResult {
    pub action: String,
    pub ok: bool,
    pub exit: Option<i32>,
    /// `detail`: the verb's data on success, a sentence on failure.
    pub detail: serde_json::Value,
    pub prompts: Vec<Prompt>,
    /// The launcher's own notes on stderr: why it started a different agent
    /// than the one asked for (laird/agents#454, "starting on <agent> instead").
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CtlRun {
    Done(CtlResult),
    /// No `autocoder-ctl.sh`, or it answered exit 3 ("not implemented").
    Unsupported,
}

/// Read a verb's run (the result contract, read by `web::ctl::read`, which
/// the village's worker controls share), adding the prompts it found.
pub fn parse_result(action: &str, stdout: &[u8], stderr: &[u8], exit: Option<i32>) -> CtlRun {
    let (ok, exit, detail) = match crate::web::ctl::read(exit, stdout, stderr) {
        crate::web::ctl::Answer::NotInstalled => return CtlRun::Unsupported,
        crate::web::ctl::Answer::Result { ok, exit, detail } => (ok && exit == 0, Some(exit), detail),
        crate::web::ctl::Answer::Broken(why) => (false, exit, serde_json::Value::String(why)),
    };
    let prompts = detail
        .get("prompts")
        .and_then(|p| p.as_array())
        .into_iter()
        .flatten()
        .map(|p| {
            let s = |k: &str| p.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
            Prompt { role: s("role"), worker: p.get("worker").and_then(|v| v.as_u64()), pane: s("pane"), kind: s("kind"), text: s("text") }
        })
        .collect();
    let notes = String::from_utf8_lossy(stderr)
        .lines()
        .filter(|l| l.contains("starting on "))
        .map(shown)
        .collect();
    CtlRun::Done(CtlResult { action: action.to_string(), ok, exit, detail, prompts, notes })
}

/// `start <repo> --workers N --agent A [--no-plugin-install]`.
pub fn start_args(repo: &Path, workers: u32, agent: &AgentType, plugin_install: bool) -> Vec<String> {
    let mut args = vec![
        "start".to_string(),
        repo.to_string_lossy().into_owned(),
        "--workers".to_string(),
        workers.to_string(),
        "--agent".to_string(),
        agent.script_flag().to_string(),
    ];
    if !plugin_install {
        args.push("--no-plugin-install".to_string());
    }
    args
}

/// `start`'s arguments with `--mux` added when one was chosen.
pub fn with_mux(mut args: Vec<String>, mux: Option<&str>) -> Vec<String> {
    if let Some(m) = mux {
        args.extend(["--mux".to_string(), m.to_string()]);
    }
    args
}

/// `add-worker 1 --repo <repo> --session <session>`.
pub fn add_worker_args(repo: &Path, session: &str) -> Vec<String> {
    ["add-worker", "1", "--repo", &repo.to_string_lossy(), "--session", session]
        .map(str::to_string)
        .to_vec()
}

/// `stop-swarm --repo <repo>`.
pub fn stop_swarm_args(repo: &Path) -> Vec<String> {
    ["stop-swarm", "--repo", &repo.to_string_lossy()].map(str::to_string).to_vec()
}

/// Whether autocoder knows the swarm `session` in `repo`: its manifest
/// (`.autocoder/swarm/<session>.json`) is in the repo's main worktree. A
/// swarm the TUI launched itself has none, and stays with the TUI's code.
pub fn has_manifest(repo: &Path, session: &str) -> bool {
    let file = format!(".autocoder/swarm/{session}.json");
    if repo.join(&file).is_file() {
        return true;
    }
    // A linked worktree: the manifests are in the main one.
    std::process::Command::new("git")
        .args(["rev-parse", "--path-format=absolute", "--git-common-dir"])
        .current_dir(repo)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| {
            let common = PathBuf::from(String::from_utf8_lossy(&o.stdout).trim());
            common.parent().map(|main| main.join(&file).is_file())
        })
        .unwrap_or(false)
}

/// Run `bash <script> <args>` in `cwd`. `None` for the script means none is
/// installed.
pub async fn run(script: Option<&Path>, args: &[String], cwd: &Path) -> CtlRun {
    let Some(script) = script.filter(|s| s.exists()) else {
        return CtlRun::Unsupported;
    };
    let action = args.first().map(String::as_str).unwrap_or("");
    let out = tokio::time::timeout(
        TIMEOUT,
        tokio::process::Command::new("bash")
            .arg(script)
            .args(args)
            .current_dir(cwd)
            .env("AUTOCODER_CTL_ACTOR", "agents-tui")
            .stdin(std::process::Stdio::null())
            .kill_on_drop(true)
            .output(),
    )
    .await
    .unwrap_or_else(|_| Err(std::io::Error::other(format!("no answer in {}s", TIMEOUT.as_secs()))));
    match out {
        Ok(out) => parse_result(action, &out.stdout, &out.stderr, out.status.code()),
        Err(e) => CtlRun::Done(CtlResult {
            action: action.to_string(),
            ok: false,
            exit: None,
            detail: serde_json::Value::String(format!("could not run {}: {e}", script.display())),
            prompts: Vec::new(),
            notes: Vec::new(),
        }),
    }
}

/// The installed `autocoder-ctl.sh`, if any. None in tests, as for the
/// village's controls (`web::ctl::script`): a test must never act on a real swarm.
pub fn script() -> Option<PathBuf> {
    if cfg!(test) {
        return None;
    }
    crate::scripts::launcher::find_script("autocoder-ctl.sh")
}

/// Pane text made safe to print: control characters (escape sequences
/// included) removed, and cut to one short line.
fn shown(text: &str) -> String {
    let clean: String = text.chars().filter(|c| !c.is_control()).collect();
    let clean = clean.trim();
    match clean.char_indices().nth(120) {
        Some((i, _)) => format!("{}…", &clean[..i]),
        None => clean.to_string(),
    }
}

/// One line per prompt, saying where to answer it.
pub fn prompt_lines(prompts: &[Prompt]) -> Vec<String> {
    prompts
        .iter()
        .map(|p| {
            let who = match p.worker {
                Some(n) => format!("{} {n}", shown(&p.role)),
                None => shown(&p.role),
            };
            format!(
                "⚠️  {who} waits on a {} prompt in pane {}: \"{}\". Answer it there; agents-tui does not.",
                shown(&p.kind),
                shown(&p.pane),
                shown(&p.text)
            )
        })
        .collect()
}

/// What the launch dialog shows: the outcome, any prompts, and the JSON result.
pub fn report(result: &CtlResult) -> String {
    let mut out = if result.ok {
        format!("✅ autocoder-ctl {}: done\n", result.action)
    } else {
        let why = result.detail.as_str().map(shown).unwrap_or_else(|| "see the result below".into());
        format!("❌ autocoder-ctl {} failed: {why}\n", result.action)
    };
    for line in prompt_lines(&result.prompts) {
        out.push_str(&line);
        out.push('\n');
    }
    let json = serde_json::to_string_pretty(&result.detail).unwrap_or_default();
    let mut json = shown_block(&json);
    if let Some((i, _)) = json.char_indices().nth(RESULT_SHOWN) {
        json.truncate(i);
        json.push_str("\n…");
    }
    out.push_str(&format!("\nResult:\n{json}\n"));
    out
}

/// A block of the result, control characters removed except newlines.
fn shown_block(text: &str) -> String {
    text.chars().filter(|c| *c == '\n' || !c.is_control()).collect()
}

/// The one-line status after a verb, for the TUI's status bar.
pub fn status_line(result: &CtlResult, done: &str) -> String {
    let mut line = if result.ok {
        done.to_string()
    } else {
        let why = result.detail.as_str().map(shown).unwrap_or_else(|| format!("exit {:?}", result.exit));
        format!("autocoder-ctl {} failed: {why}", result.action)
    };
    if let Some(first) = prompt_lines(&result.prompts).first() {
        line.push_str(" — ");
        line.push_str(first);
    }
    line
}

#[cfg(test)]
mod tests {
    use super::*;

    const STARTED: &str = r#"{"ok": true, "action": "start", "target": "/src/r",
      "detail": {"repo": "/src/r", "started": true, "session": "claude-r", "workers": [1, 2],
        "prompts": [{"role": "worker", "worker": 1, "pane": "claude-r:1.1", "kind": "trust",
                     "text": "Do you trust the files in this folder?\u001b[0m"}],
        "prompts_pending": true, "output": "tmux attach -t claude-r"}}"#;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(crate::testutil::artifact_name(&format!("ctl-{name}")));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn reads_a_started_swarm_and_its_prompts() {
        let CtlRun::Done(r) = parse_result("start", STARTED.as_bytes(), b"", Some(0)) else { panic!("unsupported") };
        assert!(r.ok);
        assert_eq!(r.detail["session"], "claude-r");
        assert_eq!(r.prompts.len(), 1);
        assert_eq!((r.prompts[0].worker, r.prompts[0].kind.as_str()), (Some(1), "trust"));
    }

    #[test]
    fn exit_3_means_this_autocoder_lacks_the_verb() {
        let old = r#"{"ok": false, "action": "start", "target": null, "detail": "not implemented"}"#;
        assert_eq!(parse_result("start", old.as_bytes(), b"", Some(3)), CtlRun::Unsupported);
    }

    #[test]
    fn a_failed_verb_says_why() {
        let failed = r#"{"ok": false, "action": "add-worker", "target": null, "detail": "no swarm in /src/r: start one first"}"#;
        let CtlRun::Done(r) = parse_result("add-worker", failed.as_bytes(), b"", Some(1)) else { panic!() };
        assert!(!r.ok);
        assert_eq!(status_line(&r, "added"), "autocoder-ctl add-worker failed: no swarm in /src/r: start one first");
        let CtlRun::Done(r) = parse_result("start", "garbage".as_bytes(), b"", Some(0)) else { panic!() };
        assert!(!r.ok, "no JSON is no success");
    }

    #[test]
    fn a_prompt_is_shown_never_answered_and_its_text_is_cleaned() {
        let CtlRun::Done(r) = parse_result("start", STARTED.as_bytes(), b"", Some(0)) else { panic!() };
        let lines = prompt_lines(&r.prompts);
        assert_eq!(
            lines,
            vec!["⚠️  worker 1 waits on a trust prompt in pane claude-r:1.1: \"Do you trust the files in this folder?[0m\". Answer it there; agents-tui does not."]
        );
        assert!(!lines[0].contains('\u{1b}'));
        let text = report(&r);
        assert!(text.starts_with("✅ autocoder-ctl start: done\n⚠️  worker 1"));
        assert!(text.contains("\"session\": \"claude-r\""), "the JSON result is shown: {text}");
        assert!(status_line(&r, "Started").starts_with("Started — ⚠️  worker 1"));
    }

    #[test]
    fn builds_each_verbs_arguments() {
        let repo = Path::new("/src/r");
        assert_eq!(
            start_args(repo, 3, &AgentType::Codex, false),
            ["start", "/src/r", "--workers", "3", "--agent", "codex", "--no-plugin-install"]
        );
        assert_eq!(start_args(repo, 1, &AgentType::Claude, true).len(), 6);
        assert_eq!(add_worker_args(repo, "claude-r"), ["add-worker", "1", "--repo", "/src/r", "--session", "claude-r"]);
        assert_eq!(stop_swarm_args(repo), ["stop-swarm", "--repo", "/src/r"]);
    }

    #[test]
    fn a_swarm_is_autocoders_when_its_manifest_is_there() {
        let dir = temp_dir("manifest");
        assert!(!has_manifest(&dir, "claude-r"));
        std::fs::create_dir_all(dir.join(".autocoder/swarm")).unwrap();
        std::fs::write(dir.join(".autocoder/swarm/claude-r.json"), "{}").unwrap();
        assert!(has_manifest(&dir, "claude-r"));
        assert!(!has_manifest(&dir, "claude-other"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn no_script_means_the_tuis_own_path() {
        let dir = temp_dir("none");
        assert_eq!(run(None, &stop_swarm_args(&dir), &dir).await, CtlRun::Unsupported);
        assert_eq!(run(Some(&dir.join("autocoder-ctl.sh")), &stop_swarm_args(&dir), &dir).await, CtlRun::Unsupported);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn runs_the_verb_and_reads_its_answer() {
        let dir = temp_dir("run");
        let script = dir.join("autocoder-ctl.sh");
        // Echoes its verb and the actor it was given.
        std::fs::write(
            &script,
            r#"printf '{"ok":true,"action":"%s","target":"%s","detail":{"actor":"%s","stopped":true}}' "$1" "$3" "$AUTOCODER_CTL_ACTOR""#,
        )
        .unwrap();
        let CtlRun::Done(r) = run(Some(&script), &stop_swarm_args(&dir), &dir).await else { panic!() };
        assert!(r.ok);
        assert_eq!((r.action.as_str(), r.detail["actor"].as_str()), ("stop-swarm", Some("agents-tui")));

        std::fs::write(&script, r#"echo '{"ok":false,"detail":"not implemented"}'; exit 3"#).unwrap();
        assert_eq!(run(Some(&script), &stop_swarm_args(&dir), &dir).await, CtlRun::Unsupported);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
