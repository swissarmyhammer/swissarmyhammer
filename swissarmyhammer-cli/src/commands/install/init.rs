//! Set up sah for all detected AI coding agents (skills + MCP).

use crate::cli::InstallTarget;
use swissarmyhammer_config::TemplateContext;
use swissarmyhammer_prompts::PromptLibrary;

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
        command: "sah".to_string(),
        args: vec!["serve".to_string()],
        env: std::collections::BTreeMap::new(),
    };

    let mut installed_count = 0;
    for agent in &agents {
        if let Some(mcp_def) = &agent.def.mcp_config {
            let config_path = if global {
                mirdan::agents::agent_global_mcp_config(&agent.def)
            } else {
                mirdan::agents::agent_project_mcp_config(&agent.def)
            };
            if let Some(config_path) = config_path {
                match mirdan::mcp_config::register_mcp_server(
                    &config_path,
                    &mcp_def.servers_key,
                    "sah",
                    &entry,
                ) {
                    Ok(()) => {
                        println!(
                            "sah MCP server installed for {} ({})",
                            agent.def.name,
                            config_path.display()
                        );
                        installed_count += 1;
                    }
                    Err(e) => {
                        eprintln!(
                            "Warning: failed to install MCP for {}: {}",
                            agent.def.name, e
                        );
                    }
                }
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

/// Install builtin skills via mirdan's deploy + lockfile.
///
/// 1. Write each builtin to a temp dir (they're embedded in the binary)
/// 2. Call `deploy_skill_to_agents()` for each — handles store copy + symlinks
/// 3. Write lockfile entries via `Lockfile`
///
/// Skill instructions are rendered through the prompt library's Liquid template
/// engine before writing to disk, so `{% include %}` partials are expanded.
fn install_skills_via_mirdan(global: bool) -> Result<(), String> {
    use swissarmyhammer_skills::SkillResolver;

    let resolver = SkillResolver::new();
    let skills = resolver.resolve_all();

    // Build prompt library for rendering skill templates with partials
    let prompt_library = PromptLibrary::default();
    let template_context = TemplateContext::new();

    let project_root =
        std::env::current_dir().map_err(|e| format!("Failed to get current directory: {}", e))?;
    let mut lockfile = mirdan::lockfile::Lockfile::load(&project_root)
        .map_err(|e| format!("Failed to load lockfile: {}", e))?;

    let mut installed_count = 0;
    for (name, skill) in &skills {
        if skill.source != swissarmyhammer_skills::SkillSource::Builtin {
            continue;
        }

        // Write builtin to a temp dir so deploy_skill_to_agents can copy it
        let temp_dir =
            tempfile::tempdir().map_err(|e| format!("Failed to create temp dir: {}", e))?;
        let skill_dir = temp_dir.path().join(name);
        std::fs::create_dir_all(&skill_dir)
            .map_err(|e| format!("Failed to create temp skill dir: {}", e))?;

        // Render instructions through the prompt library's Liquid engine
        // so {% include %} partials are expanded before writing to disk
        let rendered_skill = render_skill_instructions(skill, &prompt_library, &template_context);

        // Write the SKILL.md from the rendered content
        let skill_md_path = skill_dir.join("SKILL.md");
        let content = format_skill_md(&rendered_skill);
        std::fs::write(&skill_md_path, &content)
            .map_err(|e| format!("Failed to write {}: {}", skill_md_path.display(), e))?;

        // Write any additional resource files
        for (filename, file_content) in &skill.resources.files {
            let file_path = skill_dir.join(filename);
            std::fs::write(&file_path, file_content)
                .map_err(|e| format!("Failed to write {}: {}", file_path.display(), e))?;
        }

        // Deploy via mirdan: store copy + agent symlinks
        let targets = mirdan::install::deploy_skill_to_agents(name, &skill_dir, None, global)
            .map_err(|e| format!("Failed to deploy skill '{}': {}", name, e))?;

        // Record in lockfile
        let version = skill
            .metadata
            .get("version")
            .cloned()
            .unwrap_or_else(|| "0.0.0".to_string());
        lockfile.add_package(
            name.clone(),
            mirdan::lockfile::LockedPackage {
                package_type: mirdan::package_type::PackageType::Skill,
                version,
                resolved: "builtin".to_string(),
                integrity: String::new(),
                installed_at: chrono::Utc::now().to_rfc3339(),
                targets,
            },
        );

        installed_count += 1;
    }

    if installed_count > 0 {
        lockfile
            .save(&project_root)
            .map_err(|e| format!("Failed to save lockfile: {}", e))?;
        println!(
            "Installed {} builtin skills (lockfile updated)",
            installed_count
        );
    }

    Ok(())
}

/// Render skill instructions through the prompt library's Liquid template engine.
///
/// This expands `{% include %}` partials so the installed SKILL.md contains
/// the full rendered content rather than raw Liquid tags.
fn render_skill_instructions(
    skill: &swissarmyhammer_skills::Skill,
    prompt_library: &PromptLibrary,
    template_context: &TemplateContext,
) -> swissarmyhammer_skills::Skill {
    let rendered_instructions =
        match prompt_library.render_text(&skill.instructions, template_context) {
            Ok(rendered) => rendered,
            Err(e) => {
                eprintln!(
                    "Warning: failed to render partials for skill '{}': {}",
                    skill.name, e
                );
                skill.instructions.clone()
            }
        };

    let mut rendered = skill.clone();
    rendered.instructions = rendered_instructions;
    rendered
}

/// Format a Skill back into SKILL.md content (frontmatter + body)
fn format_skill_md(skill: &swissarmyhammer_skills::Skill) -> String {
    let mut content = String::from("---\n");
    content.push_str(&format!("name: {}\n", skill.name));
    content.push_str(&format!("description: {}\n", skill.description));

    if !skill.allowed_tools.is_empty() {
        let tools = skill.allowed_tools.join(" ");
        content.push_str(&format!("allowed-tools: \"{}\"\n", tools));
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
