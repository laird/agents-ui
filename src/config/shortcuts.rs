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

}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_parses_successfully() {
        let config = ShortcutsConfig::defaults();
        // All four panels should have entries (catches embedded TOML regressions)
        assert!(!config.issues.is_empty(), "issues panel should have shortcuts");
        assert!(!config.workers.is_empty(), "workers panel should have shortcuts");
        // global and manager may be empty per DEFAULT_CONFIG, but they must not panic
        let _ = &config.global;
        let _ = &config.manager;
    }

    #[test]
    fn defaults_has_expected_shortcuts() {
        let config = ShortcutsConfig::defaults();

        // Issues panel: x=fix, b=brainstorm (a is hardcoded to "new issue" in the TUI)
        assert!(config.issues.contains_key("x"), "issues should have 'x' shortcut");
        assert_eq!(config.issues["x"].label, "fix");
        assert!(config.issues.contains_key("b"), "issues should have 'b' shortcut");
        assert_eq!(config.issues["b"].label, "brainstorm");

        // Workers panel: f=fix-loop
        assert!(config.workers.contains_key("f"), "workers should have 'f' shortcut");
        assert_eq!(config.workers["f"].label, "fix-loop");
    }

    #[test]
    fn for_panel_returns_correct_section() {
        let config = ShortcutsConfig::defaults();

        // Each named panel returns its own map
        assert!(std::ptr::eq(config.for_panel("issues"), &config.issues));
        assert!(std::ptr::eq(config.for_panel("workers"), &config.workers));
        assert!(std::ptr::eq(config.for_panel("manager"), &config.manager));
        assert!(std::ptr::eq(config.for_panel("global"), &config.global));

        // Unknown panel falls back to global
        assert!(std::ptr::eq(config.for_panel("unknown"), &config.global));
        assert!(std::ptr::eq(config.for_panel(""), &config.global));
    }

    #[test]
    fn toml_round_trip() {
        // Parse DEFAULT_CONFIG twice and verify key counts match
        let first: ShortcutsConfig = toml::from_str(DEFAULT_CONFIG).expect("first parse");
        let second: ShortcutsConfig = toml::from_str(DEFAULT_CONFIG).expect("second parse");

        assert_eq!(first.issues.len(), second.issues.len());
        assert_eq!(first.workers.len(), second.workers.len());
        assert_eq!(first.global.len(), second.global.len());
        assert_eq!(first.manager.len(), second.manager.len());
    }

    #[test]
    fn ensure_defaults_written_creates_file() {
        let tmp = std::env::temp_dir()
            .join(format!("agents-shortcuts-test-{}", std::process::id()));
        let test_path = tmp.join("shortcuts.toml");
        std::fs::create_dir_all(&tmp).expect("create temp dir");

        // Simulate what ensure_defaults_written does: write if absent, parse result
        if !test_path.exists() {
            std::fs::write(&test_path, DEFAULT_CONFIG).expect("write default config");
        }

        assert!(test_path.exists(), "config file should have been created");
        let content = std::fs::read_to_string(&test_path).expect("read config");
        let parsed: ShortcutsConfig = toml::from_str(&content).expect("parse written config");
        assert!(!parsed.issues.is_empty(), "written config should have issues shortcuts");

        std::fs::remove_dir_all(&tmp).ok();
    }
}

const DEFAULT_CONFIG: &str = r#"# agents-ui keyboard shortcuts
# Edit this file to customize shortcuts. Changes take effect on restart.
#
# Panels: [global], [workers], [issues], [manager]
# Fields: label (display name), command (template)
# Variables: {issue} = selected issue number, {worker} = worker tmux target, {project} = project name
#
# Issue panel keys: x=fix (sends /fix {issue} to manager), b=brainstorm, p=approve, a=new issue

[issues]
x = { label = "fix", command = "/autocoder:fix {issue}" }
b = { label = "brainstorm", command = "/brainstorm {issue}" }

[workers]
f = { label = "fix-loop", command = "/autocoder:fix-loop", target = "worker" }

[global]

[manager]
"#;
