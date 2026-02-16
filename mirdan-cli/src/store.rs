//! Central skill store and symlink management.
//!
//! Skills are stored once in `.skills/` (project) or `~/.skills/` (global),
//! then symlinked into each agent's skill directory. This avoids duplicating
//! files and works around agents that require flat subdirectories.

use std::path::{Path, PathBuf};

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

/// Compute the symlink name for a sanitized package path, given a policy.
///
/// - `LastSegment`: `"anthropics/skills/algorithmic-art"` → `"algorithmic-art"`
/// - `FullPath`: preserves the full sanitized path as-is
pub fn symlink_name(sanitized_name: &str, policy: &SymlinkPolicy) -> String {
    match policy {
        SymlinkPolicy::LastSegment => {
            sanitized_name
                .rsplit('/')
                .next()
                .unwrap_or(sanitized_name)
                .to_string()
        }
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
    let link_parent = link_path
        .parent()
        .unwrap_or_else(|| Path::new("."));

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
pub fn store_entry_still_referenced(
    store_path: &Path,
    agent_skill_dirs: &[PathBuf],
) -> bool {
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
        assert_eq!(
            sanitize_dir_name("http://example.com/foo/bar"),
            "foo/bar"
        );
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
    fn test_symlink_name_last_segment() {
        assert_eq!(
            symlink_name("anthropics/skills/algorithmic-art", &SymlinkPolicy::LastSegment),
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
            symlink_name("anthropics/skills/algorithmic-art", &SymlinkPolicy::FullPath),
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
        std::os::unix::fs::symlink(
            std::fs::canonicalize(&store).unwrap(),
            &link,
        )
        .unwrap();

        assert!(store_entry_still_referenced(&store, &[agent_dir.clone()]));

        // Remove the symlink
        std::fs::remove_file(&link).unwrap();
        assert!(!store_entry_still_referenced(&store, &[agent_dir]));
    }
}
