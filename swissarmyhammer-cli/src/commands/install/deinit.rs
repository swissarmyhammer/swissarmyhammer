//! Remove sah from all detected AI coding agents (skills + MCP).
//!
//! Delegates to composable `Initializable` components registered in `super::components`.

use crate::cli::InstallTarget;
use swissarmyhammer_common::lifecycle::{InitRegistry, InitScope, InitStatus};

use super::components;

/// Uninstall sah from all detected AI coding agents.
///
/// Creates an `InitRegistry`, registers all components, and runs `deinit` in
/// reverse priority order. The `remove_directory` flag controls whether
/// `ProjectStructure` removes `.swissarmyhammer/` and `.prompts/`.
pub fn uninstall(target: InstallTarget, remove_directory: bool) -> Result<(), String> {
    let scope: InitScope = target.into();
    let global = matches!(target, InstallTarget::User);

    let mut registry = InitRegistry::new();
    components::register_all(&mut registry, global, remove_directory);

    let results = registry.run_all_deinit(&scope);

    // Display results and check for errors
    let mut has_errors = false;
    for r in &results {
        match r.status {
            InitStatus::Ok => {} // component already printed its messages
            InitStatus::Warning => eprintln!("Warning: {}", r.message),
            InitStatus::Error => {
                eprintln!("Error: {}", r.message);
                has_errors = true;
            }
            InitStatus::Skipped => {} // silent
        }
    }

    if has_errors {
        Err("Some components failed to deinitialize".to_string())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::components::{remove_if_symlink, remove_store_entries};
    use std::fs;
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
