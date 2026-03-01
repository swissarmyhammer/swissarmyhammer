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

    // Remove Bash deny rule from Claude Code settings
    if matches!(target, InstallTarget::Project | InstallTarget::Local) {
        uninstall_deny_bash()?;
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

/// Remove "Bash" from permissions.deny in .claude/settings.json.
fn uninstall_deny_bash() -> Result<(), String> {
    let path = settings::claude_settings_path();
    if !path.exists() {
        return Ok(());
    }

    let mut claude_settings = settings::read_settings(&path)?;
    let changed = settings::remove_deny_bash(&mut claude_settings);

    if changed {
        settings::write_settings(&path, &claude_settings)?;
        println!("Bash tool deny rule removed from {}", path.display());
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

/// Remove builtin skill symlinks from agent directories and clean up the .skills/ store.
///
/// Only removes symlinks that point into the .skills/ store. Never removes
/// directories — agent skill directories like .github/copilot/skills/ are
/// left intact even if empty.
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

        // Remove only symlinks from each agent's skill directory
        for agent in &agents {
            let link_name = mirdan::store::symlink_name(name, &agent.def.symlink_policy);
            let agent_skill_dir = if global {
                mirdan::agents::agent_global_skill_dir(&agent.def)
            } else {
                mirdan::agents::agent_project_skill_dir(&agent.def)
            };
            let link_path = agent_skill_dir.join(&link_name);
            remove_if_symlink(&link_path);
        }

        // Remove skill from the .skills/ store
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

/// Remove a path only if it is a symlink. Returns true if removed.
///
/// This is the safety-critical function: it ensures deinit never deletes
/// real directories or files that weren't created by `sah init`.
fn remove_if_symlink(path: &std::path::Path) -> bool {
    match std::fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_symlink() => {
            if let Err(e) = std::fs::remove_file(path) {
                eprintln!("Warning: failed to remove {}: {}", path.display(), e);
                false
            } else {
                println!("Removed skill link: {}", path.display());
                true
            }
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use tempfile::TempDir;

    /// Set up a simulated agent skill directory structure like .github/copilot/skills/
    /// with a .skills/ store and symlinks pointing from agent dir to store.
    fn setup_skill_structure(
        root: &Path,
    ) -> (
        std::path::PathBuf, // store_dir (.skills/)
        std::path::PathBuf, // agent_skill_dir (.github/copilot/skills/)
    ) {
        let store_dir = root.join(".skills");
        let agent_skill_dir = root.join(".github").join("copilot").join("skills");
        fs::create_dir_all(&store_dir).unwrap();
        fs::create_dir_all(&agent_skill_dir).unwrap();
        (store_dir, agent_skill_dir)
    }

    /// Create a skill in the store and symlink it into the agent dir.
    fn create_skill_symlink(
        store_dir: &Path,
        agent_skill_dir: &Path,
        name: &str,
    ) -> (std::path::PathBuf, std::path::PathBuf) {
        let store_path = store_dir.join(name);
        fs::create_dir_all(&store_path).unwrap();
        fs::write(store_path.join("SKILL.md"), "# Test skill").unwrap();

        let link_path = agent_skill_dir.join(name);
        #[cfg(unix)]
        std::os::unix::fs::symlink(&store_path, &link_path).unwrap();

        (store_path, link_path)
    }

    #[test]
    fn test_remove_if_symlink_removes_symlink() {
        let tmp = TempDir::new().unwrap();
        let (store_dir, agent_dir) = setup_skill_structure(tmp.path());
        let (_store_path, link_path) = create_skill_symlink(&store_dir, &agent_dir, "commit");

        assert!(link_path.exists(), "Symlink should exist before removal");
        let removed = remove_if_symlink(&link_path);
        assert!(removed, "Should return true when removing a symlink");
        assert!(!link_path.exists(), "Symlink should be gone after removal");
    }

    #[test]
    fn test_remove_if_symlink_preserves_real_directory() {
        let tmp = TempDir::new().unwrap();
        let agent_dir = tmp.path().join(".github").join("copilot").join("skills");
        fs::create_dir_all(&agent_dir).unwrap();

        // Create a real directory (not a symlink) that looks like a skill
        let real_dir = agent_dir.join("commit");
        fs::create_dir_all(&real_dir).unwrap();
        fs::write(real_dir.join("SKILL.md"), "# Real skill").unwrap();

        let removed = remove_if_symlink(&real_dir);
        assert!(!removed, "Should return false for a real directory");
        assert!(real_dir.exists(), "Real directory must not be deleted");
        assert!(
            real_dir.join("SKILL.md").exists(),
            "Contents of real directory must be intact"
        );
    }

    #[test]
    fn test_remove_if_symlink_preserves_regular_file() {
        let tmp = TempDir::new().unwrap();
        let file_path = tmp.path().join("some_file.txt");
        fs::write(&file_path, "important data").unwrap();

        let removed = remove_if_symlink(&file_path);
        assert!(!removed, "Should return false for a regular file");
        assert!(file_path.exists(), "Regular file must not be deleted");
    }

    #[test]
    fn test_remove_if_symlink_nonexistent_path_is_noop() {
        let tmp = TempDir::new().unwrap();
        let nonexistent = tmp.path().join("does_not_exist");

        let removed = remove_if_symlink(&nonexistent);
        assert!(!removed, "Should return false for nonexistent path");
    }

    #[test]
    fn test_remove_if_symlink_preserves_agent_skill_directory() {
        let tmp = TempDir::new().unwrap();
        let (store_dir, agent_dir) = setup_skill_structure(tmp.path());

        // Create symlinks for two skills
        create_skill_symlink(&store_dir, &agent_dir, "commit");
        create_skill_symlink(&store_dir, &agent_dir, "plan");

        // Also put a non-sah file in the agent's parent (.github/workflows/)
        let workflows_dir = tmp.path().join(".github").join("workflows");
        fs::create_dir_all(&workflows_dir).unwrap();
        fs::write(workflows_dir.join("ci.yml"), "name: CI").unwrap();

        // Remove both symlinks
        remove_if_symlink(&agent_dir.join("commit"));
        remove_if_symlink(&agent_dir.join("plan"));

        // Agent skill directory must still exist (even though it's now empty)
        assert!(
            agent_dir.exists(),
            "Agent skill directory must not be deleted"
        );

        // .github/ and its other contents must be intact
        assert!(
            workflows_dir.join("ci.yml").exists(),
            "Non-sah files in .github/ must be preserved"
        );
    }

    #[test]
    fn test_symlink_removal_does_not_affect_store() {
        let tmp = TempDir::new().unwrap();
        let (store_dir, agent_dir) = setup_skill_structure(tmp.path());
        let (store_path, link_path) = create_skill_symlink(&store_dir, &agent_dir, "test");

        // Remove the symlink
        remove_if_symlink(&link_path);

        // Store entry should still exist — remove_if_symlink only removes the link
        assert!(
            store_path.exists(),
            "Store entry must not be affected by symlink removal"
        );
        assert!(
            store_path.join("SKILL.md").exists(),
            "Store contents must be intact"
        );
    }

    #[test]
    fn test_dangling_symlink_is_still_removed() {
        let tmp = TempDir::new().unwrap();
        let (store_dir, agent_dir) = setup_skill_structure(tmp.path());
        let (store_path, link_path) = create_skill_symlink(&store_dir, &agent_dir, "commit");

        // Delete the store entry so the symlink is dangling
        fs::remove_dir_all(&store_path).unwrap();
        assert!(!store_path.exists(), "Store entry should be gone");
        // The symlink itself still exists (as a dangling symlink)
        assert!(
            std::fs::symlink_metadata(&link_path).is_ok(),
            "Dangling symlink should still be detectable"
        );

        let removed = remove_if_symlink(&link_path);
        assert!(removed, "Should remove dangling symlinks too");
        assert!(
            std::fs::symlink_metadata(&link_path).is_err(),
            "Dangling symlink should be gone"
        );
    }
}
