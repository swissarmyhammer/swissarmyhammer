//! Central skill store and symlink management.
//!
//! Skills are stored once in `.skills/` (project) or `~/.skills/` (global),
//! then symlinked into each agent's skill directory. This avoids duplicating
//! files and works around agents that require flat subdirectories.

use std::path::{Path, PathBuf};

use swissarmyhammer_common::reporter::{InitEvent, InitReporter};

use crate::agents::SymlinkPolicy;
use crate::registry::RegistryError;

/// Sanitize a package name for use as a filesystem directory name.
///
/// If the name is a URL (e.g. `https://github.com/anthropics/skills/algorithmic-art`),
/// strip the scheme and host to produce a path-safe name (e.g. `anthropics/skills/algorithmic-art`).
pub fn sanitize_dir_name(name: &str) -> String {
    if let Some(rest) = name.strip_prefix("https://") {
        if let Some((_host, path)) = rest.split_once('/') {
            return path.to_string();
        }
        return rest.to_string();
    }
    if let Some(rest) = name.strip_prefix("http://") {
        if let Some((_host, path)) = rest.split_once('/') {
            return path.to_string();
        }
        return rest.to_string();
    }
    name.to_string()
}

/// Return the central skill store directory.
///
/// - Project scope: `.skills/`
/// - Global scope: `~/.skills/`
pub fn skill_store_dir(global: bool) -> PathBuf {
    if global {
        dirs::home_dir()
            .expect("Could not find home directory")
            .join(".skills")
    } else {
        PathBuf::from(".skills")
    }
}

/// Return the central agent (subagent) store directory.
///
/// - Project scope: `.agents/`
/// - Global scope: `~/.agents/`
pub fn agent_store_dir(global: bool) -> PathBuf {
    if global {
        dirs::home_dir()
            .expect("Could not find home directory")
            .join(".agents")
    } else {
        PathBuf::from(".agents")
    }
}

/// Return the central tool store directory.
///
/// - Project scope: `.tools/`
/// - Global scope: `~/.tools/`
pub fn tool_store_dir(global: bool) -> PathBuf {
    if global {
        dirs::home_dir()
            .expect("Could not find home directory")
            .join(".tools")
    } else {
        PathBuf::from(".tools")
    }
}

/// Compute the symlink name for a sanitized package path, given a policy.
///
/// - `LastSegment`: `"anthropics/skills/algorithmic-art"` → `"algorithmic-art"`
/// - `FullPath`: preserves the full sanitized path as-is
pub fn symlink_name(sanitized_name: &str, policy: &SymlinkPolicy) -> String {
    match policy {
        SymlinkPolicy::LastSegment => sanitized_name
            .rsplit('/')
            .next()
            .unwrap_or(sanitized_name)
            .to_string(),
        SymlinkPolicy::FullPath => sanitized_name.to_string(),
    }
}

/// Create a relative symlink from `link_path` pointing to `store_path`.
///
/// On Unix, creates a symlink. On Windows, tries a junction first, then
/// falls back to copying the directory.
pub fn create_skill_link(store_path: &Path, link_path: &Path) -> Result<(), RegistryError> {
    // Ensure the link's parent directory exists
    if let Some(parent) = link_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Compute relative path from link_path's parent to store_path
    let link_parent = link_path.parent().unwrap_or_else(|| Path::new("."));

    let relative = pathdiff::diff_paths(store_path, link_parent).unwrap_or_else(|| {
        // Fallback: use absolute path
        store_path.to_path_buf()
    });

    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&relative, link_path).map_err(|e| {
            RegistryError::Io(std::io::Error::new(
                e.kind(),
                format!(
                    "Failed to create symlink {} -> {}: {}",
                    link_path.display(),
                    relative.display(),
                    e
                ),
            ))
        })?;
    }

    #[cfg(windows)]
    {
        // Try junction first (doesn't require elevated privileges)
        if let Err(_) = std::os::windows::fs::symlink_dir(&relative, link_path) {
            // Fallback: copy the directory
            copy_dir_for_fallback(store_path, link_path)?;
        }
    }

    Ok(())
}

/// Copy a directory recursively (fallback for systems without symlink support).
#[cfg(windows)]
fn copy_dir_for_fallback(src: &Path, dst: &Path) -> Result<(), RegistryError> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if src_path.is_dir() {
            copy_dir_for_fallback(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

/// Remove a path if it exists — handles files, symlinks, and directories.
pub fn remove_if_exists(path: &Path) -> Result<(), RegistryError> {
    // Check symlink_metadata to detect symlinks without following them
    match std::fs::symlink_metadata(path) {
        Ok(meta) => {
            if meta.is_dir() && !meta.file_type().is_symlink() {
                std::fs::remove_dir_all(path)?;
            } else {
                // symlink or file
                std::fs::remove_file(path)?;
            }
            Ok(())
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(RegistryError::Io(e)),
    }
}

/// Check whether any agent skill directory still has a symlink pointing to the
/// given store path. Used to decide if the store entry can be removed during
/// uninstall.
pub fn store_entry_still_referenced(store_path: &Path, agent_skill_dirs: &[PathBuf]) -> bool {
    let canonical_store = match std::fs::canonicalize(store_path) {
        Ok(p) => p,
        Err(_) => return false, // store path doesn't exist, so not referenced
    };

    for skill_dir in agent_skill_dirs {
        let entries = match std::fs::read_dir(skill_dir) {
            Ok(e) => e,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            // Check if it's a symlink
            if let Ok(meta) = std::fs::symlink_metadata(&path) {
                if meta.file_type().is_symlink() {
                    // Resolve the symlink target
                    if let Ok(target) = std::fs::canonicalize(&path) {
                        if target == canonical_store {
                            return true;
                        }
                    }
                }
            }
        }
    }

    false
}

/// Validate that a name is safe to use as a filesystem path component.
///
/// Rejects names containing path separators, parent-directory references,
/// or absolute paths to prevent path traversal attacks.
pub fn is_safe_name(name: &str) -> bool {
    !name.is_empty()
        && !name.contains('/')
        && !name.contains('\\')
        && !name.contains("..")
        && !std::path::Path::new(name).is_absolute()
}

/// Validate that a forward-slash-separated relative path is safe to join
/// under a target directory.
///
/// Accepts paths with `/` separators (e.g. `references/helper.md`) so skills
/// can deploy resources into subdirectories, but rejects parent-directory
/// references (`..`), backslashes, absolute paths, and empty segments — all of
/// which could escape the target directory via path traversal.
///
/// This is the multi-segment sibling of [`is_safe_name`]; use it at deploy
/// sites that genuinely need to preserve subdirectory structure, not as a
/// general replacement — callers that expect a single filename component
/// should still use [`is_safe_name`].
pub fn is_safe_relative_path(path: &str) -> bool {
    if path.is_empty() {
        return false;
    }
    if path.contains('\\') {
        return false;
    }
    if std::path::Path::new(path).is_absolute() {
        return false;
    }

    // Each path segment must be non-empty, must not be a parent-directory
    // reference, and must not contain the parent-directory sequence `..`.
    for segment in path.split('/') {
        if segment.is_empty() || segment == ".." || segment.contains("..") {
            return false;
        }
    }

    true
}

/// Remove named entries from a store directory and their symlinks from link directories.
///
/// This is the shared filesystem logic for both skill and agent uninstall.
/// The caller resolves names and directories, this function does the filesystem work.
/// Names are validated to prevent path traversal — any name containing `/`, `\`, or `..`
/// is skipped with a warning.
///
/// `link_dirs` and `symlink_policies` are zipped pairwise; both must be the same length.
/// After removing entries, the store directory itself is removed if it is empty.
pub fn remove_store_entries(
    store_dir: &Path,
    names: &[String],
    link_dirs: &[PathBuf],
    symlink_policies: &[SymlinkPolicy],
    kind: &str,
    reporter: &dyn InitReporter,
) {
    for name in names {
        if !is_safe_name(name) {
            reporter.emit(&InitEvent::Warning {
                message: format!("skipping unsafe {} name: {:?}", kind, name),
            });
            continue;
        }

        remove_single_store_entry(store_dir, name, link_dirs, symlink_policies, kind, reporter);
    }

    // Remove the store directory if empty
    if store_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(store_dir) {
            if entries.count() == 0 {
                let _ = std::fs::remove_dir(store_dir);
            }
        }
    }
}

/// Remove a single named entry from the store and its symlinks from link directories.
fn remove_single_store_entry(
    store_dir: &Path,
    name: &str,
    link_dirs: &[PathBuf],
    symlink_policies: &[SymlinkPolicy],
    kind: &str,
    reporter: &dyn InitReporter,
) {
    let store_path = store_dir.join(name);

    for (dir, policy) in link_dirs.iter().zip(symlink_policies.iter()) {
        let link_name = symlink_name(name, policy);
        let link_path = dir.join(&link_name);
        remove_if_symlink(&link_path, reporter);
    }

    if store_path.exists() {
        if let Err(e) = std::fs::remove_dir_all(&store_path) {
            reporter.emit(&InitEvent::Warning {
                message: format!(
                    "failed to remove store entry {}: {}",
                    store_path.display(),
                    e
                ),
            });
        } else {
            tracing::debug!("Removed {} store: {}", kind, store_path.display());
        }
    }
}

/// Remove a path only if it is a symlink. Returns true if removed.
///
/// This is the safety-critical function: it ensures deinit never deletes
/// real directories or files that weren't created by `sah init`.
pub fn remove_if_symlink(path: &Path, reporter: &dyn InitReporter) -> bool {
    match std::fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_symlink() => {
            if let Err(e) = std::fs::remove_file(path) {
                reporter.emit(&InitEvent::Warning {
                    message: format!("failed to remove {}: {}", path.display(), e),
                });
                false
            } else {
                tracing::debug!("Removed link: {}", path.display());
                true
            }
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_dir_name_url() {
        assert_eq!(
            sanitize_dir_name("https://github.com/anthropics/skills/algorithmic-art"),
            "anthropics/skills/algorithmic-art"
        );
    }

    #[test]
    fn test_sanitize_dir_name_http() {
        assert_eq!(sanitize_dir_name("http://example.com/foo/bar"), "foo/bar");
    }

    #[test]
    fn test_sanitize_dir_name_plain() {
        assert_eq!(sanitize_dir_name("no-secrets"), "no-secrets");
    }

    #[test]
    fn test_sanitize_dir_name_host_only() {
        assert_eq!(sanitize_dir_name("https://github.com"), "github.com");
    }

    #[test]
    fn test_skill_store_dir_project() {
        let dir = skill_store_dir(false);
        assert_eq!(dir, PathBuf::from(".skills"));
    }

    #[test]
    fn test_skill_store_dir_global() {
        let dir = skill_store_dir(true);
        assert!(dir.ends_with(".skills"));
        let home = dirs::home_dir().unwrap();
        assert!(dir.starts_with(home));
    }

    #[test]
    fn test_agent_store_dir_project() {
        let dir = agent_store_dir(false);
        assert_eq!(dir, PathBuf::from(".agents"));
    }

    #[test]
    fn test_agent_store_dir_global() {
        let dir = agent_store_dir(true);
        assert!(dir.ends_with(".agents"));
        let home = dirs::home_dir().unwrap();
        assert!(dir.starts_with(home));
    }

    #[test]
    fn test_tool_store_dir_project() {
        let dir = tool_store_dir(false);
        assert_eq!(dir, PathBuf::from(".tools"));
    }

    #[test]
    fn test_tool_store_dir_global() {
        let dir = tool_store_dir(true);
        assert!(dir.ends_with(".tools"));
        let home = dirs::home_dir().unwrap();
        assert!(dir.starts_with(home));
    }

    #[test]
    fn test_symlink_name_last_segment() {
        assert_eq!(
            symlink_name(
                "anthropics/skills/algorithmic-art",
                &SymlinkPolicy::LastSegment
            ),
            "algorithmic-art"
        );
    }

    #[test]
    fn test_symlink_name_last_segment_plain() {
        assert_eq!(
            symlink_name("no-secrets", &SymlinkPolicy::LastSegment),
            "no-secrets"
        );
    }

    #[test]
    fn test_symlink_name_full_path() {
        assert_eq!(
            symlink_name(
                "anthropics/skills/algorithmic-art",
                &SymlinkPolicy::FullPath
            ),
            "anthropics/skills/algorithmic-art"
        );
    }

    #[test]
    fn test_remove_if_exists_nonexistent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nonexistent");
        assert!(remove_if_exists(&path).is_ok());
    }

    #[test]
    fn test_remove_if_exists_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.txt");
        std::fs::write(&path, "hello").unwrap();
        assert!(path.exists());
        remove_if_exists(&path).unwrap();
        assert!(!path.exists());
    }

    #[test]
    fn test_remove_if_exists_directory() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("subdir");
        std::fs::create_dir(&path).unwrap();
        std::fs::write(path.join("file.txt"), "hello").unwrap();
        assert!(path.exists());
        remove_if_exists(&path).unwrap();
        assert!(!path.exists());
    }

    #[cfg(unix)]
    #[test]
    fn test_remove_if_exists_symlink() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("target");
        std::fs::create_dir(&target).unwrap();
        let link = dir.path().join("link");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        assert!(link.exists());
        remove_if_exists(&link).unwrap();
        assert!(!link.exists());
        // target should still exist
        assert!(target.exists());
    }

    #[cfg(unix)]
    #[test]
    fn test_create_skill_link() {
        let dir = tempfile::tempdir().unwrap();
        let store = dir.path().join(".skills/my-skill");
        std::fs::create_dir_all(&store).unwrap();
        std::fs::write(store.join("SKILL.md"), "# test").unwrap();

        let link = dir.path().join(".claude/skills/my-skill");
        create_skill_link(&store, &link).unwrap();

        assert!(link.exists());
        assert!(link.join("SKILL.md").exists());

        // Verify it's actually a symlink
        let meta = std::fs::symlink_metadata(&link).unwrap();
        assert!(meta.file_type().is_symlink());
    }

    #[cfg(unix)]
    #[test]
    fn test_create_skill_link_relative() {
        let dir = tempfile::tempdir().unwrap();
        let store = dir.path().join(".skills/my-skill");
        std::fs::create_dir_all(&store).unwrap();
        std::fs::write(store.join("SKILL.md"), "# test").unwrap();

        let link = dir.path().join(".claude/skills/my-skill");
        create_skill_link(&store, &link).unwrap();

        // Read the raw symlink target — it should be relative
        let target = std::fs::read_link(&link).unwrap();
        assert!(
            target.to_string_lossy().starts_with(".."),
            "symlink target should be relative: {}",
            target.display()
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_store_entry_still_referenced() {
        let dir = tempfile::tempdir().unwrap();
        let store = dir.path().join(".skills/my-skill");
        std::fs::create_dir_all(&store).unwrap();

        let agent_dir = dir.path().join(".claude/skills");
        std::fs::create_dir_all(&agent_dir).unwrap();
        let link = agent_dir.join("my-skill");
        std::os::unix::fs::symlink(std::fs::canonicalize(&store).unwrap(), &link).unwrap();

        assert!(store_entry_still_referenced(
            &store,
            std::slice::from_ref(&agent_dir)
        ));

        // Remove the symlink
        std::fs::remove_file(&link).unwrap();
        assert!(!store_entry_still_referenced(
            &store,
            std::slice::from_ref(&agent_dir)
        ));
    }

    /// Simulate deploy_skill_to_agents twice without deinit.
    ///
    /// This is the idempotency contract: running init again should overwrite
    /// the store and recreate symlinks, leaving the same result as a fresh install.
    #[cfg(unix)]
    #[test]
    fn test_deploy_idempotent_without_deinit() {
        let root = tempfile::tempdir().unwrap();
        let store_path = root.path().join(".skills/my-skill");
        let agent_dirs = vec![
            root.path().join(".claude/skills"),
            root.path().join(".github/copilot/skills"),
        ];

        // --- first deploy ---
        deploy_to_store_and_agents(
            &store_path,
            &agent_dirs,
            "my-skill",
            "# V1\nOriginal content",
        );

        for agent_dir in &agent_dirs {
            let link = agent_dir.join("my-skill");
            assert!(link.exists(), "symlink should exist after first deploy");
            let content = std::fs::read_to_string(link.join("SKILL.md")).unwrap();
            assert_eq!(content, "# V1\nOriginal content");
        }

        // --- second deploy (no deinit, updated content) ---
        deploy_to_store_and_agents(
            &store_path,
            &agent_dirs,
            "my-skill",
            "# V2\nUpdated content",
        );

        // Store should have new content
        let store_content = std::fs::read_to_string(store_path.join("SKILL.md")).unwrap();
        assert_eq!(store_content, "# V2\nUpdated content");

        // Every agent symlink should resolve to updated content
        for agent_dir in &agent_dirs {
            let link = agent_dir.join("my-skill");
            assert!(link.exists(), "symlink should survive re-deploy");

            let meta = std::fs::symlink_metadata(&link).unwrap();
            assert!(meta.file_type().is_symlink(), "should still be a symlink");

            let content = std::fs::read_to_string(link.join("SKILL.md")).unwrap();
            assert_eq!(
                content, "# V2\nUpdated content",
                "agent should see updated content through symlink"
            );
        }
    }

    /// Deploy → delete some agent symlinks → deploy again → verify recreated.
    ///
    /// This is the scenario where a user deletes .claude/skills/<skill> and
    /// expects `sah init` to recreate it without needing `sah deinit` first.
    #[cfg(unix)]
    #[test]
    fn test_deploy_recreates_deleted_agent_links() {
        let root = tempfile::tempdir().unwrap();
        let skills = vec!["commit", "plan", "review"];
        let agent_dirs = vec![
            root.path().join(".claude/skills"),
            root.path().join(".github/copilot/skills"),
        ];

        // --- first deploy: all skills to all agents ---
        for skill in &skills {
            let store_path = root.path().join(format!(".skills/{}", skill));
            deploy_to_store_and_agents(
                &store_path,
                &agent_dirs,
                skill,
                &format!("# {}\nContent", skill),
            );
        }

        // Verify all links exist
        for agent_dir in &agent_dirs {
            for skill in &skills {
                let link = agent_dir.join(skill);
                assert!(
                    link.exists(),
                    "{} should exist in {}",
                    skill,
                    agent_dir.display()
                );
            }
        }

        // --- delete some agent links (simulating user cleanup / breakage) ---
        // Delete "commit" from .claude/skills/ and "review" from both agents
        std::fs::remove_file(root.path().join(".claude/skills/commit")).unwrap();
        std::fs::remove_file(root.path().join(".claude/skills/review")).unwrap();
        std::fs::remove_file(root.path().join(".github/copilot/skills/review")).unwrap();

        // Verify they're gone
        assert!(!root.path().join(".claude/skills/commit").exists());
        assert!(!root.path().join(".claude/skills/review").exists());
        assert!(!root.path().join(".github/copilot/skills/review").exists());
        // "plan" should still be there
        assert!(root.path().join(".claude/skills/plan").exists());

        // --- second deploy (no deinit) ---
        for skill in &skills {
            let store_path = root.path().join(format!(".skills/{}", skill));
            deploy_to_store_and_agents(
                &store_path,
                &agent_dirs,
                skill,
                &format!("# {} v2\nUpdated", skill),
            );
        }

        // --- verify ALL links recreated with updated content ---
        for agent_dir in &agent_dirs {
            for skill in &skills {
                let link = agent_dir.join(skill);
                assert!(
                    link.exists(),
                    "{} should be recreated in {}",
                    skill,
                    agent_dir.display()
                );

                let meta = std::fs::symlink_metadata(&link).unwrap();
                assert!(
                    meta.file_type().is_symlink(),
                    "{} should be a symlink",
                    skill
                );

                let content = std::fs::read_to_string(link.join("SKILL.md")).unwrap();
                assert_eq!(
                    content,
                    format!("# {} v2\nUpdated", skill),
                    "{} should have updated content",
                    skill
                );
            }
        }
    }

    #[test]
    fn test_is_safe_name() {
        assert!(is_safe_name("my-skill"));
        assert!(is_safe_name("agent_v2"));
        assert!(!is_safe_name("../escape"));
        assert!(!is_safe_name("foo/bar"));
        assert!(!is_safe_name("foo\\bar"));
        assert!(!is_safe_name(""));
    }

    #[test]
    fn test_is_safe_relative_path() {
        // Accepted: single-segment and multi-segment forward-slash paths.
        assert!(is_safe_relative_path("helper.md"));
        assert!(is_safe_relative_path("references/helper.md"));
        assert!(is_safe_relative_path("references/foo.md"));
        assert!(is_safe_relative_path("a/b/c/d.md"));

        // Rejected: parent-directory traversal, absolute paths, backslashes,
        // and empty strings or empty segments.
        assert!(!is_safe_relative_path("../escape.md"));
        assert!(!is_safe_relative_path("references/../escape.md"));
        assert!(!is_safe_relative_path("/abs/path.md"));
        assert!(!is_safe_relative_path("foo\\bar.md"));
        assert!(!is_safe_relative_path(""));
        assert!(!is_safe_relative_path("references//helper.md"));
    }

    // ── remove_if_symlink tests ─────────────────────────────────────────

    use swissarmyhammer_common::reporter::NullReporter;

    /// Set up a simulated agent skill directory structure with a store dir
    /// and a links dir, with symlinks pointing from links to store.
    #[cfg(unix)]
    fn setup_skill_structure(root: &Path) -> (PathBuf, PathBuf) {
        let store_dir = root.join(".skills");
        let agent_skill_dir = root.join(".github").join("copilot").join("skills");
        std::fs::create_dir_all(&store_dir).unwrap();
        std::fs::create_dir_all(&agent_skill_dir).unwrap();
        (store_dir, agent_skill_dir)
    }

    /// Create a skill in the store and symlink it into the agent dir.
    #[cfg(unix)]
    fn create_skill_symlink(
        store_dir: &Path,
        agent_skill_dir: &Path,
        name: &str,
    ) -> (PathBuf, PathBuf) {
        let store_path = store_dir.join(name);
        std::fs::create_dir_all(&store_path).unwrap();
        std::fs::write(store_path.join("SKILL.md"), "# Test skill").unwrap();

        let link_path = agent_skill_dir.join(name);
        std::os::unix::fs::symlink(&store_path, &link_path).unwrap();

        (store_path, link_path)
    }

    #[cfg(unix)]
    #[test]
    fn test_remove_if_symlink_removes_symlink() {
        let tmp = tempfile::TempDir::new().unwrap();
        let (store_dir, agent_dir) = setup_skill_structure(tmp.path());
        let (_store_path, link_path) = create_skill_symlink(&store_dir, &agent_dir, "commit");

        assert!(link_path.exists(), "Symlink should exist before removal");
        let removed = remove_if_symlink(&link_path, &NullReporter);
        assert!(removed, "Should return true when removing a symlink");
        assert!(!link_path.exists(), "Symlink should be gone after removal");
    }

    #[cfg(unix)]
    #[test]
    fn test_remove_if_symlink_preserves_real_directory() {
        let tmp = tempfile::TempDir::new().unwrap();
        let agent_dir = tmp.path().join(".github").join("copilot").join("skills");
        std::fs::create_dir_all(&agent_dir).unwrap();

        // Create a real directory (not a symlink) that looks like a skill
        let real_dir = agent_dir.join("commit");
        std::fs::create_dir_all(&real_dir).unwrap();
        std::fs::write(real_dir.join("SKILL.md"), "# Real skill").unwrap();

        let removed = remove_if_symlink(&real_dir, &NullReporter);
        assert!(!removed, "Should return false for a real directory");
        assert!(real_dir.exists(), "Real directory must not be deleted");
        assert!(
            real_dir.join("SKILL.md").exists(),
            "Contents of real directory must be intact"
        );
    }

    #[test]
    fn test_remove_if_symlink_preserves_regular_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        let file_path = tmp.path().join("some_file.txt");
        std::fs::write(&file_path, "important data").unwrap();

        let removed = remove_if_symlink(&file_path, &NullReporter);
        assert!(!removed, "Should return false for a regular file");
        assert!(file_path.exists(), "Regular file must not be deleted");
    }

    #[test]
    fn test_remove_if_symlink_nonexistent_path_is_noop() {
        let tmp = tempfile::TempDir::new().unwrap();
        let nonexistent = tmp.path().join("does_not_exist");

        let removed = remove_if_symlink(&nonexistent, &NullReporter);
        assert!(!removed, "Should return false for nonexistent path");
    }

    #[cfg(unix)]
    #[test]
    fn test_remove_if_symlink_preserves_agent_skill_directory() {
        let tmp = tempfile::TempDir::new().unwrap();
        let (store_dir, agent_dir) = setup_skill_structure(tmp.path());

        // Create symlinks for two skills
        create_skill_symlink(&store_dir, &agent_dir, "commit");
        create_skill_symlink(&store_dir, &agent_dir, "plan");

        // Also put a non-sah file in the agent's parent (.github/workflows/)
        let workflows_dir = tmp.path().join(".github").join("workflows");
        std::fs::create_dir_all(&workflows_dir).unwrap();
        std::fs::write(workflows_dir.join("ci.yml"), "name: CI").unwrap();

        // Remove both symlinks
        remove_if_symlink(&agent_dir.join("commit"), &NullReporter);
        remove_if_symlink(&agent_dir.join("plan"), &NullReporter);

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

    #[cfg(unix)]
    #[test]
    fn test_remove_if_symlink_does_not_affect_store() {
        let tmp = tempfile::TempDir::new().unwrap();
        let (store_dir, agent_dir) = setup_skill_structure(tmp.path());
        let (store_path, link_path) = create_skill_symlink(&store_dir, &agent_dir, "test");

        // Remove the symlink
        remove_if_symlink(&link_path, &NullReporter);

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

    #[cfg(unix)]
    #[test]
    fn test_remove_if_symlink_dangling_symlink_is_still_removed() {
        let tmp = tempfile::TempDir::new().unwrap();
        let (store_dir, agent_dir) = setup_skill_structure(tmp.path());
        let (store_path, link_path) = create_skill_symlink(&store_dir, &agent_dir, "commit");

        // Delete the store entry so the symlink is dangling
        std::fs::remove_dir_all(&store_path).unwrap();
        assert!(!store_path.exists(), "Store entry should be gone");
        assert!(
            std::fs::symlink_metadata(&link_path).is_ok(),
            "Dangling symlink should still be detectable"
        );

        let removed = remove_if_symlink(&link_path, &NullReporter);
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
    ) -> (PathBuf, PathBuf) {
        let store_dir = root.join(store_name);
        let link_dir = root.join(link_dir_name);
        std::fs::create_dir_all(&store_dir).unwrap();
        std::fs::create_dir_all(&link_dir).unwrap();
        (store_dir, link_dir)
    }

    /// Create an entry in the store and symlink it into the link directory.
    #[cfg(unix)]
    fn create_store_entry_with_symlink(
        store_dir: &Path,
        link_dir: &Path,
        name: &str,
    ) -> (PathBuf, PathBuf) {
        let store_path = store_dir.join(name);
        std::fs::create_dir_all(&store_path).unwrap();
        std::fs::write(store_path.join("AGENT.md"), "# Test agent").unwrap();

        let link_path = link_dir.join(name);
        std::os::unix::fs::symlink(&store_path, &link_path).unwrap();

        (store_path, link_path)
    }

    #[cfg(unix)]
    #[test]
    fn test_remove_store_entries_removes_symlinks_and_store() {
        let tmp = tempfile::TempDir::new().unwrap();
        let (store_dir, link_dir) = setup_store_structure(tmp.path(), ".agents", ".agents-links");

        let (store_path, link_path) =
            create_store_entry_with_symlink(&store_dir, &link_dir, "tester");

        assert!(store_path.exists());
        assert!(link_path.exists());

        let names = vec!["tester".to_string()];
        let link_dirs = vec![link_dir.clone()];
        let policies = vec![SymlinkPolicy::LastSegment];

        remove_store_entries(
            &store_dir,
            &names,
            &link_dirs,
            &policies,
            "agent",
            &NullReporter,
        );

        assert!(!link_path.exists(), "Symlink should be removed");
        assert!(!store_path.exists(), "Store entry should be removed");
        assert!(
            !store_dir.exists(),
            "Empty store directory should be removed"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_remove_store_entries_preserves_unrelated_store_entries() {
        let tmp = tempfile::TempDir::new().unwrap();
        let (store_dir, link_dir) = setup_store_structure(tmp.path(), ".agents", ".agents-links");

        // Create two entries: one we'll remove, one we won't
        create_store_entry_with_symlink(&store_dir, &link_dir, "tester");
        let (unrelated_store, _unrelated_link) =
            create_store_entry_with_symlink(&store_dir, &link_dir, "custom-agent");

        // Only remove "tester"
        let names = vec!["tester".to_string()];
        let link_dirs = vec![link_dir.clone()];
        let policies = vec![SymlinkPolicy::LastSegment];

        remove_store_entries(
            &store_dir,
            &names,
            &link_dirs,
            &policies,
            "agent",
            &NullReporter,
        );

        assert!(
            unrelated_store.exists(),
            "Unrelated store entry must not be removed"
        );
        assert!(
            store_dir.exists(),
            "Store directory should remain when non-empty"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_remove_store_entries_handles_multiple_link_dirs() {
        let tmp = tempfile::TempDir::new().unwrap();
        let store_dir = tmp.path().join(".agents");
        let link_dir_a = tmp.path().join("agent-a");
        let link_dir_b = tmp.path().join("agent-b");
        std::fs::create_dir_all(&store_dir).unwrap();
        std::fs::create_dir_all(&link_dir_a).unwrap();
        std::fs::create_dir_all(&link_dir_b).unwrap();

        // Create store entry and symlinks in both link dirs
        let store_path = store_dir.join("reviewer");
        std::fs::create_dir_all(&store_path).unwrap();
        std::fs::write(store_path.join("AGENT.md"), "# Reviewer").unwrap();

        let link_a = link_dir_a.join("reviewer");
        let link_b = link_dir_b.join("reviewer");
        std::os::unix::fs::symlink(&store_path, &link_a).unwrap();
        std::os::unix::fs::symlink(&store_path, &link_b).unwrap();

        let names = vec!["reviewer".to_string()];
        let link_dirs = vec![link_dir_a.clone(), link_dir_b.clone()];
        let policies = vec![SymlinkPolicy::LastSegment, SymlinkPolicy::LastSegment];

        remove_store_entries(
            &store_dir,
            &names,
            &link_dirs,
            &policies,
            "agent",
            &NullReporter,
        );

        assert!(!link_a.exists(), "Symlink A should be removed");
        assert!(!link_b.exists(), "Symlink B should be removed");
        assert!(!store_path.exists(), "Store entry should be removed");
    }

    #[cfg(unix)]
    #[test]
    fn test_remove_store_entries_handles_missing_symlinks_gracefully() {
        let tmp = tempfile::TempDir::new().unwrap();
        let (store_dir, link_dir) = setup_store_structure(tmp.path(), ".agents", ".agents-links");

        // Create store entry but NO symlink
        let store_path = store_dir.join("implementer");
        std::fs::create_dir_all(&store_path).unwrap();
        std::fs::write(store_path.join("AGENT.md"), "# Implementer").unwrap();

        let names = vec!["implementer".to_string()];
        let link_dirs = vec![link_dir.clone()];
        let policies = vec![SymlinkPolicy::LastSegment];

        // Should not panic — gracefully handles missing symlinks
        remove_store_entries(
            &store_dir,
            &names,
            &link_dirs,
            &policies,
            "agent",
            &NullReporter,
        );

        assert!(!store_path.exists(), "Store entry should still be removed");
    }

    #[cfg(unix)]
    #[test]
    fn test_remove_store_entries_preserves_real_dirs_in_link_dir() {
        let tmp = tempfile::TempDir::new().unwrap();
        let (store_dir, link_dir) = setup_store_structure(tmp.path(), ".agents", ".agents-links");

        create_store_entry_with_symlink(&store_dir, &link_dir, "tester");

        // Also create a real directory in the link dir (not a symlink)
        let real_dir = link_dir.join("custom-real-agent");
        std::fs::create_dir_all(&real_dir).unwrap();
        std::fs::write(real_dir.join("AGENT.md"), "# Custom").unwrap();

        let names = vec!["tester".to_string()];
        let link_dirs = vec![link_dir.clone()];
        let policies = vec![SymlinkPolicy::LastSegment];

        remove_store_entries(
            &store_dir,
            &names,
            &link_dirs,
            &policies,
            "agent",
            &NullReporter,
        );

        assert!(
            real_dir.exists(),
            "Real directory in link dir must not be removed"
        );
        assert!(
            real_dir.join("AGENT.md").exists(),
            "Real directory contents must be intact"
        );
    }

    /// Reproduce the exact sequence from deploy_skill_to_agents:
    /// remove store → copy → remove each link → create each link.
    #[cfg(unix)]
    fn deploy_to_store_and_agents(
        store_path: &Path,
        agent_dirs: &[PathBuf],
        skill_name: &str,
        skill_content: &str,
    ) {
        // 1. Remove existing store entry + copy new content
        remove_if_exists(store_path).unwrap();
        std::fs::create_dir_all(store_path).unwrap();
        std::fs::write(store_path.join("SKILL.md"), skill_content).unwrap();

        // 2. Remove existing symlinks + create new ones
        for agent_dir in agent_dirs {
            let link_path = agent_dir.join(skill_name);
            remove_if_exists(&link_path).unwrap();
            create_skill_link(store_path, &link_path).unwrap();
        }
    }
}
