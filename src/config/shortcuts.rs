use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::PathBuf;

/// A single keyboard shortcut definition.
#[derive(Debug, Clone, Deserialize)]
pub struct Shortcut {
    /// Display label shown in help bar and viewer.
    pub label: String,
    /// Command template to send. Supports {issue}, {worker}, {project} variables.
    pub command: String,
    /// Where to send the command: "manager" (default) or "worker".
    #[serde(default = "default_target")]
    #[allow(dead_code)]
    pub target: String,
    /// If true, send as raw tmux key (e.g., "C-c") instead of text+Enter.
    #[serde(default)]
    #[allow(dead_code)]
    pub raw: bool,
}

fn default_target() -> String {
    "manager".to_string()
}

/// All shortcut panels loaded from config.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ShortcutsConfig {
    #[serde(default)]
    pub global: BTreeMap<String, Shortcut>,
    #[serde(default)]
    pub workers: BTreeMap<String, Shortcut>,
    #[serde(default)]
    pub issues: BTreeMap<String, Shortcut>,
    #[serde(default)]
    pub manager: BTreeMap<String, Shortcut>,
}

impl ShortcutsConfig {
    /// Load shortcuts from the config file, falling back to defaults.
    pub fn load() -> Self {
        let path = Self::config_path();
        if path.exists() {
            match std::fs::read_to_string(&path) {
                Ok(contents) => match toml::from_str::<ShortcutsConfig>(&contents) {
                    Ok(config) => {
                        tracing::info!("Loaded shortcuts from {}", path.display());
                        return config;
                    }
                    Err(e) => {
                        tracing::warn!("Invalid shortcuts config: {e}, using defaults");
                    }
                },
                Err(e) => {
                    tracing::warn!("Failed to read shortcuts config: {e}, using defaults");
                }
            }
        }
        Self::defaults()
    }

    /// Path to the shortcuts config file.
    pub fn config_path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".agents-ui")
            .join("shortcuts.toml")
    }

    /// Write default config file if it doesn't exist.
    pub fn ensure_defaults_written() {
        let path = Self::config_path();
        if !path.exists() {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(&path, DEFAULT_CONFIG);
            tracing::info!("Created default shortcuts config at {}", path.display());
        }
    }

    /// Built-in default shortcuts.
    pub fn defaults() -> Self {
        toml::from_str(DEFAULT_CONFIG).unwrap_or_default()
    }

    /// Get shortcuts for a given panel name.
    pub fn for_panel(&self, panel: &str) -> &BTreeMap<String, Shortcut> {
        match panel {
            "workers" => &self.workers,
            "issues" => &self.issues,
            "manager" => &self.manager,
            "global" => &self.global,
            _ => &self.global,
        }
    }

    /// Expand template variables in a command string.
    #[allow(dead_code)]
    pub fn expand_command(template: &str, issue: Option<u32>, worker: Option<&str>, project: Option<&str>) -> String {
        let mut cmd = template.to_string();
        if let Some(n) = issue {
            cmd = cmd.replace("{issue}", &n.to_string());
        }
        if let Some(w) = worker {
            cmd = cmd.replace("{worker}", w);
        }
        if let Some(p) = project {
            cmd = cmd.replace("{project}", p);
        }
        cmd
    }
}

const DEFAULT_CONFIG: &str = r#"# agents-ui keyboard shortcuts
# Edit this file to customize shortcuts. Changes take effect on restart.
#
# Panels: [global], [workers], [issues], [manager]
# Fields: label (display name), command (template), target ("manager" or "worker"), raw (bool)
# Variables: {issue} = selected issue number, {worker} = worker tmux target, {project} = project name

[issues]
a = { label = "approve", command = "gh issue edit {issue} --remove-label proposal" }
x = { label = "fix", command = "/autocoder:fix {issue}" }
b = { label = "brainstorm", command = "/brainstorm {issue}" }

[workers]
f = { label = "fix-loop", command = "/autocoder:fix-loop", target = "worker" }

[global]

[manager]
"#;
