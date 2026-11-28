//! The web server's swarm launch (`POST /api/swarms`, #583): the dashboard's
//! "Launch New Swarm" panel and the village's Start swarm form (#578) both
//! start a swarm with `autocoder-ctl start`, as the TUI does (#544,
//! `crate::adapter::ctl`), and the page gets the verb's answer: whether it
//! started, the prompts an agent waits on (shown, never answered), and the
//! agent it actually started on when autocoder picked another one with
//! capacity (laird/agents#454).
//!
//! A server that drives its swarms over ssh runs the remote host's own
//! `autocoder-ctl.sh` there (#585). There is no other way to start one (#546
//! removed agents-tui's own launcher): no `autocoder-ctl.sh`, or an answer of
//! exit 3 (an autocoder without the verb), answers `ok: false` saying to
//! update autocoder.

use std::path::Path;

use serde_json::{json, Value};

use crate::adapter::ctl::{self, CtlResult, CtlRun};
use crate::model::swarm::AgentType;

/// The multiplexers `autocoder-ctl start --mux` takes.
pub const MUXES: [&str; 3] = ["tmux", "cmux", "herdr"];

/// How the launch went.
#[derive(Debug, PartialEq)]
pub enum Launch {
    /// autocoder-ctl answered: the page's JSON.
    Ctl { ok: bool, body: Value },
    /// No usable `autocoder-ctl start`.
    Unsupported,
}

/// Start the swarm with `script start` (`None`: this host has no script).
pub async fn via_ctl(
    transport: &crate::transport::ServerTransport,
    script: Option<&Path>,
    repo: &Path,
    workers: u32,
    agent: &AgentType,
    mux: Option<&str>,
    project: &str,
) -> Launch {
    let args = ctl::with_mux(ctl::start_args(repo, workers, agent, true), mux);
    match ctl::run_on(transport, script, &args, repo).await {
        CtlRun::Unsupported => Launch::Unsupported,
        CtlRun::Done(result) => Launch::Ctl { ok: result.ok, body: answer(&result, agent, project) },
    }
}

/// The page's JSON when this server cannot start a swarm: the autocoder
/// where its swarms run (this machine, or `host` over ssh) lacks `start`.
pub fn cannot_start(host: Option<&str>, project: &str) -> Value {
    let why = ctl::no_start(host);
    json!({ "ok": false, "via": "autocoder-ctl", "project_name": project, "error": why.trim_start_matches("❌ ") })
}

/// The page's JSON for what `start` answered.
pub fn answer(result: &CtlResult, requested: &AgentType, project: &str) -> Value {
    let asked = requested.script_flag();
    let started = result.detail.get("agent").and_then(Value::as_str).filter(|a| !a.is_empty()).unwrap_or(asked);
    // The launcher's own note on its choice: why it is not the agent asked
    // for, or why it started that one anyway when nothing else could run.
    let reason = result.notes.iter().find(|n| n.contains(" instead")).or(result.notes.first()).cloned();
    let mut body = json!({
        "ok": result.ok,
        "via": "autocoder-ctl",
        "project_name": project,
        "detail": result.detail,
        "prompts": ctl::prompt_lines(&result.prompts),
        "agent": { "requested": asked, "started": started, "reason": reason },
    });
    if result.ok {
        body["message"] = json!(started_words(result, asked, started, reason.as_deref(), project));
    } else {
        // A prompt a half-started swarm waits on is still the user's to answer.
        let shown = std::iter::once(failure(result)).chain(ctl::prompt_lines(&result.prompts));
        body["error"] = json!(shown.collect::<Vec<_>>().join(" "));
    }
    body
}

/// What both pages say after a start: the session, the agent it is on when
/// that is not the one asked for (and why), and each prompt waiting.
fn started_words(result: &CtlResult, asked: &str, started: &str, reason: Option<&str>, project: &str) -> String {
    let session = result.detail.get("session").and_then(Value::as_str).unwrap_or(project);
    let mut words = format!("Started {session}");
    if started != asked {
        words.push_str(&format!(" on {started} instead of {asked}"));
    }
    words.push('.');
    for line in reason.into_iter().map(str::to_string).chain(ctl::prompt_lines(&result.prompts)) {
        words.push(' ');
        words.push_str(&line);
    }
    words.push_str(" Refresh in a few seconds.");
    words
}

/// One sentence for a start that didn't work: `detail` is a sentence, or an
/// object with a `reason`.
fn failure(result: &CtlResult) -> String {
    let why = result
        .detail
        .as_str()
        .or_else(|| result.detail.get("reason").and_then(Value::as_str))
        .unwrap_or("see the result");
    format!("autocoder-ctl start failed: {why}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn local() -> crate::transport::ServerTransport {
        crate::transport::ServerTransport::default()
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(crate::testutil::artifact_name(&format!("launch-{name}")));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A fake `autocoder-ctl.sh` that records its arguments and answers `stdout`
    /// (after writing `stderr` there), exiting `exit`.
    fn fake(dir: &Path, stdout: &str, stderr: &str, exit: i32) -> PathBuf {
        let script = dir.join("autocoder-ctl.sh");
        std::fs::write(
            &script,
            format!(
                "printf '%s\\n' \"$@\" > \"{args}\"\nprintf '%s\\n' '{stderr}' >&2\nprintf '%s' '{stdout}'\nexit {exit}\n",
                args = dir.join("args").display()
            ),
        )
        .unwrap();
        script
    }

    fn args(dir: &Path) -> Vec<String> {
        std::fs::read_to_string(dir.join("args")).unwrap().lines().map(str::to_string).collect()
    }

    #[tokio::test]
    async fn starts_through_autocoder_ctl_and_shows_its_answer() {
        let dir = temp_dir("ok");
        let out = r#"{"ok":true,"action":"start","target":"/r","detail":{"started":true,"session":"claude-r","agent":"claude","prompts":[{"role":"worker","worker":1,"pane":"claude-r:1.1","kind":"trust","text":"Do you trust the files in this folder?"}]}}"#;
        let script = fake(&dir, out, "", 0);
        let Launch::Ctl { ok, body } = via_ctl(&local(), Some(&script), &dir, 3, &AgentType::Claude, Some("herdr"), "r").await else {
            panic!("fell back")
        };
        assert!(ok);
        let repo = dir.to_string_lossy().into_owned();
        assert_eq!(args(&dir), ["start", repo.as_str(), "--workers", "3", "--agent", "claude", "--mux", "herdr"]);
        assert_eq!((body["ok"].as_bool(), body["via"].as_str()), (Some(true), Some("autocoder-ctl")));
        assert_eq!(body["detail"]["session"], "claude-r");
        assert_eq!(body["agent"], json!({"requested": "claude", "started": "claude", "reason": null}));
        // The prompt is shown, with where to answer it; nothing answers it.
        let prompts = body["prompts"].as_array().unwrap();
        assert_eq!(prompts.len(), 1);
        assert!(prompts[0].as_str().unwrap().contains("trust prompt in pane claude-r:1.1"), "{prompts:?}");
        assert!(body.get("error").is_none());
        let words = body["message"].as_str().unwrap();
        assert!(words.starts_with("Started claude-r. ⚠️  worker 1 waits on a trust prompt"), "{words}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn says_which_agent_it_started_on_and_why() {
        let dir = temp_dir("switch");
        let out = r#"{"ok":true,"action":"start","detail":{"started":true,"session":"codex-r","agent":"codex","prompts":[]}}"#;
        let note = "⚠️  claude is out of quota until Sat 14:00; starting on codex instead (AUTOCODER_STRICT_AGENT=1 keeps claude).";
        let script = fake(&dir, out, note, 0);
        let Launch::Ctl { body, .. } = via_ctl(&local(), Some(&script), &dir, 2, &AgentType::Claude, None, "r").await else { panic!() };
        assert!(!args(&dir).contains(&"--mux".to_string()), "no --mux unless one was chosen");
        assert_eq!(body["agent"]["started"], "codex");
        assert_eq!(body["agent"]["reason"].as_str(), Some(note));
        assert_eq!(body["message"].as_str().unwrap(), format!("Started codex-r on codex instead of claude. {note} Refresh in a few seconds."));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn a_start_on_the_agent_asked_for_still_shows_the_launchers_warning() {
        let dir = temp_dir("anyway");
        let out = r#"{"ok":true,"action":"start","detail":{"started":true,"session":"claude-r","agent":"claude","prompts":[]}}"#;
        let note = "⚠️  claude is out of quota, and no other configured agent can run either; starting on claude anyway.";
        let script = fake(&dir, out, note, 0);
        let Launch::Ctl { body, .. } = via_ctl(&local(), Some(&script), &dir, 2, &AgentType::Claude, None, "r").await else { panic!() };
        assert_eq!(body["agent"]["reason"].as_str(), Some(note));
        assert_eq!(body["message"].as_str().unwrap(), format!("Started claude-r. {note} Refresh in a few seconds."));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn a_refused_start_says_why() {
        let dir = temp_dir("refused");
        let out = r#"{"ok":false,"action":"start","detail":{"started":false,"reason":"a swarm session for this repo is already running; use add-worker, or stop-swarm first"}}"#;
        let script = fake(&dir, out, "", 1);
        let Launch::Ctl { ok, body } = via_ctl(&local(), Some(&script), &dir, 3, &AgentType::Codex, None, "r").await else { panic!() };
        assert!(!ok);
        assert_eq!(
            body["error"].as_str(),
            Some("autocoder-ctl start failed: a swarm session for this repo is already running; use add-worker, or stop-swarm first")
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The dashboard's launch panel shows the same answer the village's
    /// Start swarm form does (startswarmwords.js `launchAnswer`).
    #[test]
    fn the_dashboard_shows_autocoders_answer() {
        let ui = include_str!("ui.html");
        assert!(ui.contains("msg: data.message || ("), "the started message");
        assert!(ui.contains("(data.error || ('Launch failed (HTTP '"), "the failure");
        // A bare 400 or 403 has no body to parse (#584).
        assert!(ui.contains("const data = await r.json().catch(() => ({}));"));
        assert!(ui.contains("r.status === 403 ? (data.error || 'This dashboard is read-only"));
        assert!(ui.contains("r.status === 400 ? \"That repository path doesn't exist"));
        let words = include_str!("village/startswarmwords.js");
        assert!(words.contains("body.ok && body.message) return { ok: true, words: body.message }"));
    }

    #[tokio::test]
    async fn no_script_or_exit_3_cannot_start_and_says_to_update() {
        let dir = temp_dir("unsupported");
        assert_eq!(via_ctl(&local(), None, &dir, 3, &AgentType::Claude, None, "r").await, Launch::Unsupported);
        let script = fake(&dir, r#"{"ok":false,"detail":"not implemented"}"#, "", 3);
        assert_eq!(via_ctl(&local(), Some(&script), &dir, 3, &AgentType::Claude, None, "r").await, Launch::Unsupported);
        let _ = std::fs::remove_dir_all(&dir);
        let body = cannot_start(None, "r");
        assert_eq!(body["ok"], false);
        assert!(body["error"].as_str().unwrap().starts_with("This autocoder has no `autocoder-ctl start`"));
        assert!(cannot_start(Some("buildbox"), "r")["error"].as_str().unwrap().starts_with("The autocoder on buildbox has no"));
    }
}
