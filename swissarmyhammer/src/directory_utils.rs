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
        .or_else(|_| dirs::home_dir().map(|home| home.join(".swissarmyhammer")));

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

#[cfg(test)]
mod tests {
    use super::*;
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
}
