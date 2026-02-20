//! Set up sah for all detected AI coding agents (skills + MCP).

use crate::cli::InstallTarget;

use super::settings;

/// Install sah for all detected AI coding agents.
///
/// 1. Registers sah as an MCP server in all detected agent configs
/// 2. Creates the .swissarmyhammer/ project directory structure
/// 3. Installs builtin skills to the central .skills/ store and symlinks to all agents
pub fn install(target: InstallTarget) -> Result<(), String> {
    let global = matches!(target, InstallTarget::User);

    // Install MCP server config for all detected agents
    install_mcp_all_agents(global)?;

    // For project/local installs, handle Claude Code local-scope config specifically
    if matches!(target, InstallTarget::Local) {
        install_claude_local_scope()?;
    }

    // Create sah-specific project structure
    if matches!(target, InstallTarget::Project | InstallTarget::Local) {
        create_project_structure()?;
    }

    // Install builtin skills via mirdan store + sync
    install_skills_via_mirdan(global)?;

    Ok(())
}

/// Install sah MCP server to all detected agents using mirdan's mcp_config.
fn install_mcp_all_agents(global: bool) -> Result<(), String> {
    let config = mirdan::agents::load_agents_config()
        .map_err(|e| format!("Failed to load agents config: {}", e))?;
    let agents = mirdan::agents::get_detected_agents(&config);

    let entry = mirdan::mcp_config::McpServerEntry {
        name: "sah".to_string(),
        command: "sah".to_string(),
        args: vec!["serve".to_string()],
        env: None,
    };

    let mut installed_count = 0;
    for agent in &agents {
        match mirdan::mcp_config::install_mcp_for_agent(&agent.def, &entry, global) {
            Ok(Some(path)) => {
                println!(
                    "sah MCP server installed for {} ({})",
                    agent.def.name,
                    path.display()
                );
                installed_count += 1;
            }
            Ok(None) => {
                // Agent doesn't support MCP — skip silently
            }
            Err(e) => {
                eprintln!(
                    "Warning: failed to install MCP for {}: {}",
                    agent.def.name, e
                );
            }
        }
    }

    if installed_count == 0 {
        // Fallback to legacy settings.rs for backward compat
        install_project_legacy()?;
    }

    Ok(())
}

/// Legacy project-level install via .mcp.json (backward compat fallback).
fn install_project_legacy() -> Result<(), String> {
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

/// Install to local scope: `~/.claude.json` under `projects.<project-path>.mcpServers`.
/// This is a Claude Code-specific feature that mirdan doesn't handle generically.
fn install_claude_local_scope() -> Result<(), String> {
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

/// Install builtin skills via mirdan's central store + sync.
///
/// 1. Copy builtins to .skills/ store
/// 2. Call mirdan::sync::sync() to create symlinks to all detected agents
fn install_skills_via_mirdan(global: bool) -> Result<(), String> {
    use swissarmyhammer_skills::SkillResolver;

    let resolver = SkillResolver::new();
    let skills = resolver.resolve_all();

    let store_dir = mirdan::store::skill_store_dir(global);

    // Copy each builtin skill to the central store
    let mut installed_count = 0;
    for (name, skill) in &skills {
        if skill.source != swissarmyhammer_skills::SkillSource::Builtin {
            continue;
        }

        let skill_store_path = store_dir.join(name);
        std::fs::create_dir_all(&skill_store_path)
            .map_err(|e| format!("Failed to create skill store dir {}: {}", skill_store_path.display(), e))?;

        // Write the SKILL.md
        let skill_md_path = skill_store_path.join("SKILL.md");
        let content = format_skill_md(skill);
        std::fs::write(&skill_md_path, &content)
            .map_err(|e| format!("Failed to write {}: {}", skill_md_path.display(), e))?;

        // Write any additional resource files
        for (filename, file_content) in &skill.resources.files {
            let file_path = skill_store_path.join(filename);
            std::fs::write(&file_path, file_content)
                .map_err(|e| format!("Failed to write {}: {}", file_path.display(), e))?;
        }

        installed_count += 1;
    }

    if installed_count > 0 {
        println!(
            "Stored {} builtin skills in {}",
            installed_count,
            store_dir.display()
        );
    }

    // Run sync to create symlinks from store to all detected agent directories
    let project_root = std::env::current_dir()
        .map_err(|e| format!("Failed to get current directory: {}", e))?;
    let report = mirdan::sync::sync(&project_root, None, global)
        .map_err(|e| format!("Failed to sync skills: {}", e))?;

    if report.links_created > 0 {
        println!(
            "  Created {} symlinks across {} agent(s)",
            report.links_created,
            report.agents_synced.len()
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
