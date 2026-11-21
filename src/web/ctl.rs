//! Worker controls through `autocoder-ctl` (#540, laird/agents#192).
//!
//! The village's Nudge, Release, Stop and Restart (`controls.rs`) call
//! `autocoder-ctl.sh nudge|release|stop|restart` and show its answer, rather
//! than typing into panes, killing processes and editing labels themselves.
//! The contract is laird/agents `docs/autocoder-ctl.md`: one JSON object on
//! stdout, `{ ok, action, target, detail }`, and exit 0 done, 1 refused or
//! failed, 2 bad arguments, 3 this autocoder doesn't have the verb yet. The
//! worker verbs name a worker by its worktree; `release` takes the issue.
//! `AUTOCODER_CTL_ACTOR` says the village acted, so the issue comment names it.
//!
//! **Fallback**: with no `autocoder-ctl.sh` installed (an autocoder older than
//! #192), a server that drives its swarms over ssh (the script is found on
//! this host, not there), or an answer of exit 3, `controls.rs` does it
//! itself as before (#504, #505). Any other answer is shown as it is: a
//! refusal (exit 1) is autocoder being careful, and doing it again by hand
//! would undo that care.

use std::path::{Path, PathBuf};

use axum::http::StatusCode;
use serde_json::{json, Value};

use super::controls::Action;
use crate::transport::ServerTransport;

/// Who acted, for the comment autocoder posts.
pub const ACTOR: &str = "agents-ui village";

/// The script, when this host has it and drives the swarm itself. None in
/// tests: a test must never act on a real swarm.
pub fn script(transport: &ServerTransport) -> Option<PathBuf> {
    if cfg!(test) || transport.is_remote() {
        return None;
    }
    crate::scripts::launcher::find_script("autocoder-ctl.sh")
}

/// Whether a swarm manifest in `text` (the `.autocoder/swarm/*.json` files,
/// one after another) lists `worktree` as a worker. autocoder-ctl's worker
/// verbs find a worker only there; a swarm the TUI launched itself has no
/// manifest, so its workers are the village's to drive (the fallback).
pub fn listed(text: &str, worktree: &str) -> bool {
    if worktree.trim().is_empty() {
        return false;
    }
    let want = crate::model::recognize::canonical(worktree);
    serde_json::Deserializer::from_str(text)
        .into_iter::<Value>()
        .map_while(Result::ok)
        .flat_map(|m| m["workers"].as_array().cloned().unwrap_or_default())
        .filter_map(|w| w["worktree"].as_str().map(crate::model::recognize::canonical))
        .any(|p| p == want)
}

/// The script, when autocoder-ctl can do `action` for this worker: a release
/// needs only the script; nudge, stop and restart need the worker in one of
/// the repo's swarm manifests.
pub async fn applies(transport: &ServerTransport, repo: &str, worktree: &str, action: Action) -> Option<PathBuf> {
    let script = script(transport)?;
    if action == Action::Release {
        return Some(script);
    }
    if repo.is_empty() {
        return None;
    }
    let cat = ["-c".to_string(), "cat .autocoder/swarm/*.json 2>/dev/null".to_string()];
    let out = transport.output("sh", &cat, Some(Path::new(repo))).await.ok()?;
    listed(&String::from_utf8_lossy(&out.stdout), worktree).then_some(script)
}

/// The reason a release from the village gives.
pub fn release_reason(role: &str) -> String {
    format!("released by hand from the village (worker `{role}`)")
}

/// The verb and its arguments: the worker by its worktree, a release by its
/// issue. A nudge types `text`, the runtime's start-loop command, as the
/// village's own nudge does, rather than autocoder's default prose.
pub fn args(action: Action, worktree: &str, issue: Option<u32>, role: &str, text: Option<&str>) -> Vec<String> {
    let v = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    match action {
        Action::Nudge => match text {
            Some(t) => v(&["nudge", worktree, "--", t]),
            None => v(&["nudge", worktree]),
        },
        Action::Stop => v(&["stop", worktree]),
        Action::Restart => v(&["restart", worktree]),
        Action::Release => v(&["release", &issue.map(|n| n.to_string()).unwrap_or_default(), "--reason", &release_reason(role)]),
    }
}

/// The call as the confirmation shows it.
pub fn shown(args: &[String]) -> String {
    let quoted: Vec<String> = args
        .iter()
        .map(|a| if a.chars().all(|c| c.is_ascii_alphanumeric() || "/._-:".contains(c)) { a.clone() } else { format!("'{}'", a.replace('\'', "'\\''")) })
        .collect();
    format!("autocoder-ctl {}", quoted.join(" "))
}

/// What `autocoder-ctl` answered.
#[derive(Debug, Clone, PartialEq)]
pub enum Answer {
    /// Exit 3: this autocoder doesn't have the verb, so the caller does it itself.
    NotInstalled,
    /// Its result: `ok`, the exit code and `detail`.
    Result { ok: bool, exit: i32, detail: Value },
    /// It couldn't be run, or said nothing readable.
    Broken(String),
}

/// Read a run of the script: its exit code and its stdout (one JSON object).
pub fn read(exit: Option<i32>, stdout: &[u8], stderr: &[u8]) -> Answer {
    if exit == Some(3) {
        return Answer::NotInstalled;
    }
    let v: Value = serde_json::from_slice(stdout).unwrap_or(Value::Null);
    match v["ok"].as_bool() {
        Some(ok) => Answer::Result { ok, exit: exit.unwrap_or(-1), detail: v["detail"].clone() },
        None => {
            let err = String::from_utf8_lossy(stderr).trim().lines().last().unwrap_or("").to_string();
            Answer::Broken(if err.is_empty() { "autocoder-ctl gave no answer".into() } else { err })
        }
    }
}

/// Run the script in `cwd` (the swarm's repo: its manifests are there).
pub async fn call(transport: &ServerTransport, script: &Path, cwd: &Path, args: &[String]) -> Answer {
    let mut argv = vec![format!("AUTOCODER_CTL_ACTOR={ACTOR}"), "bash".to_string(), script.to_string_lossy().into_owned()];
    argv.extend(args.iter().cloned());
    match transport.output("env", &argv, Some(cwd)).await {
        Ok(out) => read(out.status.code(), &out.stdout, &out.stderr),
        Err(e) => Answer::Broken(e.to_string()),
    }
}

/// One line on what was done, from a successful result's `detail`.
pub fn said(action: Action, detail: &Value) -> String {
    let s = |k: &str| detail[k].as_str().unwrap_or("").to_string();
    let line = match action {
        Action::Nudge => match s("text") {
            t if t.is_empty() => String::new(),
            t => format!("typed into {}: {t}", detail["pane"].as_str().unwrap_or("its pane")),
        },
        Action::Release => {
            let issue = detail["issue"].as_str().map(str::to_string).or_else(|| detail["issue"].as_u64().map(|n| n.to_string())).unwrap_or_default();
            let comment = if detail["commented"].as_bool() == Some(false) { "; the comment could not be posted" } else { "" };
            format!("released #{issue}{comment}")
        }
        Action::Stop => {
            let left = match s("pane_left").as_str() {
                "shell" => "its pane is back at a shell",
                "closed" => "its pane is closed",
                _ => "stopped",
            };
            let claim = if detail["claim_kept"].as_bool() == Some(true) { "; its issue stays claimed: release it to hand it back" } else { "" };
            format!("{left}{claim}")
        }
        Action::Restart => s("output"),
    };
    match (line.is_empty(), detail) {
        (false, _) => line,
        (true, Value::String(t)) => t.clone(),
        (true, Value::Null) => String::new(),
        (true, d) => d.to_string(),
    }
}

/// The HTTP answer for a result. Done is `{ ok, action, via, said, detail }`;
/// a refusal (exit 1) is a 409 with autocoder's reason, bad arguments a 502.
pub fn response(action: Action, ok: bool, exit: i32, detail: &Value) -> Result<Value, (StatusCode, Value)> {
    let name = match action {
        Action::Nudge => "nudge",
        Action::Release => "release",
        Action::Stop => "stop",
        Action::Restart => "restart",
    };
    if ok {
        return Ok(json!({ "ok": true, "action": name, "via": "autocoder-ctl", "said": said(action, detail), "detail": detail }));
    }
    let why = match detail {
        Value::String(t) if !t.is_empty() => t.clone(),
        Value::Null => format!("autocoder-ctl {name} failed (exit {exit})"),
        d => d["output"].as_str().or(d["error"].as_str()).map(str::to_string).unwrap_or_else(|| d.to_string()),
    };
    let code = if exit == 1 { StatusCode::CONFLICT } else { StatusCode::BAD_GATEWAY };
    Err((code, json!({ "error": format!("autocoder-ctl: {why}"), "via": "autocoder-ctl", "detail": detail })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_control_is_its_verb_on_the_worktree_or_the_issue() {
        assert_eq!(args(Action::Nudge, "/src/p-wt-2", None, "worker-2", None), ["nudge", "/src/p-wt-2"]);
        assert_eq!(args(Action::Nudge, "/src/p-wt-2", None, "worker-2", Some("/autocoder:fix-loop")), ["nudge", "/src/p-wt-2", "--", "/autocoder:fix-loop"]);
        assert_eq!(args(Action::Stop, "/src/p-wt-2", Some(7), "worker-2", None), ["stop", "/src/p-wt-2"]);
        assert_eq!(args(Action::Restart, "/src/p-wt-2", Some(7), "worker-2", None), ["restart", "/src/p-wt-2"]);
        assert_eq!(args(Action::Release, "/src/p-wt-2", Some(42), "worker-2", None), ["release", "42", "--reason", "released by hand from the village (worker `worker-2`)"]);
        assert_eq!(shown(&args(Action::Release, "", Some(42), "w", None)), "autocoder-ctl release 42 --reason 'released by hand from the village (worker `w`)'");
        assert_eq!(shown(&["nudge".into(), "/src/it's".into()]), "autocoder-ctl nudge '/src/it'\\''s'");
    }

    /// A swarm the TUI launched has no manifest: autocoder-ctl can't find
    /// its workers, so the village drives them itself.
    #[test]
    fn only_a_worker_in_a_manifest_is_autocoders_to_drive() {
        let manifests = r#"{"session":"claude-p","workers":[{"worktree":"/src/p-wt-1","number":1},{"worktree":"/src/p-wt-2/","number":2}]}
{"session":"other","workers":[{"number":5}]}"#;
        assert!(listed(manifests, "/src/p-wt-1"));
        assert!(listed(manifests, "/src/p-wt-2"), "a trailing slash is the same worktree");
        assert!(!listed(manifests, "/src/p-wt-3"));
        assert!(!listed(manifests, "/src/p-wt"), "matched whole");
        assert!(!listed(manifests, ""), "no worktree: nothing to name the worker by");
        assert!(!listed("", "/src/p-wt-1"), "no manifest");
        assert!(!listed("not json", "/src/p-wt-1"));
    }

    #[test]
    fn exit_3_means_this_autocoder_cannot_and_the_village_does_it() {
        let out = br#"{"ok":false,"action":"restart","target":null,"detail":"not implemented"}"#;
        assert_eq!(read(Some(3), out, b""), Answer::NotInstalled);
        assert_eq!(read(Some(0), br#"{"ok":true,"action":"stop","target":"/w","detail":{"stopped":true}}"#, b""), Answer::Result { ok: true, exit: 0, detail: json!({ "stopped": true }) });
        assert_eq!(read(Some(1), br#"{"ok":false,"action":"stop","target":"/w","detail":"no manifest lists /w"}"#, b""), Answer::Result { ok: false, exit: 1, detail: json!("no manifest lists /w") });
        assert_eq!(read(Some(127), b"", b"bash: no such file\n"), Answer::Broken("bash: no such file".into()));
        assert_eq!(read(None, b"garbage", b""), Answer::Broken("autocoder-ctl gave no answer".into()));
    }

    #[test]
    fn a_done_result_says_what_it_did() {
        let nudge = json!({ "pane": "claude-p:1.2", "sent": true, "text": "Resume the autocoder fix loop" });
        let ok = response(Action::Nudge, true, 0, &nudge).unwrap();
        assert_eq!(ok["via"], "autocoder-ctl");
        assert_eq!(ok["said"], "typed into claude-p:1.2: Resume the autocoder fix loop");
        assert_eq!(ok["detail"], nudge, "the result is passed on whole");
        assert_eq!(said(Action::Release, &json!({ "issue": "17", "released": true, "commented": false })), "released #17; the comment could not be posted");
        assert_eq!(said(Action::Stop, &json!({ "pane_left": "shell", "claim_kept": true })), "its pane is back at a shell; its issue stays claimed: release it to hand it back");
        assert_eq!(said(Action::Restart, &json!({ "restarted": true, "output": "✅ Worker restarted" })), "✅ Worker restarted");
        assert_eq!(said(Action::Restart, &json!("done")), "done");
    }

    #[test]
    fn a_refusal_is_a_409_with_autocoders_reason() {
        let (code, body) = response(Action::Stop, false, 1, &json!("the pane is in /elsewhere, not /w")).unwrap_err();
        assert_eq!(code, StatusCode::CONFLICT);
        assert_eq!(body["error"], "autocoder-ctl: the pane is in /elsewhere, not /w");
        let (code, body) = response(Action::Restart, false, 1, &json!({ "restarted": false, "exit": 2, "output": "no pane" })).unwrap_err();
        assert_eq!(code, StatusCode::CONFLICT);
        assert_eq!(body["error"], "autocoder-ctl: no pane");
        assert_eq!(response(Action::Nudge, false, 2, &json!("usage: nudge <worktree>")).unwrap_err().0, StatusCode::BAD_GATEWAY);
    }

    /// The real script, faked: run with the actor set, in the repo, its JSON read.
    #[tokio::test]
    async fn the_script_runs_in_the_repo_with_the_village_as_actor() {
        let dir = std::env::temp_dir().join(format!("agents-ui-ctl-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let fake = dir.join("autocoder-ctl.sh");
        std::fs::write(&fake, "printf '{\"ok\":true,\"action\":\"%s\",\"target\":\"%s\",\"detail\":{\"actor\":\"%s\",\"cwd\":\"%s\"}}' \"$1\" \"$2\" \"$AUTOCODER_CTL_ACTOR\" \"$PWD\"\n").unwrap();
        let answer = call(&ServerTransport::default(), &fake, &dir, &args(Action::Stop, "/src/p-wt-2", None, "w", None)).await;
        let Answer::Result { ok, exit, detail } = answer else { panic!("{answer:?}") };
        assert!(ok && exit == 0);
        assert_eq!(detail["actor"], ACTOR);
        assert_eq!(std::fs::canonicalize(detail["cwd"].as_str().unwrap()).unwrap(), std::fs::canonicalize(&dir).unwrap());
        std::fs::write(&fake, "echo '{\"ok\":false,\"action\":\"x\",\"target\":null,\"detail\":\"not implemented\"}'; exit 3\n").unwrap();
        assert_eq!(call(&ServerTransport::default(), &fake, &dir, &["stop".into(), "/w".into()]).await, Answer::NotInstalled);
        let _ = std::fs::remove_dir_all(&dir);
        assert!(script(&ServerTransport::default()).is_none(), "tests never find the real script");
    }
}
