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
