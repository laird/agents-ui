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
        session_name: &str,
        _is_manager: bool,
    ) -> String {
        format!(
            "claude --dangerously-skip-permissions --append-system-prompt 'This session is managed by agents-ui via tmux. \
IMPORTANT: Always use tmux commands (tmux capture-pane, tmux send-keys, etc.) \
for reading worker screens and dispatching work. Do NOT use cmux. \
The tmux session is named {session_name}.'"
        )
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
            Some("/autocoder:monitor".to_string())
        } else if let Some(issue_number) = issue {
            Some(format!("/autocoder:fix {issue_number}"))
        } else {
            Some("/autocoder:fix".to_string())
        }
    }
}

impl RuntimeSupervisor for CodexSupervisor {
    fn uses_loop_wrapper(&self) -> bool {
        true
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
    fn launch_command(
        &self,
        runtime: &AgentType,
        _session_name: &str,
        _is_manager: bool,
    ) -> String {
        runtime.launch_cmd().to_string()
    }

    fn bootstrap_command(
        &self,
        _runtime: &AgentType,
        is_manager: bool,
        issue: Option<u32>,
    ) -> Option<String> {
        if is_manager {
            Some("/manage-loop".to_string())
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
            Some("/monitor".to_string())
        } else if let Some(issue_number) = issue {
            Some(format!("/fix {issue_number}"))
        } else {
            Some("/fix".to_string())
        }
    }
}

impl RuntimeSupervisor for DroidSupervisor {
    fn uses_loop_wrapper(&self) -> bool {
        true
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
            Some("/manage-loop".to_string())
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
            Some("/monitor".to_string())
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
        assert!(!for_runtime(&AgentType::Gemini).uses_loop_wrapper());
    }
}
