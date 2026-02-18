//! Remove sah MCP server configuration from Claude Code settings.

use std::fs;

use serde_json::json;

use crate::cli::InstallTarget;

use super::settings;

/// Uninstall sah MCP server from the specified target.
pub fn uninstall(target: InstallTarget, remove_directory: bool) -> Result<(), String> {
    match target {
        InstallTarget::Project => uninstall_project(),
        InstallTarget::Local => uninstall_local(),
        InstallTarget::User => uninstall_user(),
    }?;

    // Remove directories if requested
    if remove_directory {
        let cwd = std::env::current_dir()
            .map_err(|e| format!("Failed to get current directory: {}", e))?;

        let sah_dir = cwd.join(".swissarmyhammer");
        if sah_dir.exists() {
            fs::remove_dir_all(&sah_dir)
                .map_err(|e| format!("Failed to remove {}: {}", sah_dir.display(), e))?;
            println!("Removed {}", sah_dir.display());
        }

        let prompts_dir = cwd.join(".prompts");
        if prompts_dir.exists() {
            fs::remove_dir_all(&prompts_dir)
                .map_err(|e| format!("Failed to remove {}: {}", prompts_dir.display(), e))?;
            println!("Removed {}", prompts_dir.display());
        }

        // Remove installed skills from .claude/skills/
        remove_installed_skills(&cwd);
    }

    Ok(())
}

/// Uninstall from project-level `.mcp.json`.
fn uninstall_project() -> Result<(), String> {
    let path = settings::mcp_json_path();

    if !path.exists() {
        println!("No {} file found, nothing to uninstall", path.display());
        return Ok(());
    }

    let mut mcp_settings = settings::read_settings(&path)?;
    let changed = settings::remove_mcp_server(&mut mcp_settings);

    // If mcpServers is now empty, clean it up
    if let Some(mcp_servers) = mcp_settings.get("mcpServers").and_then(|m| m.as_object()) {
        if mcp_servers.is_empty() {
            mcp_settings.as_object_mut().unwrap().remove("mcpServers");
        }
    }

    // Delete file if empty, otherwise write back
    if mcp_settings == json!({}) {
        fs::remove_file(&path)
            .map_err(|e| format!("Failed to remove {}: {}", path.display(), e))?;
        println!(
            "sah MCP server uninstalled, removed empty {}",
            path.display()
        );
    } else if changed {
        settings::write_settings(&path, &mcp_settings)?;
        println!("sah MCP server uninstalled from {}", path.display());
    } else {
        println!("sah MCP server was not configured in {}", path.display());
    }

    Ok(())
}

/// Uninstall from user-level `~/.claude.json` top-level `mcpServers`.
fn uninstall_user() -> Result<(), String> {
    let path = settings::claude_json_path();

    if !path.exists() {
        println!("No {} file found, nothing to uninstall", path.display());
        return Ok(());
    }

    let mut root = settings::read_settings(&path)?;
    let changed = settings::remove_mcp_server(&mut root);

    // Clean up empty mcpServers object
    if let Some(mcp_servers) = root.get("mcpServers").and_then(|m| m.as_object()) {
        if mcp_servers.is_empty() {
            root.as_object_mut().unwrap().remove("mcpServers");
        }
    }

    if changed {
        settings::write_settings(&path, &root)?;
        println!(
            "sah MCP server uninstalled from {} (user scope)",
            path.display()
        );
    } else {
        println!(
            "sah MCP server was not configured in {} (user scope)",
            path.display()
        );
    }

    Ok(())
}

/// Uninstall from local scope: `~/.claude.json` under `projects.<project-path>.mcpServers`.
fn uninstall_local() -> Result<(), String> {
    let path = settings::claude_json_path();
    let key = settings::project_key()?;

    if !path.exists() {
        println!("No {} file found, nothing to uninstall", path.display());
        return Ok(());
    }

    let mut root = settings::read_settings(&path)?;

    // Navigate to the project entry
    let changed = if let Some(projects) = root.get_mut("projects") {
        if let Some(entry) = projects.get_mut(&key) {
            let changed = settings::remove_mcp_server(entry);
            // Clean up empty mcpServers in project entry
            if let Some(mcp_servers) = entry.get("mcpServers").and_then(|m| m.as_object()) {
                if mcp_servers.is_empty() {
                    entry.as_object_mut().unwrap().remove("mcpServers");
                }
            }
            changed
        } else {
            false
        }
    } else {
        false
    };

    if changed {
        settings::write_settings(&path, &root)?;
        println!(
            "sah MCP server uninstalled from {} (local scope, project: {})",
            path.display(),
            key
        );
    } else {
        println!(
            "sah MCP server was not configured in {} (local scope, project: {})",
            path.display(),
            key
        );
    }

    Ok(())
}

/// Remove installed skill files from .claude/skills/ for builtin skills
fn remove_installed_skills(project_root: &std::path::Path) {
    let skills_dir = project_root.join(".claude").join("skills");
    if !skills_dir.exists() {
        return;
    }

    // Only remove builtin skill directories (by checking known names)
    let builtin_names = ["plan", "do", "commit", "test", "implement"];

    for name in &builtin_names {
        let skill_dir = skills_dir.join(name);
        if skill_dir.exists() {
            if let Err(e) = fs::remove_dir_all(&skill_dir) {
                eprintln!("Warning: failed to remove {}: {}", skill_dir.display(), e);
            } else {
                println!("Removed skill: {}", skill_dir.display());
            }
        }
    }

    // Remove the .claude/skills/ directory if empty
    if let Ok(entries) = fs::read_dir(&skills_dir) {
        if entries.count() == 0 {
            let _ = fs::remove_dir(&skills_dir);
        }
    }
}
