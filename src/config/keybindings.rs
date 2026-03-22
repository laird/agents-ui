use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;

/// Named actions that can be bound to keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    // Global
    Quit,
    ForceQuit,
    Back,
    ShowHelp,

    // Navigation
    MoveUp,
    MoveDown,
    Select,

    // Repos list
    NewSwarm,
    Refresh,

    // Repo view
    Fullscreen,
    FocusManager,

    // Scrolling
    ScrollUp,
    ScrollDown,

    // Feedback
    FileFeedback,
}

impl fmt::Display for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Action::Quit => write!(f, "Quit"),
            Action::ForceQuit => write!(f, "Force Quit"),
            Action::Back => write!(f, "Back / Cancel"),
            Action::ShowHelp => write!(f, "Show Help"),
            Action::MoveUp => write!(f, "Move Up"),
            Action::MoveDown => write!(f, "Move Down"),
            Action::Select => write!(f, "Select / Confirm"),
            Action::NewSwarm => write!(f, "New Swarm"),
            Action::Refresh => write!(f, "Refresh"),
            Action::Fullscreen => write!(f, "Fullscreen"),
            Action::FocusManager => write!(f, "Focus Manager"),
            Action::ScrollUp => write!(f, "Scroll Up"),
            Action::ScrollDown => write!(f, "Scroll Down"),
            Action::FileFeedback => write!(f, "File Feedback"),
        }
    }
}

/// A single key binding: modifier + key.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct KeyBind {
    pub key: String,
    #[serde(default)]
    pub modifiers: Vec<String>,
}

impl fmt::Display for KeyBind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for m in &self.modifiers {
            write!(f, "{m}+")?;
        }
        write!(f, "{}", self.key)
    }
}

impl KeyBind {
    pub fn new(key: &str) -> Self {
        Self {
            key: key.to_string(),
            modifiers: vec![],
        }
    }

    pub fn ctrl(key: &str) -> Self {
        Self {
            key: key.to_string(),
            modifiers: vec!["ctrl".to_string()],
        }
    }

    /// Check if a KeyEvent matches this binding.
    pub fn matches(&self, event: &KeyEvent) -> bool {
        let key_matches = match event.code {
            KeyCode::Char(c) => self.key == c.to_string(),
            KeyCode::Enter => self.key == "enter",
            KeyCode::Esc => self.key == "esc",
            KeyCode::Tab => self.key == "tab",
            KeyCode::Backspace => self.key == "backspace",
            KeyCode::Up => self.key == "up",
            KeyCode::Down => self.key == "down",
            KeyCode::Left => self.key == "left",
            KeyCode::Right => self.key == "right",
            KeyCode::PageUp => self.key == "pageup",
            KeyCode::PageDown => self.key == "pagedown",
            KeyCode::Home => self.key == "home",
            KeyCode::End => self.key == "end",
            KeyCode::Delete => self.key == "delete",
            KeyCode::F(n) => self.key == format!("f{n}"),
            _ => false,
        };

        if !key_matches {
            return false;
        }

        let needs_ctrl = self.modifiers.iter().any(|m| m == "ctrl");
        let needs_alt = self.modifiers.iter().any(|m| m == "alt");
        let needs_shift = self.modifiers.iter().any(|m| m == "shift");

        let has_ctrl = event.modifiers.contains(KeyModifiers::CONTROL);
        let has_alt = event.modifiers.contains(KeyModifiers::ALT);
        let has_shift = event.modifiers.contains(KeyModifiers::SHIFT);

        needs_ctrl == has_ctrl && needs_alt == has_alt && needs_shift == has_shift
    }
}

/// The full keybindings configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyBindings {
    pub bindings: HashMap<Action, Vec<KeyBind>>,
}

impl Default for KeyBindings {
    fn default() -> Self {
        let mut bindings = HashMap::new();

        // Global
        bindings.insert(Action::Quit, vec![KeyBind::new("q")]);
        bindings.insert(Action::ForceQuit, vec![KeyBind::ctrl("c")]);
        bindings.insert(Action::Back, vec![KeyBind::new("esc")]);
        bindings.insert(Action::ShowHelp, vec![KeyBind::new("?")]);

        // Navigation
        bindings.insert(
            Action::MoveUp,
            vec![KeyBind::new("up"), KeyBind::new("k")],
        );
        bindings.insert(
            Action::MoveDown,
            vec![KeyBind::new("down"), KeyBind::new("j")],
        );
        bindings.insert(Action::Select, vec![KeyBind::new("enter")]);

        // Repos list
        bindings.insert(Action::NewSwarm, vec![KeyBind::new("n")]);
        bindings.insert(Action::Refresh, vec![KeyBind::new("r")]);

        // Repo view
        bindings.insert(
            Action::Fullscreen,
            vec![KeyBind::new("f"), KeyBind::new("F")],
        );
        bindings.insert(Action::FocusManager, vec![KeyBind::new("m")]);

        // Scrolling
        bindings.insert(Action::ScrollUp, vec![KeyBind::new("pageup")]);
        bindings.insert(Action::ScrollDown, vec![KeyBind::new("pagedown")]);

        // Feedback
        bindings.insert(Action::FileFeedback, vec![KeyBind {
            key: "f".to_string(),
            modifiers: vec!["alt".to_string()],
        }]);

        Self { bindings }
    }
}

impl KeyBindings {
    /// Check if a key event matches any binding for the given action.
    pub fn matches(&self, action: Action, event: &KeyEvent) -> bool {
        self.bindings
            .get(&action)
            .map(|binds| binds.iter().any(|b| b.matches(event)))
            .unwrap_or(false)
    }

    /// Get display string for an action's bindings (e.g., "q", "up/k").
    pub fn display(&self, action: Action) -> String {
        self.bindings
            .get(&action)
            .map(|binds| {
                binds
                    .iter()
                    .map(|b| b.to_string())
                    .collect::<Vec<_>>()
                    .join("/")
            })
            .unwrap_or_else(|| "unbound".to_string())
    }

    /// Load keybindings from config file, falling back to defaults.
    pub fn load() -> Self {
        let path = config_path();
        if path.exists() {
            match std::fs::read_to_string(&path) {
                Ok(content) => match toml::from_str::<KeyBindings>(&content) {
                    Ok(kb) => {
                        tracing::info!("Loaded keybindings from {}", path.display());
                        return kb;
                    }
                    Err(e) => {
                        tracing::warn!("Failed to parse keybindings: {e}, using defaults");
                    }
                },
                Err(e) => {
                    tracing::warn!("Failed to read keybindings: {e}, using defaults");
                }
            }
        }
        Self::default()
    }

    /// Save current keybindings to config file.
    #[allow(dead_code)] // Public API for users to save customized keybindings
    pub fn save(&self) -> Result<()> {
        let path = config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self)?;
        std::fs::write(&path, content)?;
        tracing::info!("Saved keybindings to {}", path.display());
        Ok(())
    }

    /// Get all bindings as sorted (action, display_keys) pairs for the help overlay.
    pub fn help_entries(&self) -> Vec<(String, String)> {
        let actions = [
            Action::ForceQuit,
            Action::Quit,
            Action::Back,
            Action::ShowHelp,
            Action::MoveUp,
            Action::MoveDown,
            Action::Select,
            Action::NewSwarm,
            Action::Refresh,
            Action::Fullscreen,
            Action::FocusManager,
            Action::ScrollUp,
            Action::ScrollDown,
            Action::FileFeedback,
        ];

        actions
            .iter()
            .map(|a| (a.to_string(), self.display(*a)))
            .collect()
    }
}

/// Path to the keybindings config file.
fn config_path() -> PathBuf {
    super::persistence::config_dir()
        .join("keybindings.toml")
}
