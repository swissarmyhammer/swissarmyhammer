//! Security utilities for path validation and resource limits
//!
//! This module provides functions to ensure safe file operations and prevent
//! potential security vulnerabilities like path traversal attacks and denial
//! of service through excessive resource consumption.

use crate::{Result, SwissArmyHammerError};
use std::path::{Path, PathBuf};

/// Maximum allowed depth for directory traversal operations
pub const MAX_DIRECTORY_DEPTH: usize = 10;

/// Maximum allowed complexity for workflow graphs (states + transitions)
pub const MAX_WORKFLOW_COMPLEXITY: usize = 1000;

/// Maximum allowed template size in bytes for untrusted templates
pub const MAX_TEMPLATE_SIZE: usize = 100_000;

/// Maximum allowed recursion depth for template rendering
pub const MAX_TEMPLATE_RECURSION_DEPTH: usize = 10;

/// Maximum allowed template variables per template
pub const MAX_TEMPLATE_VARIABLES: usize = 1000;

/// Maximum allowed template render time in milliseconds
pub const MAX_TEMPLATE_RENDER_TIME_MS: u64 = 5000;

/// Checks if a path is safe to access within a given root directory
///
/// This function validates that:
/// - The path doesn't contain dangerous components like ".."
/// - The canonical path is within the root directory
/// - The path doesn't follow symlinks outside the root
///
/// # Arguments
///
/// * `path` - The path to validate
/// * `root` - The root directory that the path must be within
///
/// # Returns
///
/// The canonical path if safe, or an error if the path is unsafe
pub fn validate_path_security(path: &Path, root: &Path) -> Result<PathBuf> {
    // First check for obvious dangerous patterns
    for component in path.components() {
        match component {
            std::path::Component::ParentDir => {
                return Err(SwissArmyHammerError::Other { message: 
                    "Path contains parent directory references (..)".to_string(),
                ));
            }
            std::path::Component::RootDir => {
                return Err(SwissArmyHammerError::Other { message: 
                    "Path contains absolute root reference".to_string(),
                ));
            }
            _ => {}
        }
    }

    // Get canonical paths to resolve symlinks and relative paths
    let canonical_root = root.canonicalize().map_err(|e| {
        SwissArmyHammerError::Other { message: format!("Failed to canonicalize root path: {e}") }
    })?;

    let full_path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    };

    // For files that don't exist yet, we need to check the parent directory
    let canonical_path = if full_path.exists() {
        full_path
            .canonicalize()
            .to_swiss_error_with_context("Failed to canonicalize path")?
    } else {
        // Check the parent directory exists and is valid
        let parent = full_path.parent().ok_or_else(|| {
            SwissArmyHammerError::Other { message: "Path has no parent directory".to_string() }
        })?;

        let canonical_parent = parent.canonicalize().map_err(|e| {
            SwissArmyHammerError::Other { message: format!("Failed to canonicalize parent path: {e}") }
        })?;

        // Ensure the parent is within the root
        if !canonical_parent.starts_with(&canonical_root) {
            return Err(SwissArmyHammerError::Other { message: format!(
                "Path '{}' parent is outside allowed directory",
                path.display()
            ) });
        }

        // Return the intended path (parent + filename)
        let filename = full_path.file_name().ok_or_else(|| {
            SwissArmyHammerError::Other { message: "Path has no filename component".to_string() }
        })?;

        canonical_parent.join(filename)
    };

    // Ensure the canonical path is within the root
    if !canonical_path.starts_with(&canonical_root) {
        return Err(SwissArmyHammerError::Other { message: format!(
            "Path '{}' is outside allowed directory",
            path.display()
        ) });
    }

    Ok(canonical_path)
}

/// Calculates the depth of a path relative to a root directory
///
/// # Arguments
///
/// * `path` - The path to check
/// * `root` - The root directory to calculate depth from
///
/// # Returns
///
/// The depth as a usize, or 0 if the path is not within the root
pub fn calculate_path_depth(path: &Path, root: &Path) -> usize {
    let canonical_root = match root.canonicalize() {
        Ok(p) => p,
        Err(_) => return 0,
    };

    let canonical_path = match path.canonicalize() {
        Ok(p) => p,
        Err(_) => return 0,
    };

    if !canonical_path.starts_with(&canonical_root) {
        return 0;
    }

    canonical_path
        .strip_prefix(&canonical_root)
        .map(|p| p.components().count())
        .unwrap_or(0)
}

/// Checks if a workflow's complexity is within acceptable limits
///
/// # Arguments
///
/// * `states_count` - Number of states in the workflow
/// * `transitions_count` - Number of transitions in the workflow
///
/// # Returns
///
/// Ok if within limits, error if too complex
pub fn validate_workflow_complexity(states_count: usize, transitions_count: usize) -> Result<()> {
    let total_complexity = states_count + transitions_count;

    if total_complexity > MAX_WORKFLOW_COMPLEXITY {
        return Err(SwissArmyHammerError::Other { message: format!(
            "Workflow too complex: {states_count} states + {transitions_count} transitions = {total_complexity} (max allowed: {MAX_WORKFLOW_COMPLEXITY})"
        )));
    }

    Ok(())
}

/// Validates template content for security risks
///
/// This function checks untrusted template content for potential security issues
/// including size limits, complexity, and dangerous patterns.
///
/// # Arguments
///
/// * `template_content` - The template content to validate
/// * `is_trusted` - Whether this template comes from a trusted source
///
/// # Returns
///
/// Ok if the template is safe to render, error if it poses security risks
pub fn validate_template_security(template_content: &str, is_trusted: bool) -> Result<()> {
    // For trusted templates (builtin, user-created), apply minimal validation
    if is_trusted {
        // Even trusted templates should have reasonable size limits
        if template_content.len() > MAX_TEMPLATE_SIZE * 10 {
            return Err(SwissArmyHammerError::Other { message: format!(
                "Template too large: {} bytes (max allowed for trusted: {})",
                template_content.len(),
                MAX_TEMPLATE_SIZE * 10
            )));
        }
        return Ok(());
    }

    // Strict validation for untrusted templates

    // Check template size
    if template_content.len() > MAX_TEMPLATE_SIZE {
        return Err(SwissArmyHammerError::Other { message: format!(
            "Template too large: {} bytes (max allowed: {MAX_TEMPLATE_SIZE})",
            template_content.len()
        )));
    }

    // Count template variables and control structures
    let variable_count = count_template_variables(template_content);
    if variable_count > MAX_TEMPLATE_VARIABLES {
        return Err(SwissArmyHammerError::Other { message: format!(
            "Too many template variables: {variable_count} (max allowed: {MAX_TEMPLATE_VARIABLES})"
        )));
    }

    // Check for excessive nesting that could cause stack overflow
    let max_nesting = check_template_nesting_depth(template_content);
    if max_nesting > MAX_TEMPLATE_RECURSION_DEPTH {
        return Err(SwissArmyHammerError::Other { message: format!(
            "Template nesting too deep: {max_nesting} levels (max allowed: {MAX_TEMPLATE_RECURSION_DEPTH})"
        )));
    }

    Ok(())
}

/// Count the number of template variables in a template
fn count_template_variables(template: &str) -> usize {
    use regex::Regex;

    // Match {{ variable }} patterns
    let variable_re = Regex::new(r"\{\{\s*(\w+)").unwrap();
    let mut variables = std::collections::HashSet::new();

    for cap in variable_re.captures_iter(template) {
        variables.insert(cap[1].to_string());
    }

    variables.len()
}

/// Check the maximum nesting depth of template control structures
fn check_template_nesting_depth(template: &str) -> usize {
    use regex::Regex;

    let open_re = Regex::new(r"\{%\s*(if|unless|for|capture|tablerow)\b").unwrap();
    let close_re = Regex::new(r"\{%\s*(endif|endunless|endfor|endcapture|endtablerow)\b").unwrap();

    let mut max_depth = 0;
    let mut current_depth: i32 = 0;

    let mut pos = 0;
    while pos < template.len() {
        if let Some(open_match) = open_re.find_at(template, pos) {
            if let Some(close_match) = close_re.find_at(template, pos) {
                if open_match.start() < close_match.start() {
                    // Opening tag comes first
                    current_depth += 1;
                    max_depth = max_depth.max(current_depth);
                    pos = open_match.end();
                } else {
                    // Closing tag comes first
                    current_depth = current_depth.saturating_sub(1);
                    pos = close_match.end();
                }
            } else {
                // Only opening tag found
                current_depth += 1;
                max_depth = max_depth.max(current_depth);
                pos = open_match.end();
            }
        } else if let Some(close_match) = close_re.find_at(template, pos) {
            // Only closing tag found
            current_depth = current_depth.saturating_sub(1);
            pos = close_match.end();
        } else {
            break;
        }
    }

    max_depth.max(0) as usize
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_validate_path_security_safe_path() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();
        let safe_file = root.join("test.txt");
        fs::write(&safe_file, "test").unwrap();

        // Test with relative path
        let result = validate_path_security(Path::new("test.txt"), root);
        if let Err(e) = &result {
            panic!("Expected Ok, got error: {e}");
        }
        assert_eq!(result.unwrap(), safe_file.canonicalize().unwrap());
    }

    #[test]
    fn test_validate_path_security_parent_dir() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();

        // Test with relative path containing parent directory
        let result = validate_path_security(Path::new("../outside.txt"), root);
        match &result {
            Ok(_) => panic!("Expected error, got Ok"),
            Err(e) => {
                let error_str = e.to_string();
                assert!(
                    error_str.contains("parent directory"),
                    "Expected 'parent directory' in error, got: {error_str}"
                );
            }
        }
    }

    #[test]
    fn test_validate_path_security_absolute_path() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();

        // Create a file outside the temp directory
        let other_temp = TempDir::new().unwrap();
        let outside_file = other_temp.path().join("outside.txt");
        fs::write(&outside_file, "test").unwrap();

        let result = validate_path_security(&outside_file, root);
        match &result {
            Ok(_) => panic!("Expected error, got Ok"),
            Err(e) => {
                let error_str = e.to_string();
                // Absolute paths are rejected early in the validation
                assert!(error_str.contains("absolute root reference") ||
                        error_str.contains("outside allowed directory"),
                    "Expected 'absolute root reference' or 'outside allowed directory' in error, got: {error_str}");
            }
        }
    }

    #[test]
    fn test_calculate_path_depth() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();

        // Create nested directories
        let level1 = root.join("level1");
        let level2 = level1.join("level2");
        let level3 = level2.join("level3");

        fs::create_dir_all(&level3).unwrap();

        assert_eq!(calculate_path_depth(root, root), 0);
        assert_eq!(calculate_path_depth(&level1, root), 1);
        assert_eq!(calculate_path_depth(&level2, root), 2);
        assert_eq!(calculate_path_depth(&level3, root), 3);
    }

    #[test]
    fn test_validate_workflow_complexity_within_limits() {
        assert!(validate_workflow_complexity(10, 20).is_ok());
        assert!(validate_workflow_complexity(100, 200).is_ok());
        assert!(validate_workflow_complexity(500, 499).is_ok());
    }

    #[test]
    fn test_validate_workflow_complexity_exceeds_limits() {
        let result = validate_workflow_complexity(600, 600);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("too complex"));
    }

    #[test]
    fn test_validate_template_security_trusted() {
        let large_template = "a".repeat(MAX_TEMPLATE_SIZE + 1000);
        // Trusted templates have higher limits
        assert!(validate_template_security(&large_template, true).is_ok());

        let very_large_template = "a".repeat(MAX_TEMPLATE_SIZE * 10 + 1);
        assert!(validate_template_security(&very_large_template, true).is_err());
    }

    #[test]
    fn test_validate_template_security_untrusted_size() {
        let large_template = "a".repeat(MAX_TEMPLATE_SIZE + 1);
        let result = validate_template_security(&large_template, false);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("too large"));
    }



    #[test]
    fn test_validate_template_security_excessive_nesting() {
        let deeply_nested = "{% if a %}{% if b %}{% if c %}{% if d %}{% if e %}{% if f %}{% if g %}{% if h %}{% if i %}{% if j %}{% if k %}deep{% endif %}{% endif %}{% endif %}{% endif %}{% endif %}{% endif %}{% endif %}{% endif %}{% endif %}{% endif %}{% endif %}";
        let result = validate_template_security(deeply_nested, false);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("nesting too deep"));
    }

    #[test]
    fn test_validate_template_security_safe_template() {
        let safe_template = "Hello {{ name }}! {% if premium %}You have premium access.{% endif %}";
        assert!(validate_template_security(safe_template, false).is_ok());
    }

    #[test]
    fn test_count_template_variables() {
        let template = "Hello {{ name }}! Your score is {{ score }} and your rank is {{ rank }}.";
        assert_eq!(count_template_variables(template), 3);
    }

    #[test]
    fn test_check_template_nesting_depth() {
        let shallow = "{% if a %}content{% endif %}";
        assert_eq!(check_template_nesting_depth(shallow), 1);

        let nested = "{% if a %}{% for item in items %}{{ item }}{% endfor %}{% endif %}";
        assert_eq!(check_template_nesting_depth(nested), 2);
    }
}
