//! Install sah MCP server configuration into Claude Code settings.

use crate::cli::InstallTarget;

use super::settings;

/// Install sah MCP server to the specified target.
pub fn install(target: InstallTarget) -> Result<(), String> {
    match target {
        InstallTarget::Project => install_project(),
        InstallTarget::Local => install_local(),
        InstallTarget::User => install_user(),
    }?;

    // For project/local installs, also create the .swissarmyhammer directory structure
    if matches!(target, InstallTarget::Project | InstallTarget::Local) {
        create_project_structure()?;
    }

    // Install skills for Claude Code
    install_skills(&target)?;

    Ok(())
}

/// Install to project-level `.mcp.json`.
fn install_project() -> Result<(), String> {
    let path = settings::mcp_json_path();

    let mut mcp_settings = settings::read_settings(&path)?;
    let changed = settings::merge_mcp_server(&mut mcp_settings);
    settings::write_settings(&path, &mcp_settings)?;

    if changed {
        println!("sah MCP server installed to {}", path.display());
    } else {
        println!("sah MCP server already configured in {}", path.display());
    }
    Ok(())
}

/// Install to user-level `~/.claude.json` top-level `mcpServers`.
fn install_user() -> Result<(), String> {
    let path = settings::claude_json_path();

    let mut root = settings::read_settings(&path)?;
    let changed = settings::merge_mcp_server(&mut root);
    settings::write_settings(&path, &root)?;

    if changed {
        println!(
            "sah MCP server installed to {} (user scope)",
            path.display()
        );
    } else {
        println!(
            "sah MCP server already configured in {} (user scope)",
            path.display()
        );
    }
    Ok(())
}

/// Install to local scope: `~/.claude.json` under `projects.<project-path>.mcpServers`.
fn install_local() -> Result<(), String> {
    let path = settings::claude_json_path();
    let key = settings::project_key()?;

    let mut root = settings::read_settings(&path)?;
    let entry = settings::ensure_project_entry(&mut root, &key);
    let changed = settings::merge_mcp_server(entry);
    settings::write_settings(&path, &root)?;

    if changed {
        println!(
            "sah MCP server installed to {} (local scope, project: {})",
            path.display(),
            key
        );
    } else {
        println!(
            "sah MCP server already configured in {} (local scope, project: {})",
            path.display(),
            key
        );
    }
    Ok(())
}

/// Create the project directory structure with .prompts, .swissarmyhammer, and workflows.
fn create_project_structure() -> Result<(), String> {
    use swissarmyhammer_common::SwissarmyhammerDirectory;

    let sah_dir = SwissarmyhammerDirectory::from_git_root()
        .or_else(|_| {
            let cwd = std::env::current_dir()
                .map_err(|e| format!("Failed to get current directory: {}", e))?;
            SwissarmyhammerDirectory::from_custom_root(cwd)
                .map_err(|e| format!("Failed to create .swissarmyhammer directory: {}", e))
        })
        .map_err(|e: String| e)?;

    // Create .prompts/ as a sibling to .swissarmyhammer/ (dot-directory path for PromptResolver)
    let project_root = sah_dir.root().parent().ok_or_else(|| {
        "Failed to determine project root from .swissarmyhammer directory".to_string()
    })?;
    let prompts_dir = project_root.join(".prompts");
    std::fs::create_dir_all(&prompts_dir)
        .map_err(|e| format!("Failed to create .prompts directory: {}", e))?;

    sah_dir
        .ensure_subdir("workflows")
        .map_err(|e| format!("Failed to create workflows directory: {}", e))?;

    println!(
        "Project structure initialized at {}",
        sah_dir.root().display()
    );

    Ok(())
}

/// Install builtin skills to the appropriate `.claude/skills/` directory.
///
/// For project/local installs: `.claude/skills/` in the project root.
/// For user installs: `~/.claude/skills/`.
/// Idempotent: overwrites with latest version.
fn install_skills(target: &InstallTarget) -> Result<(), String> {
    use swissarmyhammer_skills::SkillResolver;

    let resolver = SkillResolver::new();
    let skills = resolver.resolve_all();

    // Determine the target directory for skills
    let skills_dir = match target {
        InstallTarget::Project | InstallTarget::Local => {
            // Use the project root's .claude/skills/
            let cwd = std::env::current_dir()
                .map_err(|e| format!("Failed to get current directory: {}", e))?;

            // Try to find git root, fallback to cwd
            let root = swissarmyhammer_common::utils::find_git_repository_root().unwrap_or(cwd);
            root.join(".claude").join("skills")
        }
        InstallTarget::User => {
            // Use ~/.claude/skills/
            let home = dirs::home_dir()
                .ok_or_else(|| "Could not determine home directory".to_string())?;
            home.join(".claude").join("skills")
        }
    };

    // Install each builtin skill
    let mut installed_count = 0;
    for (name, skill) in &skills {
        // Only install builtin skills (don't copy local/user overrides back)
        if skill.source != swissarmyhammer_skills::SkillSource::Builtin {
            continue;
        }

        let skill_dir = skills_dir.join(name);
        std::fs::create_dir_all(&skill_dir)
            .map_err(|e| format!("Failed to create skill directory {}: {}", skill_dir.display(), e))?;

        // Write the SKILL.md with full frontmatter + body
        let skill_md_path = skill_dir.join("SKILL.md");
        let content = format_skill_md(skill);
        std::fs::write(&skill_md_path, &content)
            .map_err(|e| format!("Failed to write {}: {}", skill_md_path.display(), e))?;

        // Write any additional resource files
        for (filename, file_content) in &skill.resources.files {
            let file_path = skill_dir.join(filename);
            std::fs::write(&file_path, file_content)
                .map_err(|e| format!("Failed to write {}: {}", file_path.display(), e))?;
        }

        installed_count += 1;
    }

    if installed_count > 0 {
        println!(
            "Installed {} skills to {}",
            installed_count,
            skills_dir.display()
        );
    }

    Ok(())
}

/// Format a Skill back into SKILL.md content (frontmatter + body)
fn format_skill_md(skill: &swissarmyhammer_skills::Skill) -> String {
    let mut content = String::from("---\n");
    content.push_str(&format!("name: {}\n", skill.name));
    content.push_str(&format!("description: {}\n", skill.description));

    if !skill.allowed_tools.is_empty() {
        content.push_str(&format!("allowed-tools: {}\n", skill.allowed_tools.join(" ")));
    }

    if let Some(ref license) = skill.license {
        content.push_str(&format!("license: {}\n", license));
    }

    if !skill.metadata.is_empty() {
        content.push_str("metadata:\n");
        let mut keys: Vec<_> = skill.metadata.keys().collect();
        keys.sort();
        for key in keys {
            content.push_str(&format!("  {}: \"{}\"\n", key, skill.metadata[key]));
        }
    }

    content.push_str("---\n\n");
    content.push_str(&skill.instructions);
    content.push('\n');

    content
}
