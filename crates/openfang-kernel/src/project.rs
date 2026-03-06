//! Project-local `.openfang/` directory initialization and management.

use std::path::Path;

/// The default `.gitignore` content for a project's `.openfang/` directory.
/// Protects runtime state, secrets, and user-specific files from being committed.
const PROJECT_GITIGNORE: &str = r#"# OpenFang project-local runtime state -- DO NOT COMMIT
# This file is auto-generated. Edit with care.

# Runtime workspaces (per-user agent state)
workspaces/

# Database and state files
*.db
*.db-wal
*.db-shm
state/

# Secrets
.env
.env.*
*.key
*.pem
vault.enc

# User-specific identity files (generated per-user, not team-shared)
# Note: SOUL.md and TOOLS.md in agents/ directories ARE meant to be shared.
# Only top-level USER.md should be ignored.
USER.md
"#;

/// Initialize a project's `.openfang/` directory with scaffold and `.gitignore`.
///
/// Creates:
/// - `.openfang/.gitignore`
/// - `.openfang/hands/` (empty)
/// - `.openfang/agents/` (empty)
///
/// Returns `Ok(true)` if the directory was created, `Ok(false)` if it already existed.
pub fn init_project_dir(project_dir: &Path) -> std::io::Result<bool> {
    let openfang_dir = project_dir.join(".openfang");
    let already_existed = openfang_dir.is_dir();

    // Create directory structure
    std::fs::create_dir_all(openfang_dir.join("hands"))?;
    std::fs::create_dir_all(openfang_dir.join("agents"))?;

    // Write .gitignore (only if it doesn't exist — don't overwrite user edits)
    let gitignore_path = openfang_dir.join(".gitignore");
    if !gitignore_path.exists() {
        std::fs::write(&gitignore_path, PROJECT_GITIGNORE)?;
    }

    Ok(!already_existed)
}

/// Resolve project directories from config and CLI, with CWD fallback.
///
/// Priority order:
/// 1. `cli_projects` — from `--project` CLI flags (highest priority)
/// 2. `config_projects` — from `config.toml` `project_dirs` field
/// 3. CWD — only if neither of the above provided anything and CWD has `.openfang/`
pub fn resolve_project_dirs(
    cli_projects: &[std::path::PathBuf],
    config_projects: &[std::path::PathBuf],
) -> Vec<std::path::PathBuf> {
    let mut dirs = Vec::new();
    let mut seen = std::collections::HashSet::new();

    // CLI flags take priority
    for p in cli_projects {
        let expanded = expand_path(p);
        if seen.insert(expanded.clone()) {
            dirs.push(expanded);
        }
    }

    // Config entries
    for p in config_projects {
        let expanded = expand_path(p);
        if seen.insert(expanded.clone()) {
            dirs.push(expanded);
        }
    }

    // CWD fallback only if nothing else was provided
    if dirs.is_empty() {
        if let Ok(cwd) = std::env::current_dir() {
            if cwd.join(".openfang").is_dir() && seen.insert(cwd.clone()) {
                dirs.push(cwd);
            }
        }
    }

    dirs
}

/// Expand `~` in paths to the user's home directory.
fn expand_path(p: &Path) -> std::path::PathBuf {
    if let Ok(stripped) = p.strip_prefix("~") {
        if let Some(home) = dirs::home_dir() {
            return home.join(stripped);
        }
    }
    p.to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_init_project_dir_creates_scaffold() {
        let tmp = TempDir::new().unwrap();
        let project = tmp.path().join("my-project");
        std::fs::create_dir_all(&project).unwrap();

        let created = init_project_dir(&project).unwrap();
        assert!(created);

        // Check structure
        assert!(project.join(".openfang").is_dir());
        assert!(project.join(".openfang/.gitignore").is_file());
        assert!(project.join(".openfang/hands").is_dir());
        assert!(project.join(".openfang/agents").is_dir());

        // Check .gitignore content
        let content = std::fs::read_to_string(project.join(".openfang/.gitignore")).unwrap();
        assert!(content.contains("workspaces/"));
        assert!(content.contains("USER.md"));
    }

    #[test]
    fn test_init_project_dir_idempotent() {
        let tmp = TempDir::new().unwrap();
        let project = tmp.path().join("my-project");
        std::fs::create_dir_all(&project).unwrap();

        let first = init_project_dir(&project).unwrap();
        assert!(first);

        // Write custom content to .gitignore
        std::fs::write(
            project.join(".openfang/.gitignore"),
            "# custom\n",
        )
        .unwrap();

        let second = init_project_dir(&project).unwrap();
        assert!(!second); // Already existed

        // Custom content preserved
        let content = std::fs::read_to_string(project.join(".openfang/.gitignore")).unwrap();
        assert_eq!(content, "# custom\n");
    }

    #[test]
    fn test_resolve_project_dirs_cli_priority() {
        let cli = vec![std::path::PathBuf::from("/cli/project")];
        let config = vec![std::path::PathBuf::from("/config/project")];
        let resolved = resolve_project_dirs(&cli, &config);
        assert_eq!(resolved.len(), 2);
        assert_eq!(resolved[0], std::path::PathBuf::from("/cli/project"));
        assert_eq!(resolved[1], std::path::PathBuf::from("/config/project"));
    }

    #[test]
    fn test_resolve_project_dirs_dedup() {
        let cli = vec![std::path::PathBuf::from("/same/path")];
        let config = vec![std::path::PathBuf::from("/same/path")];
        let resolved = resolve_project_dirs(&cli, &config);
        assert_eq!(resolved.len(), 1);
    }
}
