use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

use crate::model::swarm::AgentType;

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

/// Ensure runtime-specific autocoder skills exist in the target repo.
///
/// For Codex: installs `skills/autocoder` when missing.
/// For Droid: installs `.factory/skills/autocoder` when missing.
/// Claude/Gemini do not require repo-local skill installation.
pub fn ensure_runtime_skills(
    repo_path: &Path,
    agent_type: &AgentType,
    agents_dir: &Path,
) -> Result<()> {
    match agent_type {
        AgentType::Codex => ensure_skill_from_candidates(
            repo_path,
            Path::new("skills/autocoder"),
            &codex_skill_candidates(agents_dir),
            "Codex",
        ),
        AgentType::Droid => ensure_skill_from_candidates(
            repo_path,
            Path::new(".factory/skills/autocoder"),
            &droid_skill_candidates(agents_dir),
            "Droid",
        ),
        AgentType::Claude | AgentType::Gemini => Ok(()),
    }
}

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

fn ensure_skill_from_candidates(
    repo_path: &Path,
    target_relative: &Path,
    source_candidates: &[PathBuf],
    runtime_name: &str,
) -> Result<()> {
    let target = repo_path.join(target_relative);
    if target.exists() {
        return Ok(());
    }

    let source = source_candidates
        .iter()
        .find(|p| p.is_dir())
        .cloned()
        .with_context(|| {
            let searched = source_candidates
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "{runtime_name} autocoder skill not found; searched: {searched}. \
Run the {runtime_name} installer from the shared agents repo and retry."
            )
        })?;

    copy_dir_recursive(&source, &target).with_context(|| {
        format!(
            "Failed to install {runtime_name} autocoder skill into {}",
            target.display()
        )
    })?;
    tracing::info!(
        "Installed {runtime_name} autocoder skill: {} -> {}",
        source.display(),
        target.display()
    );
    Ok(())
}

fn copy_dir_recursive(source: &Path, target: &Path) -> Result<()> {
    std::fs::create_dir_all(target)
        .with_context(|| format!("Failed to create directory {}", target.display()))?;

    for entry in std::fs::read_dir(source)
        .with_context(|| format!("Failed to read directory {}", source.display()))?
    {
        let entry =
            entry.with_context(|| format!("Failed to read entry in {}", source.display()))?;
        let file_type = entry
            .file_type()
            .with_context(|| format!("Failed to inspect {}", entry.path().display()))?;
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());

        if file_type.is_dir() {
            copy_dir_recursive(&source_path, &target_path)?;
        } else {
            std::fs::copy(&source_path, &target_path).with_context(|| {
                format!(
                    "Failed to copy {} -> {}",
                    source_path.display(),
                    target_path.display()
                )
            })?;
        }
    }

    Ok(())
}

fn codex_skill_candidates(agents_dir: &Path) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(home) = dirs::home_dir() {
        candidates.push(home.join(".codex/skills/autocoder"));
    }
    for root in agents_root_candidates(agents_dir) {
        candidates.push(root.join("skills/autocoder"));
    }
    candidates
}

fn droid_skill_candidates(agents_dir: &Path) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(home) = dirs::home_dir() {
        candidates.push(home.join(".factory/skills/autocoder"));
    }
    for root in agents_root_candidates(agents_dir) {
        candidates.push(root.join(".factory/skills/autocoder"));
    }
    candidates
}

fn agents_root_candidates(agents_dir: &Path) -> Vec<PathBuf> {
    let mut roots = vec![agents_dir.to_path_buf()];
    if let Some(parent) = agents_dir.parent() {
        roots.push(parent.to_path_buf());
        if let Some(grandparent) = parent.parent() {
            roots.push(grandparent.to_path_buf());
        }
    }
    if let Ok(dir) = std::env::var("AGENTS_DIR") {
        roots.push(PathBuf::from(dir));
    }
    roots.push(PathBuf::from("../agents"));

    // Best-effort dedupe while preserving order.
    let mut deduped = Vec::new();
    for root in roots {
        let canonical = std::fs::canonicalize(&root).unwrap_or(root);
        if !deduped.contains(&canonical) {
            deduped.push(canonical);
        }
    }
    deduped
}

#[cfg(test)]
mod tests {
    use super::ensure_skill_from_candidates;
    use std::path::{Path, PathBuf};

    fn temp_path(prefix: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time went backwards")
            .as_nanos();
        std::env::temp_dir().join(format!("{prefix}-{}-{nanos}", std::process::id()))
    }

    #[test]
    fn installs_missing_skill_tree_from_candidates() {
        let root = temp_path("launcher-install-skill");
        let repo = root.join("repo");
        let source = root.join("agents/skills/autocoder/references");
        std::fs::create_dir_all(&repo).expect("create repo");
        std::fs::create_dir_all(&source).expect("create source");
        std::fs::write(root.join("agents/skills/autocoder/SKILL.md"), "# autocoder")
            .expect("write skill");
        std::fs::write(source.join("workflow-map.md"), "map").expect("write reference");

        let result = ensure_skill_from_candidates(
            &repo,
            Path::new("skills/autocoder"),
            &[root.join("agents/skills/autocoder")],
            "Codex",
        );

        assert!(result.is_ok(), "expected install to succeed: {result:?}");
        assert!(repo.join("skills/autocoder/SKILL.md").exists());
        assert!(
            repo.join("skills/autocoder/references/workflow-map.md")
                .exists()
        );

        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn returns_error_when_skill_is_missing() {
        let root = temp_path("launcher-missing-skill");
        let repo = root.join("repo");
        std::fs::create_dir_all(&repo).expect("create repo");

        let result = ensure_skill_from_candidates(
            &repo,
            Path::new("skills/autocoder"),
            &[root.join("does/not/exist")],
            "Codex",
        );

        assert!(result.is_err(), "expected missing-skill error");

        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn keeps_existing_target_unchanged() {
        let root = temp_path("launcher-existing-skill");
        let repo = root.join("repo");
        let target = repo.join("skills/autocoder");
        std::fs::create_dir_all(&target).expect("create target");
        std::fs::write(target.join("SKILL.md"), "existing").expect("write target skill");

        let result = ensure_skill_from_candidates(
            &repo,
            Path::new("skills/autocoder"),
            &[root.join("missing-source")],
            "Codex",
        );

        assert!(result.is_ok(), "existing target should short-circuit");
        let content = std::fs::read_to_string(target.join("SKILL.md")).expect("read skill");
        assert_eq!(content, "existing");

        std::fs::remove_dir_all(root).ok();
    }
}
