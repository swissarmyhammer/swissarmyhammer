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

    // Always remove builtin agents from .agents/ store and agent dirs
    uninstall_builtin_agents(global)?;

    // Clean up lockfile entries for builtin skills and agents
    uninstall_lockfile_entries()?;

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

    // Collect link directories from detected agents
    let link_dirs: Vec<std::path::PathBuf> = agents
        .iter()
        .map(|agent| {
            if global {
                mirdan::agents::agent_global_skill_dir(&agent.def)
            } else {
                mirdan::agents::agent_project_skill_dir(&agent.def)
            }
        })
        .collect();

    let symlink_policies: Vec<_> = agents
        .iter()
        .map(|agent| agent.def.symlink_policy.clone())
        .collect();

    remove_store_entries(
        &store_dir,
        &builtin_names,
        &link_dirs,
        &symlink_policies,
        "skill",
    );

    Ok(())
}

/// Remove builtin agent symlinks from coding agent directories and clean up the .agents/ store.
///
/// Mirrors `uninstall_builtin_skills` but for the agent store.
fn uninstall_builtin_agents(global: bool) -> Result<(), String> {
    use swissarmyhammer_agents::AgentResolver;

    let store_dir = mirdan::store::agent_store_dir(global);

    let config = mirdan::agents::load_agents_config()
        .map_err(|e| format!("Failed to load agents config: {}", e))?;
    let agents = mirdan::agents::get_detected_agents(&config);

    let resolver = AgentResolver::new();
    let builtins = resolver.resolve_builtins();
    let builtin_names: Vec<String> = builtins.keys().cloned().collect();

    // Collect link directories from detected agents (filtering out agents without agent dirs)
    let mut link_dirs: Vec<std::path::PathBuf> = Vec::new();
    let mut symlink_policies: Vec<mirdan::agents::SymlinkPolicy> = Vec::new();

    for agent in &agents {
        let agent_dir = if global {
            mirdan::agents::agent_global_agent_dir(&agent.def)
        } else {
            mirdan::agents::agent_project_agent_dir(&agent.def)
        };
        if let Some(dir) = agent_dir {
            link_dirs.push(dir);
            symlink_policies.push(agent.def.symlink_policy.clone());
        }
    }

    remove_store_entries(
        &store_dir,
        &builtin_names,
        &link_dirs,
        &symlink_policies,
        "agent",
    );

    Ok(())
}

/// Remove named entries from a store directory and their symlinks from link directories.
///
/// This is the shared filesystem logic for both skill and agent uninstall.
/// Extracted for testability — the caller resolves names and directories,
/// this function does the filesystem work.
fn remove_store_entries(
    store_dir: &std::path::Path,
    names: &[String],
    link_dirs: &[std::path::PathBuf],
    symlink_policies: &[mirdan::agents::SymlinkPolicy],
    kind: &str,
) {
    for name in names {
        let store_path = store_dir.join(name);

        // Remove symlinks from each link directory
        for (dir, policy) in link_dirs.iter().zip(symlink_policies.iter()) {
            let link_name = mirdan::store::symlink_name(name, policy);
            let link_path = dir.join(&link_name);
            remove_if_symlink(&link_path);
        }

        // Remove entry from the store
        if store_path.exists() {
            if let Err(e) = fs::remove_dir_all(&store_path) {
                eprintln!(
                    "Warning: failed to remove store entry {}: {}",
                    store_path.display(),
                    e
                );
            } else {
                println!("Removed {} store: {}", kind, store_path.display());
            }
        }
    }

    // Remove the store directory if empty
    if store_dir.exists() {
        if let Ok(entries) = fs::read_dir(store_dir) {
            if entries.count() == 0 {
                let _ = fs::remove_dir(store_dir);
            }
        }
    }
}

/// Remove lockfile entries for all builtin skills and agents.
///
/// init writes entries to mirdan-lock.json; deinit must clean them up
/// so that the lockfile doesn't contain stale references.
fn uninstall_lockfile_entries() -> Result<(), String> {
    use swissarmyhammer_agents::AgentResolver;
    use swissarmyhammer_skills::SkillResolver;

    let project_root =
        std::env::current_dir().map_err(|e| format!("Failed to get current directory: {}", e))?;

    let lockfile_path = project_root.join("mirdan-lock.json");
    if !lockfile_path.exists() {
        return Ok(());
    }

    let mut lockfile = mirdan::lockfile::Lockfile::load(&project_root)
        .map_err(|e| format!("Failed to load lockfile: {}", e))?;

    let skill_resolver = SkillResolver::new();
    for name in skill_resolver.resolve_builtins().keys() {
        lockfile.remove_package(name);
    }

    let agent_resolver = AgentResolver::new();
    for name in agent_resolver.resolve_builtins().keys() {
        lockfile.remove_package(name);
    }

    lockfile
        .save(&project_root)
        .map_err(|e| format!("Failed to save lockfile: {}", e))?;

    println!("Lockfile entries cleaned up");
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

    // ── remove_store_entries tests ──────────────────────────────────────

    /// Set up a store + link directory structure for testing remove_store_entries.
    fn setup_store_structure(
        root: &Path,
        store_name: &str,
        link_dir_name: &str,
    ) -> (std::path::PathBuf, std::path::PathBuf) {
        let store_dir = root.join(store_name);
        let link_dir = root.join(link_dir_name);
        fs::create_dir_all(&store_dir).unwrap();
        fs::create_dir_all(&link_dir).unwrap();
        (store_dir, link_dir)
    }

    /// Create an entry in the store and symlink it into the link directory.
    fn create_store_entry_with_symlink(
        store_dir: &Path,
        link_dir: &Path,
        name: &str,
    ) -> (std::path::PathBuf, std::path::PathBuf) {
        let store_path = store_dir.join(name);
        fs::create_dir_all(&store_path).unwrap();
        fs::write(store_path.join("AGENT.md"), "# Test agent").unwrap();

        let link_path = link_dir.join(name);
        #[cfg(unix)]
        std::os::unix::fs::symlink(&store_path, &link_path).unwrap();

        (store_path, link_path)
    }

    #[test]
    fn test_remove_store_entries_removes_symlinks_and_store() {
        let tmp = TempDir::new().unwrap();
        let (store_dir, link_dir) = setup_store_structure(tmp.path(), ".agents", ".agents-links");

        let (store_path, link_path) =
            create_store_entry_with_symlink(&store_dir, &link_dir, "tester");

        assert!(store_path.exists());
        assert!(link_path.exists());

        let names = vec!["tester".to_string()];
        let link_dirs = vec![link_dir.clone()];
        let policies = vec![mirdan::agents::SymlinkPolicy::LastSegment];

        remove_store_entries(&store_dir, &names, &link_dirs, &policies, "agent");

        assert!(!link_path.exists(), "Symlink should be removed");
        assert!(!store_path.exists(), "Store entry should be removed");
        assert!(
            !store_dir.exists(),
            "Empty store directory should be removed"
        );
    }

    #[test]
    fn test_remove_store_entries_preserves_unrelated_store_entries() {
        let tmp = TempDir::new().unwrap();
        let (store_dir, link_dir) = setup_store_structure(tmp.path(), ".agents", ".agents-links");

        // Create two entries: one we'll remove, one we won't
        create_store_entry_with_symlink(&store_dir, &link_dir, "tester");
        let (unrelated_store, _unrelated_link) =
            create_store_entry_with_symlink(&store_dir, &link_dir, "custom-agent");

        // Only remove "tester"
        let names = vec!["tester".to_string()];
        let link_dirs = vec![link_dir.clone()];
        let policies = vec![mirdan::agents::SymlinkPolicy::LastSegment];

        remove_store_entries(&store_dir, &names, &link_dirs, &policies, "agent");

        assert!(
            unrelated_store.exists(),
            "Unrelated store entry must not be removed"
        );
        assert!(
            store_dir.exists(),
            "Store directory should remain when non-empty"
        );
    }

    #[test]
    fn test_remove_store_entries_handles_multiple_link_dirs() {
        let tmp = TempDir::new().unwrap();
        let store_dir = tmp.path().join(".agents");
        let link_dir_a = tmp.path().join("agent-a");
        let link_dir_b = tmp.path().join("agent-b");
        fs::create_dir_all(&store_dir).unwrap();
        fs::create_dir_all(&link_dir_a).unwrap();
        fs::create_dir_all(&link_dir_b).unwrap();

        // Create store entry and symlinks in both link dirs
        let store_path = store_dir.join("reviewer");
        fs::create_dir_all(&store_path).unwrap();
        fs::write(store_path.join("AGENT.md"), "# Reviewer").unwrap();

        let link_a = link_dir_a.join("reviewer");
        let link_b = link_dir_b.join("reviewer");
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&store_path, &link_a).unwrap();
            std::os::unix::fs::symlink(&store_path, &link_b).unwrap();
        }

        let names = vec!["reviewer".to_string()];
        let link_dirs = vec![link_dir_a.clone(), link_dir_b.clone()];
        let policies = vec![
            mirdan::agents::SymlinkPolicy::LastSegment,
            mirdan::agents::SymlinkPolicy::LastSegment,
        ];

        remove_store_entries(&store_dir, &names, &link_dirs, &policies, "agent");

        assert!(!link_a.exists(), "Symlink A should be removed");
        assert!(!link_b.exists(), "Symlink B should be removed");
        assert!(!store_path.exists(), "Store entry should be removed");
    }

    #[test]
    fn test_remove_store_entries_handles_missing_symlinks_gracefully() {
        let tmp = TempDir::new().unwrap();
        let (store_dir, link_dir) = setup_store_structure(tmp.path(), ".agents", ".agents-links");

        // Create store entry but NO symlink
        let store_path = store_dir.join("implementer");
        fs::create_dir_all(&store_path).unwrap();
        fs::write(store_path.join("AGENT.md"), "# Implementer").unwrap();

        let names = vec!["implementer".to_string()];
        let link_dirs = vec![link_dir.clone()];
        let policies = vec![mirdan::agents::SymlinkPolicy::LastSegment];

        // Should not panic — gracefully handles missing symlinks
        remove_store_entries(&store_dir, &names, &link_dirs, &policies, "agent");

        assert!(!store_path.exists(), "Store entry should still be removed");
    }

    #[test]
    fn test_remove_store_entries_preserves_real_dirs_in_link_dir() {
        let tmp = TempDir::new().unwrap();
        let (store_dir, link_dir) = setup_store_structure(tmp.path(), ".agents", ".agents-links");

        create_store_entry_with_symlink(&store_dir, &link_dir, "tester");

        // Also create a real directory in the link dir (not a symlink)
        let real_dir = link_dir.join("custom-real-agent");
        fs::create_dir_all(&real_dir).unwrap();
        fs::write(real_dir.join("AGENT.md"), "# Custom").unwrap();

        let names = vec!["tester".to_string()];
        let link_dirs = vec![link_dir.clone()];
        let policies = vec![mirdan::agents::SymlinkPolicy::LastSegment];

        remove_store_entries(&store_dir, &names, &link_dirs, &policies, "agent");

        assert!(
            real_dir.exists(),
            "Real directory in link dir must not be removed"
        );
        assert!(
            real_dir.join("AGENT.md").exists(),
            "Real directory contents must be intact"
        );
    }
}
