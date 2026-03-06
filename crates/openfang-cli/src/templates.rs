//! Discover and load agent templates from the agents directory.

use std::path::PathBuf;

/// A discovered agent template.
#[allow(dead_code)]
pub struct AgentTemplate {
    /// Template name (directory name).
    pub name: String,
    /// Description from the manifest.
    pub description: String,
    /// Raw TOML content.
    pub content: String,
    /// Source of this template (e.g. "bundled", "project:/path/to/repo").
    pub source: String,
    /// If from a project, the path to the project's `.openfang/agents/{name}/` directory.
    /// Used to copy seed files (SOUL.md, TOOLS.md, skills/) into the workspace.
    pub project_template_dir: Option<std::path::PathBuf>,
}

/// Discover template directories. Checks:
/// 1. The repo `agents/` dir (for dev builds)
/// 2. `~/.openfang/agents/` (installed templates)
/// 3. `OPENFANG_AGENTS_DIR` env var
pub fn discover_template_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    // Dev: repo agents/ directory (relative to the binary)
    if let Ok(exe) = std::env::current_exe() {
        // Walk up from the binary to find the workspace root
        let mut dir = exe.as_path();
        for _ in 0..5 {
            if let Some(parent) = dir.parent() {
                let agents = parent.join("agents");
                if agents.is_dir() {
                    dirs.push(agents);
                    break;
                }
                dir = parent;
            }
        }
    }

    // Installed templates (respects OPENFANG_HOME)
    let of_home = if let Ok(h) = std::env::var("OPENFANG_HOME") {
        PathBuf::from(h)
    } else if let Some(home) = dirs::home_dir() {
        home.join(".openfang")
    } else {
        std::env::temp_dir().join(".openfang")
    };
    {
        let agents = of_home.join("agents");
        if agents.is_dir() && !dirs.contains(&agents) {
            dirs.push(agents);
        }
    }

    // Environment override
    if let Ok(env_dir) = std::env::var("OPENFANG_AGENTS_DIR") {
        let p = PathBuf::from(env_dir);
        if p.is_dir() && !dirs.contains(&p) {
            dirs.push(p);
        }
    }

    dirs
}

/// Load all templates from discovered directories, falling back to bundled templates.
pub fn load_all_templates() -> Vec<AgentTemplate> {
    let mut templates = Vec::new();
    let mut seen_names = std::collections::HashSet::new();

    // First: load from filesystem (user-installed or dev repo)
    for dir in discover_template_dirs() {
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                let manifest = path.join("agent.toml");
                if !manifest.exists() {
                    continue;
                }
                let name = entry.file_name().to_string_lossy().to_string();
                if name == "custom" || !seen_names.insert(name.clone()) {
                    continue;
                }
                if let Ok(content) = std::fs::read_to_string(&manifest) {
                    let description = extract_description(&content);
                    templates.push(AgentTemplate {
                        name,
                        description,
                        content,
                        source: "filesystem".to_string(),
                        project_template_dir: None,
                    });
                }
            }
        }
    }

    // Fallback: load bundled templates for any not found on disk
    for (name, content) in crate::bundled_agents::bundled_agents() {
        if seen_names.insert(name.to_string()) {
            let description = extract_description(content);
            templates.push(AgentTemplate {
                name: name.to_string(),
                description,
                content: content.to_string(),
                source: "bundled".to_string(),
                project_template_dir: None,
            });
        }
    }

    templates.sort_by(|a, b| a.name.cmp(&b.name));
    templates
}

/// Extract the `description` field from raw TOML without full parsing.
fn extract_description(toml_str: &str) -> String {
    for line in toml_str.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("description") {
            if let Some(rest) = rest.trim_start().strip_prefix('=') {
                let val = rest.trim().trim_matches('"');
                return val.to_string();
            }
        }
    }
    String::new()
}

/// Load agent templates from project directories' `.openfang/agents/` folders.
/// Project templates are namespaced with `project:{dir_name}:` prefix.
/// Existing names in `seen` are skipped to avoid duplicates.
#[allow(dead_code)]
pub fn load_project_templates(
    project_dirs: &[std::path::PathBuf],
    seen: &mut std::collections::HashSet<String>,
) -> Vec<AgentTemplate> {
    let mut templates = Vec::new();
    for project_dir in project_dirs {
        let agents_dir = project_dir.join(".openfang").join("agents");
        if !agents_dir.is_dir() {
            continue;
        }
        let dir_name = project_dir
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string());

        let entries = match std::fs::read_dir(&agents_dir) {
            Ok(e) => e,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            // SECURITY: skip symlinks
            if path.read_link().is_ok() {
                continue;
            }
            let manifest = path.join("agent.toml");
            if !manifest.exists() {
                continue;
            }
            let base_name = entry.file_name().to_string_lossy().to_string();
            let namespaced = format!("project:{dir_name}:{base_name}");
            if !seen.insert(namespaced.clone()) {
                continue;
            }
            if let Ok(content) = std::fs::read_to_string(&manifest) {
                let description = extract_description(&content);
                templates.push(AgentTemplate {
                    name: namespaced,
                    description,
                    content,
                    source: format!("project:{}", project_dir.display()),
                    project_template_dir: Some(path),
                });
            }
        }
    }
    templates
}

/// Format a template description as a hint for cliclack select items.
pub fn template_display_hint(t: &AgentTemplate) -> String {
    if t.description.is_empty() {
        String::new()
    } else if t.description.chars().count() > 60 {
        let truncated: String = t.description.chars().take(57).collect();
        format!("{truncated}...")
    } else {
        t.description.clone()
    }
}
