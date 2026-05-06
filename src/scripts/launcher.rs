use std::path::PathBuf;
use std::sync::OnceLock;

static CODEX_GOALS_SUPPORTED: OnceLock<bool> = OnceLock::new();

/// Returns true if the installed Codex CLI has the `goals` feature enabled.
/// Result is cached for the lifetime of the process.
pub fn codex_supports_goals() -> bool {
    *CODEX_GOALS_SUPPORTED.get_or_init(|| {
        let output = std::process::Command::new("codex")
            .args(["features", "list"])
            .output();
        match output {
            Ok(out) => parse_goals_feature(&String::from_utf8_lossy(&out.stdout)),
            Err(_) => false,
        }
    })
}

fn parse_goals_feature(output: &str) -> bool {
    output.lines().any(|line| {
        let mut parts = line.split_whitespace();
        parts.next() == Some("goals") && parts.last() == Some("true")
    })
}

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
            return Some(std::fs::canonicalize(&candidate).unwrap_or(candidate));
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn resolve_agents_dir_returns_path() {
        // Must always return some path (never panics)
        let path = resolve_agents_dir();
        // Path should be non-empty
        assert!(path.components().count() > 0);
    }

    #[test]
    fn resolve_agents_dir_env_override_used_when_exists() {
        let tmp = std::env::temp_dir()
            .join(format!("agents-launcher-test-{}", std::process::id()));
        fs::create_dir_all(&tmp).expect("create temp dir");

        // AGENTS_DIR only affects resolve_agents_dir if NO earlier path exists.
        // We can confirm it's in the search paths by checking script_search_paths.
        // SAFETY: test-only, single-threaded context; no other threads read AGENTS_DIR here.
        unsafe { std::env::set_var("AGENTS_DIR", tmp.to_str().unwrap()); }
        let paths = script_search_paths();
        unsafe { std::env::remove_var("AGENTS_DIR"); }

        let expected_scripts = tmp.join("plugins/autocoder/scripts");
        let expected_plain = tmp.join("scripts");
        assert!(
            paths.contains(&expected_scripts),
            "Expected {expected_scripts:?} in search paths"
        );
        assert!(
            paths.contains(&expected_plain),
            "Expected {expected_plain:?} in search paths"
        );

        fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn find_script_returns_none_when_not_found() {
        // Script that certainly doesn't exist
        assert_eq!(find_script("__nonexistent_script_xyz__.sh"), None);
    }

    #[test]
    fn find_script_returns_path_when_found() {
        let tmp = std::env::temp_dir()
            .join(format!("agents-launcher-find-{}", std::process::id()));
        let scripts_dir = tmp.join("plugins/autocoder/scripts");
        fs::create_dir_all(&scripts_dir).expect("create scripts dir");
        let script_file = scripts_dir.join("test-script.sh");
        fs::write(&script_file, "#!/bin/sh\n").expect("write script");

        // SAFETY: test-only, single-threaded context; no other threads read AGENTS_DIR here.
        unsafe { std::env::set_var("AGENTS_DIR", tmp.to_str().unwrap()); }
        let result = find_script("test-script.sh");
        unsafe { std::env::remove_var("AGENTS_DIR"); }

        let canonical_script = script_file.canonicalize().ok();
        assert_eq!(result, canonical_script);

        fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn script_search_paths_includes_home_subdirs() {
        let paths = script_search_paths();
        if let Some(home) = dirs::home_dir() {
            assert!(
                paths.contains(&home.join(".claude/plugins/autocoder/scripts")),
                "Should include ~/.claude/plugins/autocoder/scripts"
            );
        }
    }

    #[test]
    fn codex_supports_goals_returns_bool_without_panic() {
        let _ = codex_supports_goals();
    }

    #[test]
    fn parse_goals_line_detects_true() {
        assert!(parse_goals_feature(
            "goals                               under development  true\n"
        ));
    }

    #[test]
    fn parse_goals_line_detects_false() {
        assert!(!parse_goals_feature(
            "goals                               under development  false\n"
        ));
    }

    #[test]
    fn parse_goals_line_returns_false_for_missing() {
        assert!(!parse_goals_feature(""));
    }

    #[test]
    fn parse_goals_line_does_not_match_substrings() {
        assert!(!parse_goals_feature(
            "multi_agent_goals                   under development  true\n"
        ));
    }
}
