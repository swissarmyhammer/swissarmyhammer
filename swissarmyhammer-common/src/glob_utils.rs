//! Glob pattern expansion utilities
//!
//! This module provides unified glob pattern expansion functionality with gitignore support.
//! It consolidates glob logic that was previously duplicated across multiple crates.

use crate::error::{Result, SwissArmyHammerError};
use ignore::WalkBuilder;
use std::path::{Path, PathBuf};

/// Maximum number of files to return from glob expansion
pub const MAX_FILES: usize = 10_000;

/// Configuration for glob pattern expansion
#[derive(Debug, Clone)]
pub struct GlobExpansionConfig {
    /// Whether to respect .gitignore files
    pub respect_gitignore: bool,
    /// Whether pattern matching should be case sensitive
    pub case_sensitive: bool,
    /// Whether to include hidden files
    pub include_hidden: bool,
    /// Maximum number of files to return
    pub max_files: usize,
    /// Whether to sort results by modification time (most recent first)
    pub sort_by_mtime: bool,
    /// Paths to explicitly exclude (e.g., .swissarmyhammer directory)
    pub exclude_paths: Vec<PathBuf>,
}

impl Default for GlobExpansionConfig {
    fn default() -> Self {
        Self {
            respect_gitignore: true,
            case_sensitive: false,
            include_hidden: false,
            max_files: MAX_FILES,
            sort_by_mtime: false,
            exclude_paths: Vec::new(),
        }
    }
}

/// Expand glob patterns to file paths with optional gitignore support
///
/// # Arguments
/// * `patterns` - Array of glob patterns to expand (e.g., "**/*.rs", "src/**/*.py")
/// * `config` - Configuration for glob expansion behavior
///
/// # Returns
/// * `Ok(Vec<PathBuf>)` - List of file paths matching the patterns
/// * `Err` - If patterns are invalid or filesystem operations fail
///
/// # Examples
/// ```
/// use swissarmyhammer_common::glob_utils::{expand_glob_patterns, GlobExpansionConfig};
///
/// let patterns = vec!["**/*.rs".to_string()];
/// let config = GlobExpansionConfig::default();
/// let files = expand_glob_patterns(&patterns, &config)?;
/// ```
pub fn expand_glob_patterns(
    patterns: &[String],
    config: &GlobExpansionConfig,
) -> Result<Vec<PathBuf>> {
    let mut target_files = Vec::new();
    let current_dir = std::env::current_dir().map_err(|e| SwissArmyHammerError::Other {
        message: format!("Failed to get current directory: {}", e),
    })?;

    for pattern in patterns {
        // Check if this is a direct file or directory path
        let path = PathBuf::from(pattern);
        if path.is_file() {
            // Filter out .git and .swissarmyhammer files even for direct paths
            let should_include = !path.components().any(|c| {
                let name = c.as_os_str().to_string_lossy();
                name == ".git" || name == ".swissarmyhammer"
            });
            if should_include {
                target_files.push(path);
            } else {
                tracing::info!("Filtering out direct file path: {}", path.display());
            }
            continue;
        } else if path.is_dir() {
            // Use WalkBuilder to respect gitignore when walking directories
            let walker = WalkBuilder::new(&path)
                .git_ignore(config.respect_gitignore)
                .git_global(config.respect_gitignore)
                .git_exclude(config.respect_gitignore)
                .ignore(config.respect_gitignore)
                .parents(true)
                .hidden(config.include_hidden)
                .build();

            for entry in walker {
                if target_files.len() >= config.max_files {
                    break;
                }
                if let Ok(dir_entry) = entry {
                    let entry_path = dir_entry.path();
                    if entry_path.is_file() {
                        target_files.push(entry_path.to_path_buf());
                    }
                }
            }
            continue;
        }

        // Otherwise treat as a glob pattern
        let glob_pattern = if path.is_absolute() {
            pattern.clone()
        } else {
            current_dir.join(pattern).to_string_lossy().to_string()
        };

        // Configure glob options
        let mut glob_options = glob::MatchOptions::new();
        glob_options.case_sensitive = config.case_sensitive;
        glob_options.require_literal_separator = false;
        glob_options.require_literal_leading_dot = false;

        // For patterns like **/*.rs, we need to use WalkBuilder with pattern matching
        if pattern.contains("**") || pattern.contains('*') || pattern.contains('?') {
            // Use WalkBuilder for gitignore support with glob pattern matching
            let search_dir = if path.is_absolute() {
                path.parent().unwrap_or(&current_dir).to_path_buf()
            } else {
                current_dir.clone()
            };

            let walker = WalkBuilder::new(&search_dir)
                .git_ignore(config.respect_gitignore)
                .git_global(config.respect_gitignore)
                .git_exclude(config.respect_gitignore)
                .ignore(config.respect_gitignore)
                .parents(true)
                .hidden(config.include_hidden)
                .build();

            // Compile glob pattern
            let glob_pattern_obj =
                glob::Pattern::new(pattern).map_err(|e| SwissArmyHammerError::Other {
                    message: format!("Invalid glob pattern '{}': {}", pattern, e),
                })?;

            for entry in walker {
                if target_files.len() >= config.max_files {
                    break;
                }
                if let Ok(dir_entry) = entry {
                    let entry_path = dir_entry.path();
                    if !entry_path.is_file() {
                        continue;
                    }

                    let mut matched = false;

                    // For patterns like "*.txt", match against filename
                    if !pattern.contains('/') && !pattern.starts_with("**") {
                        if let Some(file_name) = entry_path.file_name() {
                            if glob_pattern_obj
                                .matches_with(&file_name.to_string_lossy(), glob_options)
                            {
                                matched = true;
                            }
                        }
                    }

                    // For patterns like "**/*.rs" or "src/**/*.py", match against relative path
                    if !matched {
                        if let Ok(relative_path) = entry_path.strip_prefix(&search_dir) {
                            if glob_pattern_obj
                                .matches_with(&relative_path.to_string_lossy(), glob_options)
                            {
                                matched = true;
                            }
                        }
                    }

                    if matched {
                        target_files.push(entry_path.to_path_buf());
                    }
                }
            }
        } else {
            // Use basic glob for simple patterns
            let entries = glob::glob_with(&glob_pattern, glob_options).map_err(|e| {
                SwissArmyHammerError::Other {
                    message: format!("Invalid glob pattern '{}': {}", pattern, e),
                }
            })?;

            for entry in entries {
                if target_files.len() >= config.max_files {
                    break;
                }
                if let Ok(path) = entry {
                    if path.is_file() {
                        target_files.push(path);
                    }
                }
            }
        }
    }

    // Filter out .git and .swissarmyhammer directories from all results
    let before_filter = target_files.len();
    target_files.retain(|file_path| {
        let should_keep = !file_path.components().any(|c| {
            let name = c.as_os_str().to_string_lossy();
            name == ".git" || name == ".swissarmyhammer"
        });
        if !should_keep {
            tracing::debug!("Filtering out: {}", file_path.display());
        }
        should_keep
    });
    let filtered_count = before_filter - target_files.len();
    if filtered_count > 0 {
        tracing::info!(
            "Filtered out {} .git/.swissarmyhammer files ({} files remaining)",
            filtered_count,
            target_files.len()
        );
    }

    // Filter out excluded paths using canonicalized path comparison
    if !config.exclude_paths.is_empty() {
        target_files.retain(|file_path| {
            // Try to canonicalize paths for accurate comparison
            let should_keep = if let Ok(canonical_file) = file_path.canonicalize() {
                // Check if file is under any excluded path
                !config.exclude_paths.iter().any(|excluded| {
                    if let Ok(canonical_excluded) = excluded.canonicalize() {
                        canonical_file.starts_with(&canonical_excluded)
                    } else {
                        // If excluded path can't be canonicalized, try direct comparison
                        file_path.starts_with(excluded)
                    }
                })
            } else {
                // If file can't be canonicalized, try direct comparison with excluded paths
                !config
                    .exclude_paths
                    .iter()
                    .any(|excluded| file_path.starts_with(excluded))
            };

            should_keep
        });
    }

    // Sort by modification time if requested
    if config.sort_by_mtime {
        sort_files_by_modification_time(&mut target_files);
    }

    Ok(target_files)
}

/// Sort files by modification time (most recent first)
fn sort_files_by_modification_time(files: &mut [PathBuf]) {
    use std::time::SystemTime;

    files.sort_by(|a, b| {
        let a_metadata = std::fs::metadata(a).ok();
        let b_metadata = std::fs::metadata(b).ok();

        match (a_metadata, b_metadata) {
            (Some(a_meta), Some(b_meta)) => {
                let a_time = a_meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
                let b_time = b_meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
                b_time.cmp(&a_time) // Most recent first
            }
            (Some(_), None) => std::cmp::Ordering::Less, // Files with metadata come first
            (None, Some(_)) => std::cmp::Ordering::Greater, // Files with metadata come first
            (None, None) => a.cmp(b),                    // Fallback to lexicographic
        }
    });
}

/// Validate a glob pattern for common issues
///
/// # Arguments
/// * `pattern` - The glob pattern to validate
///
/// # Returns
/// * `Ok(())` - If the pattern is valid
/// * `Err` - If the pattern is invalid
pub fn validate_glob_pattern(pattern: &str) -> Result<()> {
    if pattern.trim().is_empty() {
        return Err(SwissArmyHammerError::Other {
            message: "Pattern cannot be empty".to_string(),
        });
    }

    if pattern.len() > 1000 {
        return Err(SwissArmyHammerError::Other {
            message: "Pattern is too long (maximum 1000 characters)".to_string(),
        });
    }

    // Validate pattern syntax by trying to compile it
    glob::Pattern::new(pattern).map_err(|e| SwissArmyHammerError::Other {
        message: format!("Invalid glob pattern: {}", e),
    })?;

    Ok(())
}

/// Check if a file path matches a glob pattern
///
/// # Arguments
/// * `path` - The file path to test
/// * `pattern` - The glob pattern to match against
/// * `case_sensitive` - Whether matching should be case sensitive
///
/// # Returns
/// * `Ok(bool)` - True if the path matches the pattern
/// * `Err` - If the pattern is invalid
pub fn matches_glob_pattern(path: &Path, pattern: &str, case_sensitive: bool) -> Result<bool> {
    let glob_pattern = glob::Pattern::new(pattern).map_err(|e| SwissArmyHammerError::Other {
        message: format!("Invalid glob pattern '{}': {}", pattern, e),
    })?;

    let mut match_options = glob::MatchOptions::new();
    match_options.case_sensitive = case_sensitive;
    match_options.require_literal_separator = false;
    match_options.require_literal_leading_dot = false;

    Ok(glob_pattern.matches_path_with(path, match_options))
}

/// Parse a glob pattern to extract base directory and file pattern
///
/// # Arguments
/// * `pattern` - The glob pattern to parse
///
/// # Returns
/// * `(PathBuf, String)` - Base directory and file pattern components
///
/// # Examples
/// ```
/// use swissarmyhammer_common::glob_utils::parse_glob_pattern;
///
/// let (base_dir, file_pattern) = parse_glob_pattern("src/**/*.rs");
/// assert_eq!(base_dir.to_str().unwrap(), "src");
/// assert_eq!(file_pattern, "**/*.rs");
/// ```
pub fn parse_glob_pattern(pattern: &str) -> (PathBuf, String) {
    let path = Path::new(pattern);

    // Find the first component with glob characters
    let mut base_components = Vec::new();
    let mut pattern_components = Vec::new();
    let mut found_glob = false;

    for component in path.components() {
        let component_str = component.as_os_str().to_string_lossy();
        if !found_glob
            && !component_str.contains('*')
            && !component_str.contains('?')
            && !component_str.contains('[')
        {
            base_components.push(component);
        } else {
            found_glob = true;
            pattern_components.push(component_str.to_string());
        }
    }

    let base_dir = if base_components.is_empty() {
        PathBuf::from(".")
    } else {
        base_components.iter().collect()
    };

    let file_pattern = if pattern_components.is_empty() {
        "*".to_string()
    } else {
        pattern_components.join("/")
    };

    (base_dir, file_pattern)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_expand_glob_patterns_single_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.rs");
        fs::write(&file_path, "fn main() {}").unwrap();

        let patterns = vec![file_path.to_string_lossy().to_string()];
        let config = GlobExpansionConfig::default();
        let result = expand_glob_patterns(&patterns, &config).unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0], file_path);
    }

    #[test]
    fn test_expand_glob_patterns_directory() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("file1.rs"), "fn main() {}").unwrap();
        fs::write(temp_dir.path().join("file2.rs"), "fn test() {}").unwrap();

        let patterns = vec![temp_dir.path().to_string_lossy().to_string()];
        let config = GlobExpansionConfig::default();
        let result = expand_glob_patterns(&patterns, &config).unwrap();

        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_expand_glob_patterns_wildcard() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("file1.rs"), "fn main() {}").unwrap();
        fs::write(temp_dir.path().join("file2.rs"), "fn test() {}").unwrap();
        fs::write(temp_dir.path().join("file3.txt"), "text").unwrap();

        // Change to temp directory and use relative pattern
        let orig_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        let patterns = vec!["*.rs".to_string()];
        let config = GlobExpansionConfig::default();
        let result = expand_glob_patterns(&patterns, &config).unwrap();

        std::env::set_current_dir(orig_dir).unwrap();

        assert_eq!(result.len(), 2);
        assert!(result.iter().all(|p| p.extension().unwrap() == "rs"));
    }

    #[test]
    fn test_expand_glob_patterns_recursive() {
        let temp_dir = TempDir::new().unwrap();
        let subdir = temp_dir.path().join("src");
        fs::create_dir(&subdir).unwrap();
        fs::write(temp_dir.path().join("root.rs"), "fn main() {}").unwrap();
        fs::write(subdir.join("lib.rs"), "fn test() {}").unwrap();

        // Change to temp directory and use relative pattern
        let orig_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        let patterns = vec!["**/*.rs".to_string()];
        let config = GlobExpansionConfig::default();
        let result = expand_glob_patterns(&patterns, &config).unwrap();

        std::env::set_current_dir(orig_dir).unwrap();

        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_expand_glob_patterns_multiple_patterns() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("file1.rs"), "fn main() {}").unwrap();
        fs::write(temp_dir.path().join("file2.txt"), "text").unwrap();

        // Change to temp directory and use relative patterns
        let orig_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        let patterns = vec!["*.rs".to_string(), "*.txt".to_string()];
        let config = GlobExpansionConfig::default();
        let result = expand_glob_patterns(&patterns, &config).unwrap();

        std::env::set_current_dir(orig_dir).unwrap();

        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_expand_glob_patterns_with_exclusions() {
        let temp_dir = TempDir::new().unwrap();

        // Create files in different locations
        fs::write(temp_dir.path().join("include.rs"), "fn main() {}").unwrap();

        // Create excluded directory with files
        let excluded_dir = temp_dir.path().join("excluded");
        fs::create_dir(&excluded_dir).unwrap();
        fs::write(excluded_dir.join("skip.rs"), "fn skip() {}").unwrap();

        // Change to temp directory
        let orig_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        // Configure to exclude the "excluded" directory
        let config = GlobExpansionConfig {
            exclude_paths: vec![excluded_dir.clone()],
            ..Default::default()
        };

        let patterns = vec!["**/*.rs".to_string()];
        let result = expand_glob_patterns(&patterns, &config).unwrap();

        std::env::set_current_dir(orig_dir).unwrap();

        // Should only find include.rs, not skip.rs
        assert_eq!(result.len(), 1);
        assert!(result[0].ends_with("include.rs"));
    }

    #[test]
    fn test_expand_glob_patterns_respects_gitignore() {
        use std::process::Command;

        let temp_dir = TempDir::new().unwrap();

        // Initialize git repo for gitignore to work
        Command::new("git")
            .args(["init"])
            .current_dir(temp_dir.path())
            .output()
            .unwrap();

        fs::write(temp_dir.path().join(".gitignore"), "ignored.rs\n").unwrap();
        fs::write(temp_dir.path().join("included.rs"), "fn main() {}").unwrap();
        fs::write(temp_dir.path().join("ignored.rs"), "fn test() {}").unwrap();

        // Use directory pattern which triggers WalkBuilder with gitignore support
        let patterns = vec![temp_dir.path().to_string_lossy().to_string()];
        let config = GlobExpansionConfig::default();
        let result = expand_glob_patterns(&patterns, &config).unwrap();

        // Check that ignored.rs is not in results and included.rs is
        assert!(result.iter().any(|p| p.ends_with("included.rs")));
        assert!(!result.iter().any(|p| p.ends_with("ignored.rs")));
    }

    #[test]
    fn test_expand_glob_patterns_empty_on_no_match() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("file.txt"), "text").unwrap();

        // Change to temp directory and use relative pattern
        let orig_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        let patterns = vec!["*.rs".to_string()];
        let config = GlobExpansionConfig::default();
        let result = expand_glob_patterns(&patterns, &config).unwrap();

        std::env::set_current_dir(orig_dir).unwrap();

        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_validate_glob_pattern_valid() {
        assert!(validate_glob_pattern("**/*.rs").is_ok());
        assert!(validate_glob_pattern("src/*.txt").is_ok());
        assert!(validate_glob_pattern("*.{rs,py}").is_ok());
    }

    #[test]
    fn test_validate_glob_pattern_empty() {
        assert!(validate_glob_pattern("").is_err());
        assert!(validate_glob_pattern("   ").is_err());
    }

    #[test]
    fn test_validate_glob_pattern_too_long() {
        let long_pattern = "a".repeat(1001);
        assert!(validate_glob_pattern(&long_pattern).is_err());
    }

    #[test]
    fn test_matches_glob_pattern() {
        let path = Path::new("src/main.rs");
        assert!(matches_glob_pattern(path, "**/*.rs", false).unwrap());
        assert!(matches_glob_pattern(path, "src/*.rs", false).unwrap());
        assert!(!matches_glob_pattern(path, "*.txt", false).unwrap());
    }

    #[test]
    fn test_parse_glob_pattern() {
        let (base_dir, file_pattern) = parse_glob_pattern("src/**/*.rs");
        assert_eq!(base_dir, PathBuf::from("src"));
        assert_eq!(file_pattern, "**/*.rs");

        let (base_dir, file_pattern) = parse_glob_pattern("**/*.py");
        assert_eq!(base_dir, PathBuf::from("."));
        assert_eq!(file_pattern, "**/*.py");

        let (base_dir, file_pattern) = parse_glob_pattern("*.txt");
        assert_eq!(base_dir, PathBuf::from("."));
        assert_eq!(file_pattern, "*.txt");
    }

    #[test]
    fn test_config_defaults() {
        let config = GlobExpansionConfig::default();
        assert!(config.respect_gitignore);
        assert!(!config.case_sensitive);
        assert!(!config.include_hidden);
        assert_eq!(config.max_files, MAX_FILES);
        assert!(!config.sort_by_mtime);
    }
}
