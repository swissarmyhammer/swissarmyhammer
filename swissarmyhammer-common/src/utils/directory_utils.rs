//! Directory utilities for SwissArmyHammer operations
//!
//! This module provides utilities for finding Git repositories and managing
//! the .swissarmyhammer directory structure.

use crate::error::{Result, SwissArmyHammerError};
use std::path::{Path, PathBuf};

/// Maximum directory depth to search when looking for Git repositories
/// This prevents infinite loops and excessive filesystem traversal
const MAX_DIRECTORY_DEPTH: usize = 10;

/// Find the Git repository root starting from a specific directory
///
/// This function searches upwards from the given directory until it finds
/// a .git directory or reaches the maximum search depth.
///
/// # Arguments
///
/// * `start_dir` - The directory to start searching from
///
/// # Returns
///
/// * `Option<PathBuf>` - Some(path) if Git repository found, None otherwise
pub fn find_git_repository_root_from(start_dir: &Path) -> Option<PathBuf> {
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

/// Get or create the .swissarmyhammer directory for the current working directory
///
/// This function searches for a Git repository root starting from the current
/// directory and creates the .swissarmyhammer directory within that root.
///
/// # Returns
///
/// * `Result<PathBuf>` - Path to the .swissarmyhammer directory on success
///
/// # Errors
///
/// * `NotInGitRepository` - If not currently in a Git repository
/// * `DirectoryCreation` - If .swissarmyhammer directory cannot be created
pub fn get_or_create_swissarmyhammer_directory() -> Result<PathBuf> {
    let current_dir = match std::env::current_dir() {
        Ok(dir) => dir,
        Err(e) => {
            return Err(SwissArmyHammerError::directory_creation(e));
        }
    };
    get_or_create_swissarmyhammer_directory_from(&current_dir)
}

/// Get or create the .swissarmyhammer directory starting from a specific directory
///
/// This function searches for a Git repository root starting from the given directory
/// and creates the .swissarmyhammer directory within that root.
///
/// # Arguments
///
/// * `start_dir` - The directory to start searching from
///
/// # Returns
///
/// * `Result<PathBuf>` - Path to the .swissarmyhammer directory on success
pub fn get_or_create_swissarmyhammer_directory_from(start_dir: &Path) -> Result<PathBuf> {
    let git_root =
        find_git_repository_root_from(start_dir).ok_or(SwissArmyHammerError::NotInGitRepository)?;

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
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_find_git_repository_root_from_direct() {
        let temp_dir = TempDir::new().unwrap();
        let base = temp_dir.path();

        // Create a .git directory
        fs::create_dir_all(base.join(".git")).unwrap();

        let result = find_git_repository_root_from(base);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), base);
    }

    #[test]
    fn test_find_git_repository_root_from_subdirectory() {
        let temp_dir = TempDir::new().unwrap();
        let base = temp_dir.path();

        // Create a .git directory at root
        fs::create_dir_all(base.join(".git")).unwrap();

        // Create subdirectories
        let subdir1 = base.join("subdir1");
        let subdir2 = subdir1.join("subdir2");
        fs::create_dir_all(&subdir2).unwrap();

        let result = find_git_repository_root_from(&subdir2);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), base);
    }

    #[test]
    fn test_find_git_repository_root_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let base = temp_dir.path();

        let result = find_git_repository_root_from(base);
        assert!(result.is_none());
    }

    #[test]
    fn test_get_or_create_swissarmyhammer_directory_from_create() {
        let temp_dir = TempDir::new().unwrap();
        let base = temp_dir.path();

        // Create a .git directory to make it a Git repo
        fs::create_dir_all(base.join(".git")).unwrap();

        let result = get_or_create_swissarmyhammer_directory_from(base);
        assert!(result.is_ok());

        let swissarmyhammer_dir = result.unwrap();
        assert!(swissarmyhammer_dir.exists());
        assert!(swissarmyhammer_dir.is_dir());
        assert_eq!(swissarmyhammer_dir, base.join(".swissarmyhammer"));
    }

    #[test]
    fn test_get_or_create_swissarmyhammer_directory_from_existing() {
        let temp_dir = TempDir::new().unwrap();
        let base = temp_dir.path();

        // Create a .git directory to make it a Git repo
        fs::create_dir_all(base.join(".git")).unwrap();
        // Pre-create the .swissarmyhammer directory
        fs::create_dir_all(base.join(".swissarmyhammer")).unwrap();

        let result = get_or_create_swissarmyhammer_directory_from(base);
        assert!(result.is_ok());

        let swissarmyhammer_dir = result.unwrap();
        assert!(swissarmyhammer_dir.exists());
        assert!(swissarmyhammer_dir.is_dir());
    }

    #[test]
    fn test_get_or_create_swissarmyhammer_directory_from_no_git_repo() {
        let temp_dir = TempDir::new().unwrap();
        let base = temp_dir.path();

        let result = get_or_create_swissarmyhammer_directory_from(base);
        assert!(result.is_err());

        if let Err(SwissArmyHammerError::NotInGitRepository) = result {
            // Expected error
        } else {
            panic!("Expected NotInGitRepository error");
        }
    }
}
