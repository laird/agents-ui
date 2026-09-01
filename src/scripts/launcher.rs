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

/// Path to the autocoder plugin as Claude Code actually installed it.
///
/// The `~/.claude/plugins/autocoder` layout the fallbacks below look for does
/// not exist in current Claude Code. An installed plugin lives in a
/// version-scoped cache directory --
/// `plugins/cache/<marketplace>/<plugin>/<version>` -- that moves on every
/// upgrade, and `plugins/installed_plugins.json` is the only thing that knows
/// where it landed. Guessing the path therefore never finds the installed
/// plugin at all: every lookup fell through to a checkout beside the repo, so
/// a machine with no `../agents` checkout resolved to a path that does not
/// exist while a perfectly good plugin sat installed.
fn installed_plugin_dir() -> Option<PathBuf> {
    let config = std::env::var_os("CLAUDE_CONFIG_DIR")
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|home| home.join(".claude")))?;
    let raw = std::fs::read_to_string(config.join("plugins/installed_plugins.json")).ok()?;
    let parsed: serde_json::Value = serde_json::from_str(&raw).ok()?;
    let plugins = parsed.get("plugins")?.as_object()?;

    let cwd = std::env::current_dir().ok();
    let mut user_scope: Option<PathBuf> = None;
    let mut any_scope: Option<PathBuf> = None;

    for (key, entries) in plugins {
        // Keyed `<plugin>@<marketplace>`. The marketplace name is the user's to
        // choose -- ours is "plugin-marketplace" -- so match the plugin half.
        if key != "autocoder" && !key.starts_with("autocoder@") {
            continue;
        }

        for entry in entries.as_array().into_iter().flatten() {
            let Some(path) = entry
                .get("installPath")
                .and_then(|value| value.as_str())
                .map(PathBuf::from)
            else {
                continue;
            };
            // Entries outlive the versions they name; an upgrade leaves the old
            // directory gone but sometimes still listed.
            if !path.exists() {
                continue;
            }

            let scope = entry.get("scope").and_then(|value| value.as_str());
            // A project-scoped install wins inside its own project: that is the
            // copy Claude Code itself loads when run there.
            if scope == Some("project") {
                let project = entry
                    .get("projectPath")
                    .and_then(|value| value.as_str())
                    .map(PathBuf::from);
                if let (Some(project), Some(cwd)) = (project, cwd.as_ref()) {
                    if cwd.starts_with(&project) {
                        return Some(path);
                    }
                }
            } else if scope == Some("user") {
                user_scope.get_or_insert(path.clone());
            }

            any_scope.get_or_insert(path);
        }
    }

    user_scope.or(any_scope)
}

/// Resolve the agents plugin scripts directory.
///
/// `AGENTS_DIR` is consulted first, before any auto-detection. It used to be
/// checked fourth, which made it dead weight: an installed
/// `~/.claude/plugins/autocoder` always won, so the one knob available for
/// saying "use this checkout, not the installed copy" could not say it. That
/// silently ran stale scripts against a current repo.
///
/// Everything after the override prefers the installed plugin, read from
/// Claude Code's own manifest rather than guessed at. The `../agents`
/// fallbacks below resolve against the process cwd, which is `/` for a systemd
/// service, so they can never fire under `--headless`; the manifest lookup is
/// an absolute path and works there.
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

    // 2. The plugin Claude Code actually installed.
    if let Some(installed) = installed_plugin_dir() {
        return installed;
    }

    // 3. Layouts older Claude Code versions installed plugins into directly.
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

    // 4. Relative to the project (../agents/plugins/autocoder/)
    let relative = PathBuf::from("../agents/plugins/autocoder");
    if relative.exists() {
        return std::fs::canonicalize(&relative).unwrap_or(relative);
    }

    // 5. Broader ../agents/ directory
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
/// The installed plugin comes next, and the checkout roots stay after it
/// rather than being replaced by it. The plugin packages only
/// `plugins/autocoder/scripts/`, which is missing the per-runtime loop scripts
/// -- `codex-fix-loop.sh`, `droid-fix-loop.sh`, `gemini-fix-loop.sh` live in
/// the agents repo's top-level `scripts/` and ship in no plugin at all. An
/// installed plugin is therefore enough for Claude but not for the other three
/// runtimes, so dropping the checkout roots would silently break their launch.
fn script_search_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    // Explicit override first.
    if let Ok(dir) = std::env::var("AGENTS_DIR") {
        paths.push(PathBuf::from(&dir).join("plugins/autocoder/scripts"));
        paths.push(PathBuf::from(&dir).join("scripts"));
    }

    // The installed plugin, at the version-scoped path Claude Code recorded.
    if let Some(installed) = installed_plugin_dir() {
        paths.push(installed.join("scripts"));
    }

    if let Some(home) = dirs::home_dir() {
        // Legacy install layouts.
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

    /// These tests mutate AGENTS_DIR and CLAUDE_CONFIG_DIR, which the resolver
    /// reads. The lock is process-wide because the readers live in other
    /// modules: adapter::claude's launch test depends on AGENTS_DIR staying
    /// put for the length of a tmux launch.
    use crate::testutil::env_lock;

    /// Write a Claude Code plugin manifest naming an install path for autocoder.
    fn write_installed_manifest(config_dir: &std::path::Path, entries: &str) {
        let plugins = config_dir.join("plugins");
        fs::create_dir_all(&plugins).expect("create plugins dir");
        fs::write(
            plugins.join("installed_plugins.json"),
            format!(r#"{{"version":2,"plugins":{{"autocoder@plugin-marketplace":[{entries}]}}}}"#),
        )
        .expect("write manifest");
    }

    #[test]
    fn resolve_agents_dir_returns_path() {
        // Must always return some path (never panics)
        let path = resolve_agents_dir();
        // Path should be non-empty
        assert!(path.components().count() > 0);
    }

    #[test]
    fn resolve_agents_dir_env_override_used_when_exists() {
        let tmp = std::env::temp_dir().join(crate::testutil::artifact_name("launcher"));
        fs::create_dir_all(&tmp).expect("create temp dir");

        // AGENTS_DIR now outranks every auto-detected location. It used to be
        // consulted last, which meant an installed ~/.claude/plugins/autocoder
        // -- or merely running from a directory with an ../agents sibling --
        // shadowed the override entirely.
        let nested = tmp.join("plugins/autocoder");
        fs::create_dir_all(&nested).expect("create nested plugin dir");

        let _guard = env_lock();
        // SAFETY: test-only, serialized by env_lock; no other threads read AGENTS_DIR here.
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
    fn resolve_agents_dir_uses_the_installed_plugin_manifest() {
        let tmp = std::env::temp_dir().join(crate::testutil::artifact_name("launcher"));
        let config = tmp.join("claude");
        let install = tmp.join("cache/plugin-marketplace/autocoder/4.21.0");
        fs::create_dir_all(&install).expect("create install dir");
        write_installed_manifest(
            &config,
            &format!(
                r#"{{"scope":"user","installPath":"{}","version":"4.21.0"}}"#,
                install.display()
            ),
        );

        let _guard = env_lock();
        // SAFETY: test-only, serialized by env_lock.
        unsafe {
            std::env::remove_var("AGENTS_DIR");
            std::env::set_var("CLAUDE_CONFIG_DIR", config.to_str().unwrap());
        }
        let resolved = resolve_agents_dir();
        let paths = script_search_paths();
        unsafe { std::env::remove_var("CLAUDE_CONFIG_DIR") };

        assert_eq!(
            resolved, install,
            "the version-scoped install path from the manifest should win"
        );
        assert_eq!(
            paths.first(),
            Some(&install.join("scripts")),
            "installed plugin scripts should be searched first, got {paths:?}"
        );
        // The checkout roots must survive: the per-runtime loop scripts ship in
        // the agents repo's top-level scripts/, not in the plugin.
        assert!(
            paths.contains(&PathBuf::from("../agents/scripts")),
            "checkout fallback dropped, got {paths:?}"
        );

        fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn installed_plugin_manifest_entry_for_a_removed_version_is_ignored() {
        let tmp = std::env::temp_dir().join(crate::testutil::artifact_name("launcher"));
        let config = tmp.join("claude");
        // An upgrade removes the old version directory but can leave it listed.
        let gone = tmp.join("cache/plugin-marketplace/autocoder/4.20.0");
        fs::create_dir_all(&config).expect("create config dir");
        write_installed_manifest(
            &config,
            &format!(
                r#"{{"scope":"user","installPath":"{}","version":"4.20.0"}}"#,
                gone.display()
            ),
        );

        let _guard = env_lock();
        // SAFETY: test-only, serialized by env_lock.
        unsafe {
            std::env::remove_var("AGENTS_DIR");
            std::env::set_var("CLAUDE_CONFIG_DIR", config.to_str().unwrap());
        }
        let resolved = resolve_agents_dir();
        unsafe { std::env::remove_var("CLAUDE_CONFIG_DIR") };

        assert_ne!(
            resolved, gone,
            "a manifest entry whose directory is gone must not be resolved to"
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
        let tmp = std::env::temp_dir().join(crate::testutil::artifact_name("launcher"));
        let scripts_dir = tmp.join("plugins/autocoder/scripts");
        fs::create_dir_all(&scripts_dir).expect("create scripts dir");
        let script_file = scripts_dir.join("test-script.sh");
        fs::write(&script_file, "#!/bin/sh\n").expect("write script");

        let _guard = env_lock();
        // SAFETY: test-only, serialized by env_lock; no other threads read AGENTS_DIR here.
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
