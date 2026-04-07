use crate::model::swarm::AgentType;
use crate::scripts::launcher;

pub trait RuntimeSupervisor: Sync {
    fn uses_loop_wrapper(&self) -> bool {
        false
    }

    fn launch_command(&self, runtime: &AgentType, session_name: &str, is_manager: bool) -> String;

    fn wrapper_command(&self, is_manager: bool) -> Option<String> {
        let _ = is_manager;
        None
    }

    fn bootstrap_command(
        &self,
        runtime: &AgentType,
        is_manager: bool,
        issue: Option<u32>,
    ) -> Option<String>;

    fn start_manager_loop_command(&self, runtime: &AgentType) -> Option<String> {
        if self.uses_loop_wrapper() {
            self.wrapper_command(true)
        } else {
            self.bootstrap_command(runtime, true, None)
        }
    }

    fn ongoing_command(
        &self,
        runtime: &AgentType,
        is_manager: bool,
        issue: Option<u32>,
    ) -> Option<String>;

    fn start_worker_loop_command(&self, runtime: &AgentType) -> Option<String> {
        if self.uses_loop_wrapper() {
            self.wrapper_command(false)
        } else {
            let loop_cmd = runtime.worker_loop_cmd();
            if loop_cmd.is_empty() {
                None
            } else {
                Some(loop_cmd.to_string())
            }
        }
    }
}

pub fn for_runtime(runtime: &AgentType) -> &'static dyn RuntimeSupervisor {
    match runtime {
        AgentType::Claude => &ClaudeSupervisor,
        AgentType::Codex => &CodexSupervisor,
        AgentType::Droid => &DroidSupervisor,
        AgentType::Gemini => &GeminiSupervisor,
    }
}

struct ClaudeSupervisor;
struct CodexSupervisor;
struct DroidSupervisor;
struct GeminiSupervisor;

impl RuntimeSupervisor for ClaudeSupervisor {
    fn launch_command(
        &self,
        _runtime: &AgentType,
        _session_name: &str,
        _is_manager: bool,
    ) -> String {
        "claude code --dangerously-skip-permissions .".to_string()
    }

    fn bootstrap_command(
        &self,
        _runtime: &AgentType,
        is_manager: bool,
        issue: Option<u32>,
    ) -> Option<String> {
        if is_manager {
            Some("/autocoder:monitor-loop".to_string())
        } else if let Some(issue_number) = issue {
            Some(format!("/autocoder:fix {issue_number}"))
        } else {
            Some("/autocoder:fix-loop".to_string())
        }
    }

    fn ongoing_command(
        &self,
        _runtime: &AgentType,
        is_manager: bool,
        issue: Option<u32>,
    ) -> Option<String> {
        if is_manager {
            Some("/autocoder:monitor-workers".to_string())
        } else if let Some(issue_number) = issue {
            Some(format!("/autocoder:fix {issue_number}"))
        } else {
            Some("/autocoder:fix".to_string())
        }
    }
}

impl RuntimeSupervisor for CodexSupervisor {
    fn uses_loop_wrapper(&self) -> bool {
        launcher::find_script("codex-fix-loop.sh").is_some()
    }

    fn launch_command(
        &self,
        _runtime: &AgentType,
        _session_name: &str,
        is_manager: bool,
    ) -> String {
        self.wrapper_command(is_manager)
            .unwrap_or_else(|| "codex".to_string())
    }

    fn wrapper_command(&self, is_manager: bool) -> Option<String> {
        let script = if is_manager {
            "codex-manage-workers-loop.sh"
        } else {
            "codex-fix-loop.sh"
        };
        let path = launcher::find_script(script)?;
        Some(format!("bash {}", shell_quote(&path.to_string_lossy())))
    }

    fn bootstrap_command(
        &self,
        _runtime: &AgentType,
        is_manager: bool,
        issue: Option<u32>,
    ) -> Option<String> {
        if is_manager {
            Some(
                "Use the autocoder skill to start the monitor loop for this repository in this manager session."
                    .to_string(),
            )
        } else if let Some(issue_number) = issue {
            Some(format!(
                "Use the autocoder skill to fix issue #{issue_number} in this repository."
            ))
        } else {
            Some(
                "Use the autocoder skill to start the fix loop for this worker in the current repository."
                    .to_string(),
            )
        }
    }

    fn ongoing_command(
        &self,
        runtime: &AgentType,
        is_manager: bool,
        issue: Option<u32>,
    ) -> Option<String> {
        if is_manager {
            Some(
                "Use the autocoder skill to monitor workers for this repository in this manager session."
                    .to_string(),
            )
        } else if let Some(issue_number) = issue {
            self.bootstrap_command(runtime, false, Some(issue_number))
        } else {
            Some(runtime.worker_loop_cmd().to_string())
        }
    }
}

impl RuntimeSupervisor for GeminiSupervisor {
    fn uses_loop_wrapper(&self) -> bool {
        launcher::find_script("gemini-fix-loop.sh").is_some()
    }

    fn launch_command(
        &self,
        _runtime: &AgentType,
        _session_name: &str,
        is_manager: bool,
    ) -> String {
        self.wrapper_command(is_manager)
            .unwrap_or_else(|| "gemini --sandbox=false".to_string())
    }

    fn wrapper_command(&self, is_manager: bool) -> Option<String> {
        let script = if is_manager {
            "gemini-manage-workers-loop.sh"
        } else {
            "gemini-fix-loop.sh"
        };
        let path = launcher::find_script(script)?;
        Some(format!("bash {}", shell_quote(&path.to_string_lossy())))
    }

    fn bootstrap_command(
        &self,
        _runtime: &AgentType,
        is_manager: bool,
        issue: Option<u32>,
    ) -> Option<String> {
        if is_manager {
            Some("/monitor-loop".to_string())
        } else if let Some(issue_number) = issue {
            Some(format!("/fix {issue_number}"))
        } else {
            Some("/fix-loop".to_string())
        }
    }

    fn ongoing_command(
        &self,
        _runtime: &AgentType,
        is_manager: bool,
        issue: Option<u32>,
    ) -> Option<String> {
        if is_manager {
            Some("/monitor-workers".to_string())
        } else if let Some(issue_number) = issue {
            Some(format!("/fix {issue_number}"))
        } else {
            Some("/fix".to_string())
        }
    }
}

impl RuntimeSupervisor for DroidSupervisor {
    fn uses_loop_wrapper(&self) -> bool {
        launcher::find_script("droid-fix-loop.sh").is_some()
    }

    fn launch_command(
        &self,
        _runtime: &AgentType,
        _session_name: &str,
        is_manager: bool,
    ) -> String {
        self.wrapper_command(is_manager)
            .unwrap_or_else(|| "droid".to_string())
    }

    fn wrapper_command(&self, is_manager: bool) -> Option<String> {
        let script = if is_manager {
            "droid-manage-workers-loop.sh"
        } else {
            "droid-fix-loop.sh"
        };
        let path = launcher::find_script(script)?;
        Some(format!("bash {}", shell_quote(&path.to_string_lossy())))
    }

    fn bootstrap_command(
        &self,
        _runtime: &AgentType,
        is_manager: bool,
        issue: Option<u32>,
    ) -> Option<String> {
        if is_manager {
            Some("/monitor-loop".to_string())
        } else if let Some(issue_number) = issue {
            Some(format!("/fix {issue_number}"))
        } else {
            Some("/fix-loop".to_string())
        }
    }

    fn ongoing_command(
        &self,
        _runtime: &AgentType,
        is_manager: bool,
        issue: Option<u32>,
    ) -> Option<String> {
        if is_manager {
            Some("/monitor-workers".to_string())
        } else if let Some(issue_number) = issue {
            Some(format!("/fix {issue_number}"))
        } else {
            Some("/fix".to_string())
        }
    }
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_wrapper_usage_matches_expected_runtimes() {
        assert!(!for_runtime(&AgentType::Claude).uses_loop_wrapper());
        assert!(for_runtime(&AgentType::Codex).uses_loop_wrapper());
        assert!(for_runtime(&AgentType::Droid).uses_loop_wrapper());
        assert!(for_runtime(&AgentType::Gemini).uses_loop_wrapper());
    }

    // --- shell_quote ---

    #[test]
    fn shell_quote_simple_string() {
        assert_eq!(shell_quote("/path/to/script.sh"), "'/path/to/script.sh'");
    }

    #[test]
    fn shell_quote_escapes_single_quotes() {
        assert_eq!(shell_quote("it's"), "'it'\"'\"'s'");
    }

    #[test]
    fn shell_quote_empty_string() {
        assert_eq!(shell_quote(""), "''");
    }

    // --- ClaudeSupervisor bootstrap_command ---

    #[test]
    fn claude_bootstrap_manager_returns_monitor_loop() {
        let sup = ClaudeSupervisor;
        assert_eq!(
            sup.bootstrap_command(&AgentType::Claude, true, None),
            Some("/autocoder:monitor-loop".to_string())
        );
    }

    #[test]
    fn claude_bootstrap_worker_no_issue_returns_fix_loop() {
        let sup = ClaudeSupervisor;
        assert_eq!(
            sup.bootstrap_command(&AgentType::Claude, false, None),
            Some("/autocoder:fix-loop".to_string())
        );
    }

    #[test]
    fn claude_bootstrap_worker_with_issue_returns_fix_issue() {
        let sup = ClaudeSupervisor;
        assert_eq!(
            sup.bootstrap_command(&AgentType::Claude, false, Some(42)),
            Some("/autocoder:fix 42".to_string())
        );
    }

    // --- ClaudeSupervisor ongoing_command ---

    #[test]
    fn claude_ongoing_manager_returns_monitor_workers() {
        let sup = ClaudeSupervisor;
        assert_eq!(
            sup.ongoing_command(&AgentType::Claude, true, None),
            Some("/autocoder:monitor-workers".to_string())
        );
    }

    #[test]
    fn claude_ongoing_worker_no_issue_returns_fix() {
        let sup = ClaudeSupervisor;
        assert_eq!(
            sup.ongoing_command(&AgentType::Claude, false, None),
            Some("/autocoder:fix".to_string())
        );
    }

    #[test]
    fn claude_ongoing_worker_with_issue_returns_fix_issue() {
        let sup = ClaudeSupervisor;
        assert_eq!(
            sup.ongoing_command(&AgentType::Claude, false, Some(7)),
            Some("/autocoder:fix 7".to_string())
        );
    }

    // --- ClaudeSupervisor launch_command ---

    #[test]
    fn claude_launch_command_returns_expected_invocation() {
        let sup = ClaudeSupervisor;
        let cmd = sup.launch_command(&AgentType::Claude, "claude-myrepo", false);
        assert_eq!(cmd, "claude code --dangerously-skip-permissions .");
    }

    // --- start_manager_loop_command via trait (Claude: no wrapper) ---

    #[test]
    fn claude_start_manager_loop_uses_bootstrap_not_wrapper() {
        let sup = ClaudeSupervisor;
        // Claude does not use loop wrapper, so start_manager_loop_command delegates to bootstrap
        assert_eq!(
            sup.start_manager_loop_command(&AgentType::Claude),
            Some("/autocoder:monitor-loop".to_string())
        );
    }

    // --- GeminiSupervisor/DroidSupervisor bootstrap and ongoing (no filesystem deps) ---

    #[test]
    fn gemini_bootstrap_manager_returns_monitor_loop() {
        let sup = GeminiSupervisor;
        assert_eq!(
            sup.bootstrap_command(&AgentType::Gemini, true, None),
            Some("/monitor-loop".to_string())
        );
    }

    #[test]
    fn gemini_bootstrap_worker_with_issue() {
        let sup = GeminiSupervisor;
        assert_eq!(
            sup.bootstrap_command(&AgentType::Gemini, false, Some(99)),
            Some("/fix 99".to_string())
        );
    }

    #[test]
    fn gemini_ongoing_manager_returns_monitor_workers() {
        let sup = GeminiSupervisor;
        assert_eq!(
            sup.ongoing_command(&AgentType::Gemini, true, None),
            Some("/monitor-workers".to_string())
        );
    }

    #[test]
    fn droid_bootstrap_worker_no_issue_returns_fix_loop() {
        let sup = DroidSupervisor;
        assert_eq!(
            sup.bootstrap_command(&AgentType::Droid, false, None),
            Some("/fix-loop".to_string())
        );
    }

    #[test]
    fn droid_ongoing_worker_with_issue() {
        let sup = DroidSupervisor;
        assert_eq!(
            sup.ongoing_command(&AgentType::Droid, false, Some(5)),
            Some("/fix 5".to_string())
        );
    }

    // --- CodexSupervisor bootstrap (pure string returns, no filesystem) ---

    #[test]
    fn codex_bootstrap_manager_returns_prose_instruction() {
        let sup = CodexSupervisor;
        let cmd = sup.bootstrap_command(&AgentType::Codex, true, None);
        assert!(cmd.is_some());
        let cmd = cmd.unwrap();
        assert!(cmd.contains("monitor") || cmd.contains("manager"));
    }

    #[test]
    fn codex_bootstrap_worker_with_issue_contains_issue_number() {
        let sup = CodexSupervisor;
        let cmd = sup.bootstrap_command(&AgentType::Codex, false, Some(123));
        assert!(cmd.is_some());
        assert!(cmd.unwrap().contains("123"));
    }
}
