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

    tracing::debug!(
        "expand_glob_patterns: processing {} patterns from cwd={}",
        patterns.len(),
        current_dir.display()
    );

    let mut direct_files = 0;
    let mut directories = 0;
    let mut globs = 0;

    for pattern in patterns {
        let path = PathBuf::from(pattern);

        if path.is_file() {
            direct_files += 1;
            handle_direct_file_path(&path, &current_dir, &mut target_files);
        } else if path.is_dir() {
            directories += 1;
            handle_directory_path(&path, config, &mut target_files)?;
        } else {
            globs += 1;
            handle_glob_pattern(pattern, &path, &current_dir, config, &mut target_files)?;
        }
    }

    tracing::debug!(
        "expand_glob_patterns: classified {} direct files, {} directories, {} glob patterns",
        direct_files,
        directories,
        globs
    );

    filter_excluded_paths(&mut target_files, config);
    apply_mtime_sorting(&mut target_files, config);

    tracing::debug!(
        "expand_glob_patterns: expanded {} patterns to {} files",
        patterns.len(),
        target_files.len()
    );

    Ok(target_files)
}

/// Handle direct file path expansion
fn handle_direct_file_path(path: &Path, current_dir: &Path, target_files: &mut Vec<PathBuf>) {
    // File is outside current directory - always include
    if !path.starts_with(current_dir) {
        target_files.push(path.to_path_buf());
        return;
    }

    // Check for hidden directories within current directory
    if is_in_hidden_directory(path, current_dir) {
        tracing::debug!(
            "Filtering out direct file path in hidden directory: {}",
            path.display()
        );
        return;
    }

    target_files.push(path.to_path_buf());
}

/// Handle directory path expansion
fn handle_directory_path(
    path: &Path,
    config: &GlobExpansionConfig,
    target_files: &mut Vec<PathBuf>,
) -> Result<()> {
    let walker = WalkBuilder::new(path)
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

    Ok(())
}

/// Handle glob pattern expansion
fn handle_glob_pattern(
    pattern: &str,
    path: &Path,
    current_dir: &Path,
    config: &GlobExpansionConfig,
    target_files: &mut Vec<PathBuf>,
) -> Result<()> {
    let glob_pattern = if path.is_absolute() {
        pattern.to_string()
    } else {
        current_dir.join(pattern).to_string_lossy().to_string()
    };

    let mut glob_options = glob::MatchOptions::new();
    glob_options.case_sensitive = config.case_sensitive;
    glob_options.require_literal_separator = false;
    glob_options.require_literal_leading_dot = false;

    if pattern.contains("**") || pattern.contains('*') || pattern.contains('?') {
        expand_glob_pattern_with_walker(pattern, current_dir, config, glob_options, target_files)?;
    } else {
        expand_simple_glob_pattern(&glob_pattern, glob_options, config, target_files)?;
    }

    Ok(())
}

/// Expand glob pattern using WalkBuilder for gitignore support
fn expand_glob_pattern_with_walker(
    pattern: &str,
    current_dir: &Path,
    config: &GlobExpansionConfig,
    glob_options: glob::MatchOptions,
    target_files: &mut Vec<PathBuf>,
) -> Result<()> {
    let (base_dir, file_pattern) = parse_glob_pattern(pattern);
    let search_dir = if base_dir.is_absolute() {
        base_dir
    } else {
        current_dir.join(base_dir)
    };

    let walker = WalkBuilder::new(&search_dir)
        .git_ignore(config.respect_gitignore)
        .git_global(config.respect_gitignore)
        .git_exclude(config.respect_gitignore)
        .ignore(config.respect_gitignore)
        .parents(true)
        .hidden(config.include_hidden)
        .build();

    let glob_pattern_obj =
        glob::Pattern::new(&file_pattern).map_err(|e| SwissArmyHammerError::Other {
            message: format!("Invalid glob pattern '{}': {}", file_pattern, e),
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

            if matches_path_against_pattern(
                entry_path,
                &search_dir,
                &file_pattern,
                &glob_pattern_obj,
                glob_options,
            ) {
                if !is_in_hidden_directory(entry_path, &search_dir) {
                    target_files.push(entry_path.to_path_buf());
                } else {
                    tracing::debug!(
                        "Skipping file in hidden directory: {}",
                        entry_path.display()
                    );
                }
            }
        }
    }

    Ok(())
}

/// Match a path against a glob pattern
fn matches_path_against_pattern(
    entry_path: &Path,
    search_dir: &Path,
    file_pattern: &str,
    glob_pattern_obj: &glob::Pattern,
    glob_options: glob::MatchOptions,
) -> bool {
    if !file_pattern.contains('/') && !file_pattern.starts_with("**") {
        if let Some(file_name) = entry_path.file_name() {
            if glob_pattern_obj.matches_with(&file_name.to_string_lossy(), glob_options) {
                tracing::debug!("Matched filename: {}", file_name.to_string_lossy());
                return true;
            }
        }
    }

    if let Ok(relative_path) = entry_path.strip_prefix(search_dir) {
        let rel_str = relative_path.to_string_lossy();
        if glob_pattern_obj.matches_with(&rel_str, glob_options) {
            return true;
        }
    }

    false
}

/// Check if a path is in a hidden directory
fn is_in_hidden_directory(entry_path: &Path, search_dir: &Path) -> bool {
    if let Ok(relative_path) = entry_path.strip_prefix(search_dir) {
        relative_path.components().any(|c| {
            let name = c.as_os_str().to_string_lossy();
            name.starts_with('.') && name != "." && name != ".."
        })
    } else {
        false
    }
}

/// Expand simple glob pattern without recursion
fn expand_simple_glob_pattern(
    glob_pattern: &str,
    glob_options: glob::MatchOptions,
    config: &GlobExpansionConfig,
    target_files: &mut Vec<PathBuf>,
) -> Result<()> {
    let entries =
        glob::glob_with(glob_pattern, glob_options).map_err(|e| SwissArmyHammerError::Other {
            message: format!("Invalid glob pattern '{}': {}", glob_pattern, e),
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

    Ok(())
}

/// Filter out excluded paths using canonicalized path comparison
fn filter_excluded_paths(target_files: &mut Vec<PathBuf>, config: &GlobExpansionConfig) {
    if config.exclude_paths.is_empty() {
        return;
    }

    target_files.retain(|file_path| {
        let should_keep = if let Ok(canonical_file) = file_path.canonicalize() {
            !config.exclude_paths.iter().any(|excluded| {
                if let Ok(canonical_excluded) = excluded.canonicalize() {
                    canonical_file.starts_with(&canonical_excluded)
                } else {
                    file_path.starts_with(excluded)
                }
            })
        } else {
            !config
                .exclude_paths
                .iter()
                .any(|excluded| file_path.starts_with(excluded))
        };

        should_keep
    });
}

/// Apply modification time sorting if requested
fn apply_mtime_sorting(target_files: &mut [PathBuf], config: &GlobExpansionConfig) {
    if config.sort_by_mtime {
        sort_files_by_modification_time(target_files);
    }
}

/// Get the modification time for a file, returning UNIX_EPOCH if unavailable
fn get_modification_time(path: &Path) -> std::time::SystemTime {
    std::fs::metadata(path)
        .ok()
        .and_then(|m| m.modified().ok())
        .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
}

/// Sort files by modification time (most recent first)
fn sort_files_by_modification_time(files: &mut [PathBuf]) {
    files.sort_by(|a, b| {
        let a_time = get_modification_time(a);
        let b_time = get_modification_time(b);
        b_time.cmp(&a_time).then_with(|| a.cmp(b))
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
        let temp_dir = TempDir::new().unwrap();

        // Initialize git repo for gitignore to work
        let repo = git2::Repository::init(temp_dir.path()).unwrap();
        // Ensure initial branch is 'main' for consistency across environments
        repo.set_head("refs/heads/main").unwrap();

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
    fn test_expand_glob_patterns_filters_all_hidden_directories() {
        let temp_dir = TempDir::new().unwrap();

        // Create regular files
        fs::write(temp_dir.path().join("visible.rs"), "fn main() {}").unwrap();

        // Create various hidden directories
        let hidden_dirs = [".git", ".vscode", ".idea", ".swissarmyhammer", ".cache"];
        for dir in &hidden_dirs {
            let hidden_dir = temp_dir.path().join(dir);
            fs::create_dir(&hidden_dir).unwrap();
            fs::write(hidden_dir.join("hidden.rs"), "fn hidden() {}").unwrap();
        }

        // Use absolute path pattern instead of changing directory
        let pattern = format!("{}/**/*.rs", temp_dir.path().display());
        let patterns = vec![pattern];
        let config = GlobExpansionConfig::default();
        let result = expand_glob_patterns(&patterns, &config).unwrap();

        // Should only find visible.rs, none of the hidden directory files
        assert_eq!(
            result.len(),
            1,
            "Expected to find 1 file, found {}",
            result.len()
        );
        assert!(result[0].ends_with("visible.rs"));

        // Verify none of the hidden directory files are included
        for dir in &hidden_dirs {
            assert!(
                !result
                    .iter()
                    .any(|p| p.components().any(|c| c.as_os_str() == *dir)),
                "Should not include files from {} directory",
                dir
            );
        }
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
