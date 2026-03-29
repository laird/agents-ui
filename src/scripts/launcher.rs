use std::path::PathBuf;

/// Resolve the agents plugin scripts directory.
/// Checks installed plugin locations first, then falls back to relative paths.
pub fn resolve_agents_dir() -> PathBuf {
    // 1. User's global Claude plugin install (primary)
    if let Some(home) = dirs::home_dir() {
        let installed = home.join(".claude/plugins/autocoder");
        if installed.exists() {
            return installed;
        }

        // Alt personal config path
        let alt = home.join(".config/claude-code/plugins/autocoder");
        if alt.exists() {
            return alt;
        }
    }

    // 2. Relative to the project (../agents/plugins/autocoder/)
    let relative = PathBuf::from("../agents/plugins/autocoder");
    if relative.exists() {
        return std::fs::canonicalize(&relative).unwrap_or(relative);
    }

    // 3. Broader ../agents/ directory
    let agents = PathBuf::from("../agents");
    if agents.exists() {
        return std::fs::canonicalize(&agents).unwrap_or(agents);
    }

    // 4. Environment variable override
    if let Ok(dir) = std::env::var("AGENTS_DIR") {
        let path = PathBuf::from(&dir);
        if path.exists() {
            return path;
        }
    }

    // Fall back
    if let Some(home) = dirs::home_dir() {
        let candidate = home.join(".claude/plugins/autocoder");
        return candidate;
    }

    PathBuf::from("../agents/plugins/autocoder")
}

#[allow(dead_code)]
/// Find a specific script, searching installed plugin paths.
pub fn find_script(name: &str) -> Option<PathBuf> {
    let search_paths = script_search_paths();
    for dir in search_paths {
        let candidate = dir.join(name);
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}

#[allow(dead_code)]
/// All directories where scripts might live, in priority order.
fn script_search_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    if let Some(home) = dirs::home_dir() {
        // Installed plugin scripts
        paths.push(home.join(".claude/plugins/autocoder/scripts"));
        paths.push(home.join(".config/claude-code/plugins/autocoder/scripts"));
    }

    // Relative to project
    paths.push(PathBuf::from("../agents/plugins/autocoder/scripts"));
    paths.push(PathBuf::from("../agents/scripts"));

    // Environment override
    if let Ok(dir) = std::env::var("AGENTS_DIR") {
        paths.push(PathBuf::from(&dir).join("plugins/autocoder/scripts"));
        paths.push(PathBuf::from(&dir).join("scripts"));
    }

    paths
}
