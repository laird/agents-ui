//! Acting on the tracker through autocoder's `autocoder-ctl` (#541,
//! laird/agents#192): the board's proposal decisions (`proposals.rs`) and
//! filing an issue (`newissue.rs`) call `autocoder-ctl.sh approve | reject |
//! request-changes | file` in the swarm's repo, so they work with whatever
//! issue backend autocoder is configured for (GitHub, its file store, Jira,
//! Azure DevOps), not only `gh`.
//!
//! The contract is laird/agents `docs/autocoder-ctl.md`: one JSON object on
//! stdout, `{"ok", "action", "target", "detail"}`, and exit 0 (done), 1 (the
//! action failed), 2 (usage) or 3 (the verb is not in this autocoder). The
//! script is found where agents-tui finds autocoder's other scripts
//! (`launcher::find_script`). Missing, or answering 3, it is `Unavailable`, and
//! the caller falls back to its own `gh` commands, for an autocoder too old to
//! have the verbs.
//!
//! The actor autocoder names in its comments is `$USER (village board)`.

use std::path::Path;

use serde_json::Value;

use crate::transport::ServerTransport;

pub const SCRIPT: &str = "autocoder-ctl.sh";

/// How an `autocoder-ctl` call went.
#[derive(Debug, Clone, PartialEq)]
pub enum CtlOutcome {
    /// Done: the result's `target` and `detail`.
    Done { target: Option<String>, detail: Value },
    /// It ran and refused or failed; the words say why.
    Failed(String),
    /// No `autocoder-ctl`, or one without this verb: use the fallback.
    Unavailable,
}

/// The installed `autocoder-ctl.sh`, if any.
pub fn find() -> Option<std::path::PathBuf> {
    crate::scripts::launcher::find_script(SCRIPT)
}

/// Who autocoder says acted.
pub fn actor() -> String {
    let user = std::env::var("USER").ok().filter(|u| !u.trim().is_empty()).unwrap_or_else(|| "someone".into());
    format!("{user} (village board)")
}

/// The program and arguments that run `script` with `verb_args` (no shell).
pub fn command(script: &Path, actor: &str, verb_args: &[String]) -> (&'static str, Vec<String>) {
    let mut args = vec![format!("AUTOCODER_CTL_ACTOR={actor}"), "bash".into(), script.display().to_string()];
    args.extend(verb_args.iter().cloned());
    ("env", args)
}

/// Reads a finished call: its exit code and stdout.
pub fn outcome(code: Option<i32>, stdout: &[u8]) -> CtlOutcome {
    // 3: this autocoder lacks the verb. 127: bash found no script (a remote
    // host without autocoder).
    if matches!(code, Some(3) | Some(127)) {
        return CtlOutcome::Unavailable;
    }
    let text = String::from_utf8_lossy(stdout);
    let result: Option<Value> = text.lines().rev().find_map(|l| serde_json::from_str::<Value>(l.trim()).ok().filter(Value::is_object));
    let Some(result) = result else {
        return CtlOutcome::Failed(format!("autocoder-ctl gave no result (exit {})", code.map_or("none".into(), |c| c.to_string())));
    };
    let detail = result.get("detail").cloned().unwrap_or(Value::Null);
    if code == Some(0) && result["ok"] == Value::Bool(true) {
        let target = match &result["target"] {
            Value::String(s) => Some(s.clone()),
            Value::Number(n) => Some(n.to_string()),
            _ => None,
        };
        return CtlOutcome::Done { target, detail };
    }
    CtlOutcome::Failed(match detail {
        Value::String(s) if !s.trim().is_empty() => s,
        Value::Null => "autocoder-ctl refused it".into(),
        other => other.to_string(),
    })
}

/// Runs `script verb_args…` in `repo`. `script` is `None` when no
/// `autocoder-ctl` is installed.
pub async fn run(transport: &ServerTransport, repo: &Path, script: Option<&Path>, verb_args: &[String]) -> CtlOutcome {
    let Some(script) = script else { return CtlOutcome::Unavailable };
    let (program, args) = command(script, &actor(), verb_args);
    match transport.output(program, &args, Some(repo)).await {
        Ok(out) => {
            let got = outcome(out.status.code(), &out.stdout);
            if let CtlOutcome::Failed(why) = &got {
                tracing::warn!("autocoder-ctl {}: {why}", verb_args.first().map_or("", String::as_str));
            }
            got
        }
        Err(e) => CtlOutcome::Failed(format!("could not run autocoder-ctl: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn runs_the_script_through_env_with_each_value_its_own_argument() {
        let (program, args) = command(Path::new("/s/autocoder-ctl.sh"), "ann (village board)", &["reject".into(), "7".into(), "no; rm -rf $HOME".into()]);
        assert_eq!(program, "env");
        assert_eq!(args, vec!["AUTOCODER_CTL_ACTOR=ann (village board)", "bash", "/s/autocoder-ctl.sh", "reject", "7", "no; rm -rf $HOME"]);
    }

    #[test]
    fn a_done_result_gives_its_target_and_detail() {
        let out = br#"{"ok":true,"action":"file","target":"42","detail":{"number":42}}"#;
        assert_eq!(outcome(Some(0), out), CtlOutcome::Done { target: Some("42".into()), detail: json!({ "number": 42 }) });
    }

    #[test]
    fn a_failure_says_why() {
        let out = br#"{"ok":false,"action":"approve","target":"7","detail":"approve: issue #7 is not a proposal (no 'proposal' label)"}"#;
        assert_eq!(outcome(Some(1), out), CtlOutcome::Failed("approve: issue #7 is not a proposal (no 'proposal' label)".into()));
        assert!(matches!(outcome(Some(2), br#"{"ok":false,"detail":"usage"}"#), CtlOutcome::Failed(_)));
        assert!(matches!(outcome(Some(1), b"bash: something broke"), CtlOutcome::Failed(_)));
        // ok:true with a failing exit is still a failure.
        assert!(matches!(outcome(Some(1), br#"{"ok":true}"#), CtlOutcome::Failed(_)));
    }

    #[test]
    fn an_autocoder_without_the_verb_falls_back() {
        let out = br#"{"ok":false,"action":"file","target":null,"detail":"not implemented"}"#;
        assert_eq!(outcome(Some(3), out), CtlOutcome::Unavailable);
        assert_eq!(outcome(Some(127), b""), CtlOutcome::Unavailable);
    }

    /// A stand-in `autocoder-ctl.sh` that records its arguments and actor,
    /// then answers as `body` says.
    fn fake_ctl(tag: &str, body: &str) -> (std::path::PathBuf, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(crate::testutil::artifact_name(tag));
        std::fs::create_dir_all(&dir).unwrap();
        let script = dir.join(SCRIPT);
        let log = dir.join("args");
        std::fs::write(&script, format!("printf '%s\\n' \"$AUTOCODER_CTL_ACTOR\" \"$@\" > '{}'\n{body}\n", log.display())).unwrap();
        (dir, script)
    }

    #[tokio::test]
    async fn runs_the_installed_script_in_the_repo() {
        let (dir, script) = fake_ctl("ctl-ok", r#"echo '{"ok":true,"action":"approve","target":"7","detail":{"approved":true}}'"#);
        let got = run(&ServerTransport::new(None), &dir, Some(&script), &["approve".into(), "7".into()]).await;
        assert_eq!(got, CtlOutcome::Done { target: Some("7".into()), detail: json!({ "approved": true }) });
        let seen = std::fs::read_to_string(dir.join("args")).unwrap();
        assert!(seen.ends_with("(village board)\napprove\n7\n"), "{seen}");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn an_old_or_missing_autocoder_ctl_is_unavailable() {
        let (dir, script) = fake_ctl("ctl-old", r#"echo '{"ok":false,"detail":"not implemented"}'; exit 3"#);
        let transport = ServerTransport::new(None);
        assert_eq!(run(&transport, &dir, Some(&script), &["file".into()]).await, CtlOutcome::Unavailable);
        assert_eq!(run(&transport, &dir, None, &["file".into()]).await, CtlOutcome::Unavailable);
        std::fs::remove_dir_all(&dir).ok();
    }
}
