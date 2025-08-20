//! Directory traversal utilities for SwissArmyHammer
//!
//! This module provides reusable directory traversal functionality to avoid
//! code duplication across the codebase.

use crate::security::MAX_DIRECTORY_DEPTH;
use std::env;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Find all `.swissarmyhammer` directories by walking up from the current directory
///
/// This function walks up the directory tree from the given starting path,
/// looking for `.swissarmyhammer` directories. It respects the MAX_DIRECTORY_DEPTH
/// limit and optionally excludes the user's home directory.
///
/// # Arguments
///
/// * `start_path` - The path to start searching from
/// * `exclude_home` - Whether to exclude the home ~/.swissarmyhammer directory
///
/// # Returns
///
/// A vector of paths to `.swissarmyhammer` directories, ordered from root to current
pub fn find_swissarmyhammer_dirs_upward(start_path: &Path, exclude_home: bool) -> Vec<PathBuf> {
    let mut directories = Vec::new();
    let mut path = start_path;
    let mut depth = 0;

    // Get home directory for exclusion check
    // Use HOME environment variable first (for test isolation), then fall back to dirs::home_dir()
    let home_swissarmyhammer = std::env::var("HOME")
        .map(|home_str| PathBuf::from(home_str).join(".swissarmyhammer"))
        .or_else(|_| {
            dirs::home_dir()
                .map(|home| home.join(".swissarmyhammer"))
                .ok_or(())
        })
        .ok();

    loop {
        if depth >= MAX_DIRECTORY_DEPTH {
            break;
        }

        let swissarmyhammer_dir = path.join(".swissarmyhammer");
        if swissarmyhammer_dir.exists() && swissarmyhammer_dir.is_dir() {
            // Check if we should exclude home directory
            if exclude_home {
                if let Some(ref home_dir) = home_swissarmyhammer {
                    if &swissarmyhammer_dir == home_dir {
                        // Skip home directory but continue searching
                        match path.parent() {
                            Some(parent) => {
                                path = parent;
                                depth += 1;
                                continue;
                            }
                            None => break,
                        }
                    }
                }
            }

            directories.push(swissarmyhammer_dir);
        }

        match path.parent() {
            Some(parent) => {
                path = parent;
                depth += 1;
            }
            None => break,
        }
    }

    // Reverse to get root-to-current order
    directories.reverse();
    directories
}

/// Walk a directory recursively to find files with specific extensions
///
/// This function uses WalkDir to recursively find all files with the given
/// extensions in a directory.
///
/// # Arguments
///
/// * `dir` - The directory to walk
/// * `extensions` - The file extensions to look for (without dots)
///
/// # Returns
///
/// An iterator over the found file paths
pub fn walk_files_with_extensions<'a>(
    dir: &Path,
    extensions: &'a [&'a str],
) -> impl Iterator<Item = PathBuf> + 'a {
    let dir = dir.to_owned();
    WalkDir::new(dir).into_iter().filter_map(move |entry| {
        entry.ok().and_then(|e| {
            let path = e.path();
            if path.is_file() {
                if let Some(filename) = path.file_name().and_then(|s| s.to_str()) {
                    // Check for compound extensions first
                    let compound_extensions = [".md.liquid", ".markdown.liquid", ".liquid.md"];

                    for compound_ext in &compound_extensions {
                        if filename.ends_with(compound_ext) {
                            // Check if any part of the compound extension matches our filter
                            let parts: Vec<&str> =
                                compound_ext.trim_start_matches('.').split('.').collect();
                            if parts.iter().any(|part| extensions.contains(part)) {
                                return Some(path.to_path_buf());
                            }
                        }
                    }

                    // Fallback to single extension check
                    if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                        if extensions.contains(&ext) {
                            return Some(path.to_path_buf());
                        }
                    }
                }
            }
            None
        })
    })
}

/// Find the repository root or return the current directory
///
/// This function walks up the directory tree looking for a `.git` directory
/// to identify a Git repository. If found, returns the repository root.
/// If no Git repository is found, returns the current directory.
///
/// # Returns
///
/// * `Result<PathBuf, std::io::Error>` - The repository root path or current directory
///
/// # Errors
///
/// Returns an error if the current directory cannot be determined.
pub fn find_repository_or_current_directory() -> Result<PathBuf, std::io::Error> {
    let current_dir = env::current_dir()?;
    let mut path = current_dir.as_path();
    let mut depth = 0;

    // Walk up looking for .git directory
    loop {
        if depth >= MAX_DIRECTORY_DEPTH {
            break;
        }

        if path.join(".git").exists() {
            return Ok(path.to_path_buf());
        }

        match path.parent() {
            Some(parent) => {
                path = parent;
                depth += 1;
            }
            None => break,
        }
    }

    // No git repository found, return current directory
    Ok(current_dir)
}

/// Find the Git repository root starting from current directory
///
/// Walks up the directory tree looking for .git directory to identify
/// a Git repository. This function respects MAX_DIRECTORY_DEPTH to prevent
/// infinite traversal and returns None if no Git repository is found.
///
/// # Returns
///
/// * `Option<PathBuf>` - Some(path) if Git repository found, None otherwise
pub fn find_git_repository_root() -> Option<PathBuf> {
    let current_dir = std::env::current_dir().ok()?;
    find_git_repository_root_from(&current_dir)
}

/// Find the Git repository root starting from a specific directory
///
/// Walks up the directory tree looking for .git directory to identify
/// a Git repository. This function respects MAX_DIRECTORY_DEPTH to prevent
/// infinite traversal and returns None if no Git repository is found.
///
/// # Arguments
///
/// * `start_dir` - The directory to start searching from
///
/// # Returns
///
/// * `Option<PathBuf>` - Some(path) if Git repository found, None otherwise
fn find_git_repository_root_from(start_dir: &Path) -> Option<PathBuf> {
    let mut path = start_dir;
    let mut depth = 0;

    loop {
        if depth >= MAX_DIRECTORY_DEPTH {
            break;
        }

        if path.join(".git").exists() {
            return Some(path.to_path_buf());
        }

        match path.parent() {
            Some(parent) => {
                path = parent;
                depth += 1;
            }
            None => break,
        }
    }

    None
}

/// Find the SwissArmyHammer directory for the current Git repository
///
/// Returns None if not in a Git repository or if no .swissarmyhammer directory exists.
/// This function enforces the new Git-centric directory resolution approach where
/// .swissarmyhammer directories should only exist at Git repository roots.
///
/// # Returns
///
/// * `Option<PathBuf>` - Some(path) if Git repository found with .swissarmyhammer directory, None otherwise
pub fn find_swissarmyhammer_directory() -> Option<PathBuf> {
    let git_root = find_git_repository_root()?;
    let swissarmyhammer_dir = git_root.join(".swissarmyhammer");

    if swissarmyhammer_dir.exists() && swissarmyhammer_dir.is_dir() {
        Some(swissarmyhammer_dir)
    } else {
        None
    }
}

/// Get or create the SwissArmyHammer directory for the current Git repository
///
/// Returns error if not in a Git repository. This function enforces the new
/// Git-centric directory resolution approach where .swissarmyhammer directories
/// should only exist at Git repository roots.
///
/// # Returns
///
/// * `Result<PathBuf, SwissArmyHammerError>` - Path to .swissarmyhammer directory or error
///
/// # Errors
///
/// * `NotInGitRepository` - If not currently in a Git repository
/// * `DirectoryCreation` - If .swissarmyhammer directory cannot be created
pub fn get_or_create_swissarmyhammer_directory() -> crate::error::Result<PathBuf> {
    use crate::error::SwissArmyHammerError;

    let git_root = find_git_repository_root().ok_or(SwissArmyHammerError::NotInGitRepository)?;

    let swissarmyhammer_dir = git_root.join(".swissarmyhammer");

    if swissarmyhammer_dir.exists() {
        if !swissarmyhammer_dir.is_dir() {
            return Err(SwissArmyHammerError::directory_creation(
                std::io::Error::new(
                    std::io::ErrorKind::AlreadyExists,
                    format!(
                        "{} exists but is not a directory",
                        swissarmyhammer_dir.display()
                    ),
                ),
            ));
        }
    } else {
        std::fs::create_dir_all(&swissarmyhammer_dir)
            .map_err(SwissArmyHammerError::directory_creation)?;
    }

    Ok(swissarmyhammer_dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_find_swissarmyhammer_dirs_upward() {
        let temp_dir = TempDir::new().unwrap();
        let base = temp_dir.path();

        // Create nested structure
        let level1 = base.join("level1");
        let level2 = level1.join("level2");
        let level3 = level2.join("level3");

        fs::create_dir_all(&level3).unwrap();

        // Create .swissarmyhammer dirs at different levels
        fs::create_dir(base.join(".swissarmyhammer")).unwrap();
        fs::create_dir(level2.join(".swissarmyhammer")).unwrap();

        // Search from level3
        let dirs = find_swissarmyhammer_dirs_upward(&level3, false);

        assert_eq!(dirs.len(), 2);
        assert_eq!(dirs[0], base.join(".swissarmyhammer"));
        assert_eq!(dirs[1], level2.join(".swissarmyhammer"));
    }

    #[test]
    fn test_walk_files_with_extensions() {
        let temp_dir = TempDir::new().unwrap();
        let base = temp_dir.path();

        // Create some test files
        fs::write(base.join("test.md"), "content").unwrap();
        fs::write(base.join("test.txt"), "content").unwrap();

        let subdir = base.join("subdir");
        fs::create_dir(&subdir).unwrap();
        fs::write(subdir.join("nested.md"), "content").unwrap();
        fs::write(subdir.join("nested.mermaid"), "content").unwrap();

        // Find markdown and mermaid files
        let files: Vec<_> = walk_files_with_extensions(base, &["md", "mermaid"]).collect();

        assert_eq!(files.len(), 3);
        assert!(files.iter().any(|p| p.ends_with("test.md")));
        assert!(files.iter().any(|p| p.ends_with("nested.md")));
        assert!(files.iter().any(|p| p.ends_with("nested.mermaid")));
        assert!(!files.iter().any(|p| p.ends_with("test.txt")));
    }

    #[test]
    #[serial]
    fn test_find_git_repository_root_found_at_current() {
        let temp_dir = TempDir::new().unwrap();
        let base = temp_dir.path();

        // Create .git directory at the base
        fs::create_dir(base.join(".git")).unwrap();

        // Change to this directory and test
        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(base).expect("Failed to change directory");

        let result = find_git_repository_root();

        // Restore original directory
        let _ = env::set_current_dir(original_dir);

        assert!(result.is_some());
        assert_eq!(
            result.unwrap().canonicalize().unwrap(),
            base.canonicalize().unwrap()
        );
    }

    #[test]
    fn test_find_git_repository_root_found_parent() {
        let temp_dir = TempDir::new().unwrap();
        let base = temp_dir.path();

        // Create nested structure with .git at parent level
        let level1 = base.join("level1");
        let level2 = level1.join("level2");
        fs::create_dir_all(&level2).unwrap();
        fs::create_dir(base.join(".git")).unwrap();

        // Test from level2 directory
        let result = find_git_repository_root_from(&level2);

        assert!(result.is_some());
        assert_eq!(
            result.unwrap().canonicalize().unwrap(),
            base.canonicalize().unwrap()
        );
    }

    #[test]
    #[serial]
    fn test_find_git_repository_root_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let base = temp_dir.path();

        // Create directory structure without .git
        let level1 = base.join("level1");
        fs::create_dir(&level1).unwrap();

        // Change to this directory and test
        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(&level1).expect("Failed to change to test directory");

        let result = find_git_repository_root();

        // Restore original directory
        let _ = env::set_current_dir(original_dir);

        assert!(result.is_none());
    }

    #[test]
    fn test_find_git_repository_root_depth_limit() {
        let temp_dir = TempDir::new().unwrap();
        let base = temp_dir.path();

        // Create very deep directory structure
        let mut current = base.to_path_buf();
        for i in 0..=MAX_DIRECTORY_DEPTH + 1 {
            current = current.join(format!("level{}", i));
            fs::create_dir_all(&current).unwrap();
        }

        // Put .git at the base (beyond MAX_DIRECTORY_DEPTH from deepest)
        fs::create_dir(base.join(".git")).unwrap();

        // Test from deepest directory
        let result = find_git_repository_root_from(&current);

        // Should return None due to depth limit
        assert!(result.is_none());
    }

    #[test]
    fn test_find_git_repository_root_within_depth_limit() {
        let temp_dir = TempDir::new().unwrap();
        let base = temp_dir.path();

        // Create directory structure well within MAX_DIRECTORY_DEPTH (5 levels)
        let mut current = base.to_path_buf();
        for i in 0..5 {
            current = current.join(format!("level{}", i));
            fs::create_dir_all(&current).unwrap();
        }

        // Put .git at the base
        fs::create_dir(base.join(".git")).unwrap();

        // Test from 5 levels deep - should find .git at base
        let result = find_git_repository_root_from(&current);

        // Should find the .git directory well within depth limit
        assert!(result.is_some());
        assert_eq!(
            result.unwrap().canonicalize().unwrap(),
            base.canonicalize().unwrap()
        );
    }

    #[test]
    #[serial]
    fn test_find_git_repository_root_git_file_not_directory() {
        let temp_dir = TempDir::new().unwrap();
        let base = temp_dir.path();

        // Create .git as a file instead of directory (as in git worktree)
        fs::write(
            base.join(".git"),
            "gitdir: /some/other/path/.git/worktrees/test",
        )
        .unwrap();

        // Change to this directory and test
        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(base).expect("Failed to change to test directory");

        let result = find_git_repository_root();

        // Restore original directory
        let _ = env::set_current_dir(original_dir);

        // Should still find the repository root (git worktree or submodule case)
        assert!(result.is_some());
        assert_eq!(
            result.unwrap().canonicalize().unwrap(),
            base.canonicalize().unwrap()
        );
    }

    #[test]
    fn test_find_git_repository_root_multiple_git_dirs() {
        let temp_dir = TempDir::new().unwrap();
        let base = temp_dir.path();

        // Create nested structure with .git at multiple levels
        let level1 = base.join("level1");
        let level2 = level1.join("level2");
        fs::create_dir_all(&level2).unwrap();

        // Create .git directories at multiple levels
        fs::create_dir(base.join(".git")).unwrap();
        fs::create_dir(level1.join(".git")).unwrap();

        // Test from level2 directory
        let result = find_git_repository_root_from(&level2);

        // Should find the nearest .git directory (level1, not base)
        assert!(result.is_some());
        assert_eq!(
            result.unwrap().canonicalize().unwrap(),
            level1.canonicalize().unwrap()
        );
    }

    #[test]
    fn test_find_git_repository_root_at_filesystem_root() {
        // This test is challenging to create reliably across platforms
        // We'll test the edge case where we reach the filesystem root
        let temp_dir = TempDir::new().unwrap();
        let base = temp_dir.path();

        // Create a single level directory
        let level1 = base.join("level1");
        fs::create_dir(&level1).unwrap();

        // Test from this directory (no .git anywhere)
        let result = find_git_repository_root_from(&level1);

        // Should return None when reaching filesystem root with no .git found
        assert!(result.is_none());
    }

    #[test]
    fn test_find_git_repository_root_depth_counting() {
        let temp_dir = TempDir::new().unwrap();
        let base = temp_dir.path();

        // Create simple structure to understand depth counting
        let level1 = base.join("level1");
        let level2 = level1.join("level2");
        let level3 = level2.join("level3");
        fs::create_dir_all(&level3).unwrap();

        // Put .git at base
        fs::create_dir(base.join(".git")).unwrap();

        // Test from different levels
        assert!(find_git_repository_root_from(base).is_some()); // depth 0
        assert!(find_git_repository_root_from(&level1).is_some()); // depth 1
        assert!(find_git_repository_root_from(&level2).is_some()); // depth 2
        assert!(find_git_repository_root_from(&level3).is_some()); // depth 3
    }

    #[test]
    #[serial]
    fn test_find_swissarmyhammer_directory_found() {
        let temp_dir = TempDir::new().unwrap();
        let base = temp_dir.path();

        // Create Git repository with .swissarmyhammer directory
        fs::create_dir(base.join(".git")).unwrap();
        fs::create_dir(base.join(".swissarmyhammer")).unwrap();

        // Change to this directory and test
        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(base).expect("Failed to change to test directory");

        let result = find_swissarmyhammer_directory();

        // Restore original directory
        let _ = env::set_current_dir(original_dir);

        assert!(result.is_some());
        assert_eq!(
            result.unwrap().canonicalize().unwrap(),
            base.join(".swissarmyhammer").canonicalize().unwrap()
        );
    }

    #[test]
    #[serial]
    fn test_find_swissarmyhammer_directory_git_no_swissarmyhammer() {
        let temp_dir = TempDir::new().unwrap();
        let base = temp_dir.path();

        // Create Git repository without .swissarmyhammer directory
        fs::create_dir(base.join(".git")).unwrap();

        // Change to this directory and test
        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(base).expect("Failed to change to test directory");

        let result = find_swissarmyhammer_directory();

        // Restore original directory
        let _ = env::set_current_dir(original_dir);

        assert!(result.is_none());
    }

    #[test]
    #[serial]
    fn test_find_swissarmyhammer_directory_no_git_repo() {
        let temp_dir = TempDir::new().unwrap();
        let base = temp_dir.path();

        // Create .swissarmyhammer directory but no Git repository
        fs::create_dir(base.join(".swissarmyhammer")).unwrap();

        // Change to this directory and test
        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(base).expect("Failed to change to test directory");

        let result = find_swissarmyhammer_directory();

        // Restore original directory
        let _ = env::set_current_dir(original_dir);

        // Should return None since no Git repository was found
        assert!(result.is_none());
    }

    #[test]
    #[serial]
    fn test_find_swissarmyhammer_directory_swissarmyhammer_is_file() {
        let temp_dir = TempDir::new().unwrap();
        let base = temp_dir.path();

        // Create Git repository with .swissarmyhammer as a file instead of directory
        fs::create_dir(base.join(".git")).unwrap();
        fs::write(base.join(".swissarmyhammer"), "not a directory").unwrap();

        // Change to this directory and test
        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(base).expect("Failed to change to test directory");

        let result = find_swissarmyhammer_directory();

        // Restore original directory
        let _ = env::set_current_dir(original_dir);

        // Should return None since .swissarmyhammer is not a directory
        assert!(result.is_none());
    }

    #[test]
    #[serial]
    fn test_find_swissarmyhammer_directory_from_subdirectory() {
        let temp_dir = TempDir::new().unwrap();
        let base = temp_dir.path();

        // Create nested structure with Git repository and .swissarmyhammer at root
        let subdir1 = base.join("src");
        let subdir2 = subdir1.join("lib");
        fs::create_dir_all(&subdir2).unwrap();

        fs::create_dir(base.join(".git")).unwrap();
        fs::create_dir(base.join(".swissarmyhammer")).unwrap();

        // Test from nested subdirectory
        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(&subdir2).expect("Failed to change to test directory");

        let result = find_swissarmyhammer_directory();

        // Restore original directory
        let _ = env::set_current_dir(original_dir);

        assert!(result.is_some());
        assert_eq!(
            result.unwrap().canonicalize().unwrap(),
            base.join(".swissarmyhammer").canonicalize().unwrap()
        );
    }

    #[test]
    #[serial]
    fn test_get_or_create_swissarmyhammer_directory_create() {
        let temp_dir = TempDir::new().unwrap();
        let base = temp_dir.path();

        // Create Git repository without .swissarmyhammer directory
        fs::create_dir(base.join(".git")).unwrap();

        // Change to this directory and test
        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(base).expect("Failed to change to test directory");

        let result = get_or_create_swissarmyhammer_directory();

        // Restore original directory
        let _ = env::set_current_dir(original_dir);

        assert!(result.is_ok());
        let swissarmyhammer_dir = result.unwrap();
        assert_eq!(
            swissarmyhammer_dir.canonicalize().unwrap(),
            base.join(".swissarmyhammer").canonicalize().unwrap()
        );

        // Verify directory was created
        assert!(base.join(".swissarmyhammer").exists());
        assert!(base.join(".swissarmyhammer").is_dir());
    }

    #[test]
    #[serial]
    fn test_get_or_create_swissarmyhammer_directory_existing() {
        let temp_dir = TempDir::new().unwrap();
        let base = temp_dir.path();

        // Create Git repository with existing .swissarmyhammer directory
        fs::create_dir(base.join(".git")).unwrap();
        fs::create_dir(base.join(".swissarmyhammer")).unwrap();

        // Change to this directory and test
        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(base).expect("Failed to change to test directory");

        let result = get_or_create_swissarmyhammer_directory();

        // Restore original directory
        let _ = env::set_current_dir(original_dir);

        assert!(result.is_ok());
        let swissarmyhammer_dir = result.unwrap();
        assert_eq!(
            swissarmyhammer_dir.canonicalize().unwrap(),
            base.join(".swissarmyhammer").canonicalize().unwrap()
        );
    }

    #[test]
    #[serial]
    fn test_get_or_create_swissarmyhammer_directory_no_git_repo() {
        let temp_dir = TempDir::new().unwrap();
        let base = temp_dir.path();

        // Create directory structure without Git repository
        fs::create_dir_all(base).unwrap();

        // Change to this directory and test
        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(base).expect("Failed to change to test directory");

        let result = get_or_create_swissarmyhammer_directory();

        // Restore original directory
        let _ = env::set_current_dir(original_dir);

        assert!(result.is_err());
        match result.unwrap_err() {
            crate::error::SwissArmyHammerError::NotInGitRepository => {
                // Expected error type
            }
            other => panic!("Expected NotInGitRepository error, got {:?}", other),
        }
    }

    #[test]
    #[serial]
    fn test_get_or_create_swissarmyhammer_directory_from_subdirectory() {
        let temp_dir = TempDir::new().unwrap();
        let base = temp_dir.path();

        // Create nested structure with Git repository at root
        let subdir1 = base.join("src");
        let subdir2 = subdir1.join("components");
        fs::create_dir_all(&subdir2).unwrap();

        fs::create_dir(base.join(".git")).unwrap();

        // Test from nested subdirectory
        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(&subdir2).expect("Failed to change to test directory");

        let result = get_or_create_swissarmyhammer_directory();

        // Restore original directory
        let _ = env::set_current_dir(original_dir);

        assert!(result.is_ok());
        let swissarmyhammer_dir = result.unwrap();
        assert_eq!(
            swissarmyhammer_dir.canonicalize().unwrap(),
            base.join(".swissarmyhammer").canonicalize().unwrap()
        );

        // Verify directory was created at repository root, not in subdirectory
        assert!(base.join(".swissarmyhammer").exists());
        assert!(base.join(".swissarmyhammer").is_dir());
        assert!(!subdir2.join(".swissarmyhammer").exists());
    }

    #[test]
    #[serial]
    fn test_get_or_create_swissarmyhammer_directory_swissarmyhammer_is_file() {
        let temp_dir = TempDir::new().unwrap();
        let base = temp_dir.path();

        // Create Git repository with .swissarmyhammer as a file instead of directory
        fs::create_dir(base.join(".git")).unwrap();
        fs::write(base.join(".swissarmyhammer"), "not a directory").unwrap();

        // Change to this directory and test
        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(base).expect("Failed to change to test directory");

        let result = get_or_create_swissarmyhammer_directory();

        // Restore original directory
        let _ = env::set_current_dir(original_dir);

        // Should fail with DirectoryCreation error since .swissarmyhammer exists as file
        assert!(result.is_err());
        match result.unwrap_err() {
            crate::error::SwissArmyHammerError::DirectoryCreation(_) => {
                // Expected error type when trying to create directory over existing file
            }
            other => panic!("Expected DirectoryCreation error, got {:?}", other),
        }
    }

    #[test]
    #[serial]
    fn test_get_or_create_swissarmyhammer_directory_multiple_git_repos() {
        let temp_dir = TempDir::new().unwrap();
        let base = temp_dir.path();

        // Create nested structure with .git at multiple levels
        let subdir = base.join("nested-repo");
        fs::create_dir_all(&subdir).unwrap();

        // Create .git directories at both levels
        fs::create_dir(base.join(".git")).unwrap();
        fs::create_dir(subdir.join(".git")).unwrap();

        // Test from nested repository
        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(&subdir).expect("Failed to change to test directory");

        let result = get_or_create_swissarmyhammer_directory();

        // Restore original directory
        let _ = env::set_current_dir(original_dir);

        assert!(result.is_ok());
        let swissarmyhammer_dir = result.unwrap();

        // Should create .swissarmyhammer in the nearest Git repository root (nested-repo)
        assert_eq!(
            swissarmyhammer_dir.canonicalize().unwrap(),
            subdir.join(".swissarmyhammer").canonicalize().unwrap()
        );

        // Verify directory was created in nested repo, not parent repo
        assert!(subdir.join(".swissarmyhammer").exists());
        assert!(subdir.join(".swissarmyhammer").is_dir());
    }

    #[test]
    #[serial]
    fn test_get_or_create_swissarmyhammer_directory_depth_limit_respected() {
        let temp_dir = TempDir::new().unwrap();
        let base = temp_dir.path();

        // Create very deep directory structure
        let mut current = base.to_path_buf();
        for i in 0..=MAX_DIRECTORY_DEPTH + 1 {
            current = current.join(format!("level{}", i));
            fs::create_dir_all(&current).unwrap();
        }

        // Put .git at the base (beyond MAX_DIRECTORY_DEPTH from deepest)
        fs::create_dir(base.join(".git")).unwrap();

        // Test from deepest directory
        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(&current).expect("Failed to change to test directory");

        let result = get_or_create_swissarmyhammer_directory();

        // Restore original directory
        let _ = env::set_current_dir(original_dir);

        // Should return NotInGitRepository error due to depth limit
        assert!(result.is_err());
        match result.unwrap_err() {
            crate::error::SwissArmyHammerError::NotInGitRepository => {
                // Expected error type when Git repository is beyond depth limit
            }
            other => panic!("Expected NotInGitRepository error, got {:?}", other),
        }
    }
}
