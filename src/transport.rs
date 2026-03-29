use anyhow::{Context, Result};
use std::path::Path;
use tokio::process::Command;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ServerTransport {
    server: Option<String>,
}

impl ServerTransport {
    pub fn new(server: Option<String>) -> Self {
        Self { server }
    }

    pub fn server(&self) -> Option<&str> {
        self.server.as_deref()
    }

    pub fn is_remote(&self) -> bool {
        self.server.is_some()
    }

    pub async fn output(
        &self,
        program: &str,
        args: &[String],
        current_dir: Option<&Path>,
    ) -> Result<std::process::Output> {
        if let Some(server) = &self.server {
            let command = build_shell_command(program, args, current_dir);
            Command::new("ssh")
                .arg(server)
                .arg(&command)
                .output()
                .await
                .with_context(|| format!("Failed to run remote command over ssh: {server}"))
        } else {
            let mut cmd = Command::new(program);
            cmd.args(args);
            if let Some(dir) = current_dir {
                cmd.current_dir(dir);
            }
            cmd.output()
                .await
                .with_context(|| format!("Failed to run command: {program}"))
        }
    }

    pub async fn command_exists(&self, program: &str) -> bool {
        self.output(
            "sh",
            &[
                "-lc".to_string(),
                format!("command -v {} >/dev/null 2>&1", shell_quote(program)),
            ],
            None,
        )
        .await
        .map(|output| output.status.success())
        .unwrap_or(false)
    }

    pub async fn path_exists(&self, path: &Path) -> bool {
        self.output(
            "test",
            &["-e".to_string(), path.to_string_lossy().to_string()],
            None,
        )
        .await
        .map(|output| output.status.success())
        .unwrap_or(false)
    }

    pub async fn dir_exists(&self, path: &Path) -> bool {
        self.output(
            "test",
            &["-d".to_string(), path.to_string_lossy().to_string()],
            None,
        )
        .await
        .map(|output| output.status.success())
        .unwrap_or(false)
    }
}

fn build_shell_command(program: &str, args: &[String], current_dir: Option<&Path>) -> String {
    let mut command = String::new();
    if let Some(dir) = current_dir {
        command.push_str("cd ");
        command.push_str(&shell_quote(&dir.to_string_lossy()));
        command.push_str(" && ");
    }

    command.push_str(&shell_quote(program));
    for arg in args {
        command.push(' ');
        command.push_str(&shell_quote(arg));
    }
    command
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

#[cfg(test)]
mod tests {
    use super::{build_shell_command, ServerTransport};
    use std::path::Path;

    #[test]
    fn builds_remote_command_with_cwd() {
        let command = build_shell_command(
            "tmux",
            &["list-sessions".to_string(), "-F".to_string(), "#{session_name}".to_string()],
            Some(Path::new("/srv/repo")),
        );

        assert_eq!(
            command,
            "cd '/srv/repo' && 'tmux' 'list-sessions' '-F' '#{session_name}'"
        );
    }

    #[test]
    fn detects_remote_transport() {
        assert!(ServerTransport::new(Some("buildbox".to_string())).is_remote());
        assert!(!ServerTransport::new(None).is_remote());
    }
}
