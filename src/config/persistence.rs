#![allow(dead_code)]

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::model::swarm::AgentType;

#[allow(dead_code)] // Persistence layer planned for swarm state save/restore
/// Root configuration directory.
pub fn config_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("agents-ui")
}

/// Persisted swarm configuration.
#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct SwarmState {
    pub repo_path: String,
    pub agent_type: String,
    pub tmux_session: String,
    pub workflow: Option<String>,
    pub num_workers: u32,
}

#[allow(dead_code)]
/// Save swarm state to disk.
pub fn save_swarm_state(project_name: &str, state: &SwarmState) -> Result<()> {
    let dir = config_dir().join("swarms").join(project_name);
    std::fs::create_dir_all(&dir)?;
    let content = toml::to_string_pretty(state)?;
    std::fs::write(dir.join("swarm.toml"), content)?;
    Ok(())
}

#[allow(dead_code)]
/// Load swarm state from disk.
pub fn load_swarm_state(project_name: &str) -> Result<Option<SwarmState>> {
    let path = config_dir()
        .join("swarms")
        .join(project_name)
        .join("swarm.toml");
    if !path.exists() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(&path)?;
    let state: SwarmState = toml::from_str(&content)?;
    Ok(Some(state))
}

#[allow(dead_code)]
/// List all saved swarm states.
pub fn list_saved_swarms() -> Result<Vec<String>> {
    let dir = config_dir().join("swarms");
    if !dir.exists() {
        return Ok(vec![]);
    }
    let mut names = Vec::new();
    for entry in std::fs::read_dir(&dir)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            if let Some(name) = entry.file_name().to_str() {
                names.push(name.to_string());
            }
        }
    }
    Ok(names)
}

#[derive(Debug, Serialize, Deserialize)]
struct RepoConfig {
    default_agent_type: String,
}

fn repo_config_path(repo_root: &Path) -> PathBuf {
    repo_root.join(".agents-ui.toml")
}

pub fn find_repo_root(start: &Path) -> Option<PathBuf> {
    let mut current = if start.is_dir() {
        start.to_path_buf()
    } else {
        start.parent()?.to_path_buf()
    };

    loop {
        if current.join(".git").exists() {
            return Some(current);
        }
        current = current.parent()?.to_path_buf();
    }
}

pub fn load_repo_agent_type(repo_root: &Path) -> Result<Option<AgentType>> {
    let path = repo_config_path(repo_root);
    if !path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(path)?;
    let cfg: RepoConfig = toml::from_str(&content)?;
    Ok(AgentType::from_name(&cfg.default_agent_type))
}

pub fn save_repo_agent_type(repo_root: &Path, agent_type: &AgentType) -> Result<()> {
    let path = repo_config_path(repo_root);
    let cfg = RepoConfig {
        default_agent_type: agent_type.script_flag().to_string(),
    };
    let content = toml::to_string_pretty(&cfg)?;
    std::fs::write(path, content)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{find_repo_root, load_repo_agent_type, load_swarm_state, list_saved_swarms, save_repo_agent_type, save_swarm_state, SwarmState};
    use crate::model::swarm::AgentType;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!("agents-ui-{name}-{}-{nanos}", std::process::id()))
    }

    #[test]
    fn finds_repo_root_from_nested_path() {
        let root = temp_path("repo-root");
        let nested = root.join("a/b/c");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::create_dir_all(root.join(".git")).unwrap();

        let found = find_repo_root(&nested);
        assert_eq!(found, Some(root.clone()));

        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn saves_and_loads_repo_agent_type() {
        let root = temp_path("repo-config");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(root.join(".git")).unwrap();

        save_repo_agent_type(&root, &AgentType::Droid).unwrap();
        let loaded = load_repo_agent_type(&root).unwrap();
        assert_eq!(loaded, Some(AgentType::Droid));

        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn save_and_load_swarm_state_round_trips() {
        let tmp = temp_path("swarm-roundtrip");
        std::fs::create_dir_all(&tmp).unwrap();

        // Override config_dir by writing directly; test via the public API with a unique name
        let state = SwarmState {
            repo_path: "/home/user/myrepo".to_string(),
            agent_type: "claude".to_string(),
            tmux_session: "claude-myrepo".to_string(),
            workflow: Some("fix-loop".to_string()),
            num_workers: 3,
        };

        // Write using the public save function (uses config_dir internally), then read back
        save_swarm_state("test-roundtrip-project", &state).unwrap();
        let loaded = load_swarm_state("test-roundtrip-project").unwrap();

        assert!(loaded.is_some(), "should load saved swarm state");
        let loaded = loaded.unwrap();
        assert_eq!(loaded.repo_path, "/home/user/myrepo");
        assert_eq!(loaded.agent_type, "claude");
        assert_eq!(loaded.tmux_session, "claude-myrepo");
        assert_eq!(loaded.workflow, Some("fix-loop".to_string()));
        assert_eq!(loaded.num_workers, 3);

        std::fs::remove_dir_all(tmp).ok();
    }

    #[test]
    fn load_swarm_state_returns_none_for_missing_project() {
        let result = load_swarm_state("__nonexistent_project_xyz__").unwrap();
        assert!(result.is_none(), "missing project should return None");
    }

    #[test]
    fn list_saved_swarms_empty_when_dir_missing() {
        // list_saved_swarms reads from config_dir()/swarms; if that dir doesn't exist it returns []
        // We can only test this indirectly — the function must not panic and must return Ok
        // (The swarms dir may or may not exist on this machine, but the result must be Ok)
        let result = list_saved_swarms();
        assert!(result.is_ok(), "list_saved_swarms should not error even if dir is missing");
    }

    #[test]
    fn list_saved_swarms_returns_saved_names() {
        // Save two swarms and verify both appear in the listing
        let state = SwarmState {
            repo_path: "/tmp/r".to_string(),
            agent_type: "claude".to_string(),
            tmux_session: "s".to_string(),
            workflow: None,
            num_workers: 1,
        };
        save_swarm_state("list-test-alpha", &state).unwrap();
        save_swarm_state("list-test-beta", &state).unwrap();

        let names = list_saved_swarms().unwrap();
        assert!(names.contains(&"list-test-alpha".to_string()), "should contain list-test-alpha");
        assert!(names.contains(&"list-test-beta".to_string()), "should contain list-test-beta");
    }

    #[test]
    fn find_repo_root_returns_none_when_no_git_dir() {
        let tmp = temp_path("no-git");
        std::fs::create_dir_all(&tmp).unwrap();
        // No .git directory created

        let result = find_repo_root(&tmp);
        assert!(result.is_none(), "should return None when no .git directory found");

        std::fs::remove_dir_all(tmp).ok();
    }

    #[test]
    fn find_repo_root_from_file_path() {
        let root = temp_path("repo-file-path");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(root.join(".git")).unwrap();
        let file = root.join("somefile.txt");
        std::fs::write(&file, "hello").unwrap();

        let found = find_repo_root(&file);
        assert_eq!(found, Some(root.clone()), "should find repo root from a file path");

        std::fs::remove_dir_all(root).ok();
    }
}
