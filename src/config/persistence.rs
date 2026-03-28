#![allow(dead_code)]
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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
