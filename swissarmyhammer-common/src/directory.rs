//! SwissArmyHammer directory structure management
//!
//! This module provides a centralized representation of the .swissarmyhammer directory
//! structure, supporting different root locations (Git root, user home, custom paths).
//!
//! This module re-exports types from `swissarmyhammer-directory` with the
//! `SwissarmyhammerConfig` configuration for backward compatibility.

// Re-export the shared directory types with SwissarmyhammerConfig bound
pub use swissarmyhammer_directory::{
    find_git_repository_root, DirectoryConfig, DirectoryRootType, ManagedDirectory,
    SwissarmyhammerConfig,
};

/// The directory name for SwissArmyHammer configuration and state.
/// This is the single source of truth for the directory name.
/// Crate-private to allow internal usage without exposing to external crates.
pub(crate) const DIR_NAME: &str = SwissarmyhammerConfig::DIR_NAME;

/// Type alias for backward compatibility.
///
/// `SwissarmyhammerDirectory` is now an alias for `ManagedDirectory<SwissarmyhammerConfig>`.
///
/// # Examples
///
/// ```no_run
/// use swissarmyhammer_common::SwissarmyhammerDirectory;
///
/// // Create from Git repository root
/// let sah_dir = SwissarmyhammerDirectory::from_git_root()?;
///
/// // Get a subdirectory, creating it if needed
/// let todo_dir = sah_dir.ensure_subdir("todo")?;
///
/// // Just get the path without creating
/// let rules_path = sah_dir.subdir("rules");
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub type SwissarmyhammerDirectory = ManagedDirectory<SwissarmyhammerConfig>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::CurrentDirGuard;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_from_custom_root() {
        let temp = TempDir::new().unwrap();
        let sah_dir =
            SwissarmyhammerDirectory::from_custom_root(temp.path().to_path_buf()).unwrap();

        assert!(sah_dir.root().exists());
        assert!(sah_dir.root().is_dir());
        assert_eq!(sah_dir.root(), temp.path().join(DIR_NAME));
        assert_eq!(
            *sah_dir.root_type(),
            DirectoryRootType::Custom(temp.path().to_path_buf())
        );
    }

    #[test]
    fn test_from_git_root_not_in_repo() {
        let temp = TempDir::new().unwrap();
        let _guard = CurrentDirGuard::new(temp.path()).unwrap();

        let result = SwissarmyhammerDirectory::from_git_root();

        assert!(result.is_err());
    }

    #[test]
    fn test_from_git_root_in_repo() {
        let temp = TempDir::new().unwrap();
        fs::create_dir_all(temp.path().join(".git")).unwrap();

        let _guard = CurrentDirGuard::new(temp.path()).unwrap();

        let sah_dir = SwissarmyhammerDirectory::from_git_root().unwrap();

        assert!(sah_dir.root().exists());
        assert_eq!(*sah_dir.root_type(), DirectoryRootType::GitRoot);
    }

    #[test]
    fn test_subdir() {
        let temp = TempDir::new().unwrap();
        let sah_dir =
            SwissarmyhammerDirectory::from_custom_root(temp.path().to_path_buf()).unwrap();

        let todo_path = sah_dir.subdir("todo");
        assert_eq!(todo_path, sah_dir.root().join("todo"));
        // Directory should NOT exist yet
        assert!(!todo_path.exists());
    }

    #[test]
    fn test_ensure_subdir() {
        let temp = TempDir::new().unwrap();
        let sah_dir =
            SwissarmyhammerDirectory::from_custom_root(temp.path().to_path_buf()).unwrap();

        let todo_dir = sah_dir.ensure_subdir("todo").unwrap();
        assert_eq!(todo_dir, sah_dir.root().join("todo"));
        // Directory SHOULD exist now
        assert!(todo_dir.exists());
        assert!(todo_dir.is_dir());
    }

    #[test]
    fn test_ensure_subdir_nested() {
        let temp = TempDir::new().unwrap();
        let sah_dir =
            SwissarmyhammerDirectory::from_custom_root(temp.path().to_path_buf()).unwrap();

        let nested_dir = sah_dir.ensure_subdir("cache/rules").unwrap();
        assert!(nested_dir.exists());
        assert!(nested_dir.is_dir());
        assert_eq!(nested_dir, sah_dir.root().join("cache/rules"));
    }

    #[test]
    fn test_contains_path() {
        let temp = TempDir::new().unwrap();
        let sah_dir =
            SwissarmyhammerDirectory::from_custom_root(temp.path().to_path_buf()).unwrap();

        // Create a file inside .swissarmyhammer
        let todo_dir = sah_dir.ensure_subdir("todo").unwrap();
        let todo_file = todo_dir.join("todo.yaml");
        fs::write(&todo_file, "test").unwrap();

        // Should detect file is inside .swissarmyhammer
        assert!(sah_dir.contains_path(&todo_file));

        // File outside .swissarmyhammer should return false
        let outside_file = temp.path().join("outside.txt");
        fs::write(&outside_file, "test").unwrap();
        assert!(!sah_dir.contains_path(&outside_file));
    }

    #[test]
    fn test_contains_path_relative() {
        let temp = TempDir::new().unwrap();
        let sah_dir =
            SwissarmyhammerDirectory::from_custom_root(temp.path().to_path_buf()).unwrap();

        // contains_path uses canonicalize which requires paths to exist
        // So we test with the actual root path
        assert!(sah_dir.contains_path(sah_dir.root()));
    }

    #[test]
    fn test_root_type_display() {
        assert_eq!(DirectoryRootType::UserHome.to_string(), "user home");
        assert_eq!(
            DirectoryRootType::GitRoot.to_string(),
            "git repository root"
        );

        let custom = DirectoryRootType::Custom(std::path::PathBuf::from("/tmp/test"));
        assert!(custom.to_string().contains("custom path"));
        assert!(custom.to_string().contains("/tmp/test"));
    }

    #[test]
    fn test_gitignore_created_on_init() {
        let temp = TempDir::new().unwrap();
        let sah_dir =
            SwissarmyhammerDirectory::from_custom_root(temp.path().to_path_buf()).unwrap();

        let gitignore_path = sah_dir.root().join(".gitignore");
        assert!(gitignore_path.exists(), ".gitignore should be created");

        let content = fs::read_to_string(&gitignore_path).unwrap();
        assert!(content.contains("tmp/"), ".gitignore should contain tmp/");
    }

    #[test]
    fn test_tmp_dir_created_on_init() {
        let temp = TempDir::new().unwrap();
        let sah_dir =
            SwissarmyhammerDirectory::from_custom_root(temp.path().to_path_buf()).unwrap();

        let tmp_path = sah_dir.subdir("tmp");
        assert!(tmp_path.exists(), "tmp/ should be created on init");
        assert!(tmp_path.is_dir(), "tmp/ should be a directory");
    }

    #[test]
    fn test_gitignore_not_overwritten() {
        let temp = TempDir::new().unwrap();

        // Create .swissarmyhammer directory manually with custom .gitignore
        let sah_path = temp.path().join(DIR_NAME);
        fs::create_dir_all(&sah_path).unwrap();

        let custom_content = "# Custom gitignore\n*.custom\n";
        let gitignore_path = sah_path.join(".gitignore");
        fs::write(&gitignore_path, custom_content).unwrap();

        // Now create SwissarmyhammerDirectory - should not overwrite
        let _sah_dir =
            SwissarmyhammerDirectory::from_custom_root(temp.path().to_path_buf()).unwrap();

        let content = fs::read_to_string(&gitignore_path).unwrap();
        assert_eq!(
            content, custom_content,
            ".gitignore should not be overwritten"
        );
    }

    #[test]
    fn test_gitignore_created_from_git_root() {
        let temp = TempDir::new().unwrap();
        fs::create_dir_all(temp.path().join(".git")).unwrap();

        let sah_dir = {
            let _guard = CurrentDirGuard::new(temp.path()).unwrap();
            SwissarmyhammerDirectory::from_git_root().unwrap()
        };
        // Guard dropped here, original directory restored

        let gitignore_path = sah_dir.root().join(".gitignore");

        assert!(
            gitignore_path.exists(),
            ".gitignore should be created in Git root at {}",
            gitignore_path.display()
        );

        let content = fs::read_to_string(&gitignore_path).unwrap();
        assert!(content.contains("tmp/"));
    }
}
