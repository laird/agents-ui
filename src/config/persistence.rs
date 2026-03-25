#![allow(dead_code)]

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::model::swarm::AgentType;

/// Root configuration directory.
pub fn config_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("agents-ui")
}

/// Persisted swarm configuration.
#[derive(Debug, Serialize, Deserialize)]
pub struct SwarmState {
    pub repo_path: String,
    pub agent_type: String,
    pub tmux_session: String,
    pub workflow: Option<String>,
    pub num_workers: u32,
}

/// Save swarm state to disk.
pub fn save_swarm_state(project_name: &str, state: &SwarmState) -> Result<()> {
    let dir = config_dir().join("swarms").join(project_name);
    std::fs::create_dir_all(&dir)?;
    let content = toml::to_string_pretty(state)?;
    std::fs::write(dir.join("swarm.toml"), content)?;
    Ok(())
}

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
    use super::{find_repo_root, load_repo_agent_type, save_repo_agent_type};
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
}
