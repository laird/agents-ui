use anyhow::{Context, Result};
use std::path::Path;
use tokio::process::Command;

/// What every remote command passes to `ssh` before the host (#587): never
/// prompt (a password or an unknown host key would otherwise ask on the
/// terminal the TUI holds in raw mode, and hang or garble it), and give up on
/// an unreachable host after 10s. ssh's own error is what the callers show.
pub const SSH_OPTIONS: [&str; 4] = ["-o", "BatchMode=yes", "-o", "ConnectTimeout=10"];

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ServerTransport {
    server: Option<String>,
    /// The program run in place of `ssh`, so a test can record what a remote
    /// command would have sent (#550). `None` runs `ssh`.
    ssh_program: Option<std::path::PathBuf>,
}

impl ServerTransport {
    pub fn new(server: Option<String>) -> Self {
        Self { server, ssh_program: None }
    }

    /// A remote transport whose commands run `program` instead of `ssh`.
    #[cfg(test)]
    pub fn with_ssh_program(server: &str, program: std::path::PathBuf) -> Self {
        Self { server: Some(server.to_string()), ssh_program: Some(program) }
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
            let ssh = self.ssh_program.as_deref().unwrap_or(Path::new("ssh"));
            Command::new(ssh)
                .args(SSH_OPTIONS)
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
    use super::{ServerTransport, build_shell_command};
    use std::path::Path;

    #[test]
    fn builds_remote_command_with_cwd() {
        let command = build_shell_command(
            "tmux",
            &[
                "list-sessions".to_string(),
                "-F".to_string(),
                "#{session_name}".to_string(),
            ],
            Some(Path::new("/srv/repo")),
        );

        assert_eq!(
            command,
            "cd '/srv/repo' && 'tmux' 'list-sessions' '-F' '#{session_name}'"
        );
    }

    #[tokio::test]
    async fn ssh_never_prompts_and_gives_up_on_an_unreachable_host() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(crate::testutil::artifact_name("transport-ssh-opts"));
        std::fs::create_dir_all(&dir).unwrap();
        let ssh = dir.join("ssh");
        std::fs::write(&ssh, "#!/bin/sh\nprintf '%s\\n' \"$@\"\n").unwrap();
        std::fs::set_permissions(&ssh, std::fs::Permissions::from_mode(0o755)).unwrap();
        let out = ServerTransport::with_ssh_program("buildbox", ssh).output("true", &[], None).await.unwrap();
        let args: Vec<String> = String::from_utf8_lossy(&out.stdout).lines().map(str::to_string).collect();
        assert_eq!(args, ["-o", "BatchMode=yes", "-o", "ConnectTimeout=10", "buildbox", "'true'"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn detects_remote_transport() {
        assert!(ServerTransport::new(Some("buildbox".to_string())).is_remote());
        assert!(!ServerTransport::new(None).is_remote());
    }
}
