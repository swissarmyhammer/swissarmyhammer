//! Remove sah from all detected AI coding agents (skills + MCP).

use std::fs;

use crate::cli::InstallTarget;

use super::settings;

/// Uninstall sah from all detected AI coding agents.
pub fn uninstall(target: InstallTarget, remove_directory: bool) -> Result<(), String> {
    let global = matches!(target, InstallTarget::User);

    // Remove MCP server from all detected agents
    uninstall_mcp_all_agents(global)?;

    // Handle Claude Code local-scope config specifically
    if matches!(target, InstallTarget::Local) {
        uninstall_claude_local_scope()?;
    }

    // Always remove builtin skills from .skills/ store and agent dirs
    // (init always installs them, so deinit should always remove them)
    uninstall_builtin_skills(global)?;

    // Remove directories if requested
    if remove_directory {
        let cwd = std::env::current_dir()
            .map_err(|e| format!("Failed to get current directory: {}", e))?;

        // Remove sah-specific directories
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
    }

    Ok(())
}

/// Remove sah MCP server from all detected agents using mirdan's mcp_config.
fn uninstall_mcp_all_agents(global: bool) -> Result<(), String> {
    let config = mirdan::agents::load_agents_config()
        .map_err(|e| format!("Failed to load agents config: {}", e))?;
    let agents = mirdan::agents::get_detected_agents(&config);

    let mut removed_count = 0;
    for agent in &agents {
        if let Some(mcp_def) = &agent.def.mcp_config {
            let config_path = if global {
                mirdan::agents::agent_global_mcp_config(&agent.def)
            } else {
                mirdan::agents::agent_project_mcp_config(&agent.def)
            };
            if let Some(config_path) = config_path {
                match mirdan::mcp_config::unregister_mcp_server(
                    &config_path,
                    &mcp_def.servers_key,
                    "sah",
                ) {
                    Ok(true) => {
                        println!(
                            "sah MCP server removed from {} ({})",
                            agent.def.name,
                            config_path.display()
                        );
                        removed_count += 1;
                    }
                    Ok(false) => {}
                    Err(e) => {
                        eprintln!(
                            "Warning: failed to remove MCP from {}: {}",
                            agent.def.name, e
                        );
                    }
                }
            }
        }
    }

    if removed_count == 0 {
        // Fallback: try legacy project-level removal
        uninstall_project_legacy()?;
    }

    Ok(())
}

/// Legacy project-level uninstall via .mcp.json (backward compat fallback).
fn uninstall_project_legacy() -> Result<(), String> {
    let path = settings::mcp_json_path();
    if !path.exists() {
        println!("No {} file found, nothing to uninstall", path.display());
        return Ok(());
    }

    let mut mcp_settings = settings::read_settings(&path)?;
    let changed = settings::remove_mcp_server(&mut mcp_settings);

    if let Some(mcp_servers) = mcp_settings.get("mcpServers").and_then(|m| m.as_object()) {
        if mcp_servers.is_empty() {
            mcp_settings.as_object_mut().unwrap().remove("mcpServers");
        }
    }

    if mcp_settings == serde_json::json!({}) {
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

/// Uninstall from local scope: `~/.claude.json` under `projects.<project-path>.mcpServers`.
fn uninstall_claude_local_scope() -> Result<(), String> {
    let path = settings::claude_json_path();
    let key = settings::project_key()?;

    if !path.exists() {
        println!("No {} file found, nothing to uninstall", path.display());
        return Ok(());
    }

    let mut root = settings::read_settings(&path)?;

    let changed = if let Some(projects) = root.get_mut("projects") {
        if let Some(entry) = projects.get_mut(&key) {
            let changed = settings::remove_mcp_server(entry);
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

/// Remove builtin skills from the .skills/ store and agent directories.
fn uninstall_builtin_skills(global: bool) -> Result<(), String> {
    use swissarmyhammer_skills::SkillResolver;

    let store_dir = mirdan::store::skill_store_dir(global);

    let config = mirdan::agents::load_agents_config()
        .map_err(|e| format!("Failed to load agents config: {}", e))?;
    let agents = mirdan::agents::get_detected_agents(&config);

    let resolver = SkillResolver::new();
    let builtins = resolver.resolve_builtins();
    let builtin_names: Vec<String> = builtins.keys().cloned().collect();

    for name in &builtin_names {
        let store_path = store_dir.join(name);

        // Remove symlinks from each agent's skill directory
        for agent in &agents {
            let link_name = mirdan::store::symlink_name(name, &agent.def.symlink_policy);
            let agent_skill_dir = if global {
                mirdan::agents::agent_global_skill_dir(&agent.def)
            } else {
                mirdan::agents::agent_project_skill_dir(&agent.def)
            };
            let link_path = agent_skill_dir.join(&link_name);

            if std::fs::symlink_metadata(&link_path).is_ok() {
                if let Err(e) = mirdan::store::remove_if_exists(&link_path) {
                    eprintln!("Warning: failed to remove {}: {}", link_path.display(), e);
                } else {
                    println!("Removed skill link: {}", link_path.display());
                }
            }
        }

        // Remove from store
        if store_path.exists() {
            if let Err(e) = fs::remove_dir_all(&store_path) {
                eprintln!(
                    "Warning: failed to remove store entry {}: {}",
                    store_path.display(),
                    e
                );
            } else {
                println!("Removed skill store: {}", store_path.display());
            }
        }
    }

    // Remove the .skills/ directory if empty
    if store_dir.exists() {
        if let Ok(entries) = fs::read_dir(&store_dir) {
            if entries.count() == 0 {
                let _ = fs::remove_dir(&store_dir);
            }
        }
    }

    Ok(())
}
