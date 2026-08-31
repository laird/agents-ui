use std::path::PathBuf;
use std::sync::OnceLock;

static CODEX_GOALS_SUPPORTED: OnceLock<bool> = OnceLock::new();

/// Returns true if the installed Codex CLI has the `goals` feature enabled.
/// Caches result in /tmp/codex-goals-probe-<version> across processes, and in memory for
/// the lifetime of this process (mirrors probe-codex-goals.sh behavior).
pub fn codex_supports_goals() -> bool {
    *CODEX_GOALS_SUPPORTED.get_or_init(|| {
        probe_codex_goals()
    })
}

fn probe_codex_goals() -> bool {
    // Get version string for cache key (spaces → hyphens, matching probe-codex-goals.sh)
    let version = std::process::Command::new("codex")
        .args(["--version"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().replace(' ', "-"))
        .unwrap_or_default();

    if !version.is_empty() {
        let cache_path = std::env::temp_dir().join(format!("codex-goals-probe-{version}"));
        if let Ok(cached) = std::fs::read_to_string(&cache_path) {
            return cached.trim() == "0";
        }
        let result = probe_goals_feature();
        let _ = std::fs::write(&cache_path, if result { "0" } else { "1" });
        return result;
    }

    probe_goals_feature()
}

fn probe_goals_feature() -> bool {
    let output = std::process::Command::new("codex")
        .args(["features", "list"])
        .output();
    match output {
        Ok(out) => parse_goals_feature(&String::from_utf8_lossy(&out.stdout)),
        Err(_) => false,
    }
}

fn parse_goals_feature(output: &str) -> bool {
    output.lines().any(|line| {
        let mut parts = line.split_whitespace();
        parts.next() == Some("goals") && parts.last() == Some("true")
    })
}

/// Resolve the agents plugin scripts directory.
///
/// `AGENTS_DIR` is consulted first, before any auto-detection. It used to be
/// checked fourth, which made it dead weight: an installed
/// `~/.claude/plugins/autocoder` always won, so the one knob available for
/// saying "use this checkout, not the installed copy" could not say it. That
/// silently ran stale scripts against a current repo.
///
/// The order also matters to anything without a meaningful working directory.
/// The `../agents` fallbacks resolve against the process cwd, which is `/` for
/// a systemd service, so `--headless` cannot rely on them at all -- it needs
/// `AGENTS_DIR` set, and needs it to win.
pub fn resolve_agents_dir() -> PathBuf {
    // 1. Explicit override always wins.
    if let Ok(dir) = std::env::var("AGENTS_DIR") {
        let path = PathBuf::from(&dir);
        // An AGENTS_DIR pointing at a repo root is the common way to write it;
        // accept that as well as a direct path to the plugin directory.
        let nested = path.join("plugins/autocoder");
        if nested.exists() {
            return std::fs::canonicalize(&nested).unwrap_or(nested);
        }
        if path.exists() {
            return std::fs::canonicalize(&path).unwrap_or(path);
        }
        tracing::warn!(
            "AGENTS_DIR is set to {} but that path does not exist; falling back to auto-detection",
            path.display()
        );
    }

    // 2. User's global Claude plugin install
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

    // 3. Relative to the project (../agents/plugins/autocoder/)
    let relative = PathBuf::from("../agents/plugins/autocoder");
    if relative.exists() {
        return std::fs::canonicalize(&relative).unwrap_or(relative);
    }

    // 4. Broader ../agents/ directory
    let agents = PathBuf::from("../agents");
    if agents.exists() {
        return std::fs::canonicalize(&agents).unwrap_or(agents);
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
/// Directories searched for a named script, in order.
///
/// `AGENTS_DIR` comes first for the same reason it does in `resolve_agents_dir`:
/// searched last, an explicit override is shadowed by any installed
/// `~/.claude/plugins/autocoder/scripts`, so pointing this at a checkout could
/// not actually override anything.
///
/// Both layouts are searched under each root. The autocoder plugin ships some
/// scripts under `plugins/autocoder/scripts/`, but the per-runtime loop scripts
/// (`codex-fix-loop.sh`, `droid-fix-loop.sh`, ...) live in the repo's top-level
/// `scripts/`. Dropping either root silently breaks one runtime's launch path.
fn script_search_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    // Explicit override first.
    if let Ok(dir) = std::env::var("AGENTS_DIR") {
        paths.push(PathBuf::from(&dir).join("plugins/autocoder/scripts"));
        paths.push(PathBuf::from(&dir).join("scripts"));
    }

    if let Some(home) = dirs::home_dir() {
        // Installed plugin scripts
        paths.push(home.join(".claude/plugins/autocoder/scripts"));
        paths.push(home.join(".config/claude-code/plugins/autocoder/scripts"));
    }

    // Relative to project. These resolve against the process cwd, so they find
    // nothing for a service started by systemd -- see resolve_agents_dir.
    paths.push(PathBuf::from("../agents/plugins/autocoder/scripts"));
    paths.push(PathBuf::from("../agents/scripts"));

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

        // AGENTS_DIR now outranks every auto-detected location. It used to be
        // consulted last, which meant an installed ~/.claude/plugins/autocoder
        // -- or merely running from a directory with an ../agents sibling --
        // shadowed the override entirely.
        let nested = tmp.join("plugins/autocoder");
        fs::create_dir_all(&nested).expect("create nested plugin dir");

        // SAFETY: test-only, single-threaded context; no other threads read AGENTS_DIR here.
        unsafe { std::env::set_var("AGENTS_DIR", tmp.to_str().unwrap()); }
        let paths = script_search_paths();
        let resolved = resolve_agents_dir();
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

        // Precedence, not just membership: the override must come before the
        // installed-plugin and cwd-relative candidates.
        assert_eq!(
            paths.first(),
            Some(&expected_scripts),
            "AGENTS_DIR must be searched first, got {paths:?}"
        );

        // resolve_agents_dir canonicalizes, so compare canonical forms.
        assert_eq!(
            resolved,
            fs::canonicalize(&nested).unwrap_or(nested),
            "AGENTS_DIR should win over auto-detection"
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
    fn probe_goals_reads_cached_file_when_present() {
        let tmp = std::env::temp_dir().join(format!(
            "codex-goals-probe-test-version-{}",
            std::process::id()
        ));
        // Write cache indicating supported (exit 0)
        fs::write(&tmp, "0").unwrap();
        // Read back and assert our convention: "0" => true
        let cached = fs::read_to_string(&tmp).unwrap();
        assert_eq!(cached.trim() == "0", true);
        // Write cache indicating not supported (exit 1)
        fs::write(&tmp, "1").unwrap();
        let cached = fs::read_to_string(&tmp).unwrap();
        assert_eq!(cached.trim() == "0", false);
        fs::remove_file(&tmp).ok();
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
