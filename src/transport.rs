use anyhow::{Context, Result};
use std::path::Path;
use tokio::process::Command;

/// What every remote command passes to `ssh` before the host (#587): never
/// prompt (a password or an unknown host key would otherwise ask on the
/// terminal the TUI holds in raw mode, and hang or garble it; a host not yet
/// in `known_hosts` now fails until someone runs `ssh <host>` once), give up
/// when the connect (and banner exchange) takes more than 10s, drop a
/// connection whose host stops answering for 30s (two missed keepalives 15s
/// apart: a dead link or a host gone away, #589), and end the options with
/// `--` so a `--server` value starting with `-` is never read as one. ssh's
/// own error is what the callers show. A remote command that hangs while its
/// host is alive is not caught here: sshd answers the keepalives. Read-only
/// callers bound that with [`ServerTransport::read`] (#590).
pub const SSH_OPTIONS: [&str; 9] = [
    "-o", "BatchMode=yes", "-o", "ConnectTimeout=10", "-o", "ServerAliveInterval=15", "-o", "ServerAliveCountMax=2", "--",
];

/// How long [`ServerTransport::read`] waits for a read-only command (#590):
/// a `git` or `gh` stuck on a stalled mount or network, on a host that still
/// answers ssh's keepalives, gives up here instead of holding swarm discovery
/// or the issue board forever. Long enough for `gh issue list --limit 200`
/// on a slow link.
pub const READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

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

    /// Run a command and wait for it however long it takes. For commands
    /// that change something (`autocoder-ctl start`, which runs for minutes;
    /// an issue comment or label): killing one partway could leave half a
    /// swarm or half an edit, so nothing here times it out. A command that
    /// only reads uses [`Self::read`].
    pub async fn output(
        &self,
        program: &str,
        args: &[String],
        current_dir: Option<&Path>,
    ) -> Result<std::process::Output> {
        self.command(program, args, current_dir).output().await.with_context(|| self.failed(program))
    }

    /// Run a read-only command, giving up after [`READ_TIMEOUT`] (#590).
    pub async fn read(&self, program: &str, args: &[String], current_dir: Option<&Path>) -> Result<std::process::Output> {
        self.read_within(READ_TIMEOUT, program, args, current_dir).await
    }

    /// Run a read-only command, giving up after `limit`. The process it
    /// started (`ssh`, or the local program) is killed when it runs out of
    /// time, since a read can be asked again. Over ssh that frees this side
    /// only: the stuck command on the host is left to its own fate.
    pub async fn read_within(
        &self,
        limit: std::time::Duration,
        program: &str,
        args: &[String],
        current_dir: Option<&Path>,
    ) -> Result<std::process::Output> {
        let mut cmd = self.command(program, args, current_dir);
        cmd.kill_on_drop(true);
        match tokio::time::timeout(limit, cmd.output()).await {
            Ok(out) => out.with_context(|| self.failed(program)),
            Err(_) => Err(anyhow::anyhow!("{}: no answer in {limit:?}", self.failed(program))),
        }
    }

    /// The command that runs `program` here, or on the server over ssh.
    fn command(&self, program: &str, args: &[String], current_dir: Option<&Path>) -> Command {
        if let Some(server) = &self.server {
            let ssh = self.ssh_program.as_deref().unwrap_or(Path::new("ssh"));
            let mut cmd = Command::new(ssh);
            cmd.args(SSH_OPTIONS).arg(server).arg(build_shell_command(program, args, current_dir));
            cmd
        } else {
            let mut cmd = Command::new(program);
            cmd.args(args);
            if let Some(dir) = current_dir {
                cmd.current_dir(dir);
            }
            cmd
        }
    }

    fn failed(&self, program: &str) -> String {
        match &self.server {
            Some(server) => format!("Failed to run remote command over ssh: {server}"),
            None => format!("Failed to run command: {program}"),
        }
    }

    pub async fn command_exists(&self, program: &str) -> bool {
        self.read(
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
        self.read(
            "test",
            &["-e".to_string(), path.to_string_lossy().to_string()],
            None,
        )
        .await
        .map(|output| output.status.success())
        .unwrap_or(false)
    }

    pub async fn dir_exists(&self, path: &Path) -> bool {
        self.read(
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
        let dir = std::env::temp_dir().join(crate::testutil::artifact_name("transport-ssh-opts"));
        std::fs::create_dir_all(&dir).unwrap();
        let ssh = dir.join("ssh");
        crate::testutil::write_executable(&ssh, "#!/bin/sh\nprintf '%s\\n' \"$@\"\n");
        let out = ServerTransport::with_ssh_program("buildbox", ssh).output("true", &[], None).await.unwrap();
        let args: Vec<String> = String::from_utf8_lossy(&out.stdout).lines().map(str::to_string).collect();
        assert_eq!(
            args,
            ["-o", "BatchMode=yes", "-o", "ConnectTimeout=10", "-o", "ServerAliveInterval=15", "-o", "ServerAliveCountMax=2", "--", "buildbox", "'true'"]
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A read on a host that answers ssh but whose command never returns
    /// gives up, and the stuck ssh is killed rather than left behind (#590).
    #[tokio::test]
    async fn a_read_that_hangs_on_a_live_host_gives_up_and_kills_ssh() {
        let dir = std::env::temp_dir().join(crate::testutil::artifact_name("transport-read-hang"));
        std::fs::create_dir_all(&dir).unwrap();
        let ssh = dir.join("ssh");
        let pid_file = dir.join("pid");
        crate::testutil::write_executable(&ssh, &format!("#!/bin/sh\necho $$ > '{}'\nexec sleep 30\n", pid_file.display()));
        let transport = ServerTransport::with_ssh_program("buildbox", ssh);
        let started = std::time::Instant::now();
        let err = transport
            .read_within(std::time::Duration::from_millis(500), "git", &["status".to_string()], None)
            .await
            .unwrap_err();
        assert!(started.elapsed() < std::time::Duration::from_secs(10), "it waited {:?}", started.elapsed());
        assert_eq!(format!("{err:#}"), "Failed to run remote command over ssh: buildbox: no answer in 500ms");
        let mut pid = String::new();
        for _ in 0..50 {
            pid = std::fs::read_to_string(&pid_file).unwrap_or_default().trim().to_string();
            if !pid.is_empty() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        assert!(!pid.is_empty(), "the stand-in ssh never started");
        // Killed: gone, or a zombie until tokio reaps it.
        let mut alive = true;
        for _ in 0..50 {
            let ps = std::process::Command::new("ps").args(["-o", "stat=", "-p", &pid]).output().unwrap();
            let stat = String::from_utf8_lossy(&ps.stdout).trim().to_string();
            if stat.is_empty() || stat.starts_with('Z') {
                alive = false;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        assert!(!alive, "ssh {pid} still running after its read gave up");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn a_read_that_answers_in_time_is_the_commands_output() {
        let dir = std::env::temp_dir().join(crate::testutil::artifact_name("transport-read-ok"));
        std::fs::create_dir_all(&dir).unwrap();
        let ssh = dir.join("ssh");
        crate::testutil::write_executable(&ssh, "#!/bin/sh\nprintf '%s\\n' \"$@\"\n");
        let out = ServerTransport::with_ssh_program("buildbox", ssh).read("git", &["status".to_string()], Some(Path::new("/r"))).await.unwrap();
        let args: Vec<String> = String::from_utf8_lossy(&out.stdout).lines().map(str::to_string).collect();
        assert_eq!(args[..9], super::SSH_OPTIONS, "a read sends the same ssh options");
        assert_eq!(args[9..], ["buildbox", "cd '/r' && 'git' 'status'"]);
        let local = ServerTransport::default().read("sh", &["-c".to_string(), "echo here".to_string()], None).await.unwrap();
        assert_eq!(String::from_utf8_lossy(&local.stdout), "here\n");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn detects_remote_transport() {
        assert!(ServerTransport::new(Some("buildbox".to_string())).is_remote());
        assert!(!ServerTransport::new(None).is_remote());
    }
}
