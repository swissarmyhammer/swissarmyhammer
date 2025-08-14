//! Plan command utilities
//!
//! This module provides specialized utilities for the plan command including
//! comprehensive file validation with enhanced error handling and user guidance.

use crate::error::PlanCommandError;
use crate::fs_utils::{FileSystem, FileSystemUtils};
use std::path::{Path, PathBuf};

/// Configuration for plan file validation
#[derive(Debug)]
pub struct PlanValidationConfig {
    /// Maximum file size in bytes (default: 10MB)
    pub max_file_size: u64,
    /// Minimum file size in bytes (default: 1 byte)
    pub min_file_size: u64,
}

impl Default for PlanValidationConfig {
    fn default() -> Self {
        Self {
            max_file_size: 10 * 1024 * 1024, // 10MB
            min_file_size: 1,
        }
    }
}

/// Represents a validated plan file
#[derive(Debug)]
pub struct ValidatedPlanFile {
    /// Canonicalized path to the plan file
    pub path: PathBuf,
    /// Content of the plan file
    pub content: String,
    /// Size of the file in bytes
    pub size: u64,
}

/// Enhanced plan file validation with comprehensive error handling
pub fn validate_plan_file_comprehensive(
    plan_filename: &str,
    config: Option<PlanValidationConfig>,
) -> Result<ValidatedPlanFile, PlanCommandError> {
    let config = config.unwrap_or_default();
    let fs_utils = FileSystemUtils::new();
    let fs = fs_utils.fs();

    validate_plan_file_with_fs(plan_filename, &config, fs)
}

/// Internal validation function that takes a file system for testability
fn validate_plan_file_with_fs(
    plan_filename: &str,
    config: &PlanValidationConfig,
    fs: &dyn FileSystem,
) -> Result<ValidatedPlanFile, PlanCommandError> {
    let path = Path::new(plan_filename);

    // Check file existence
    if !fs.exists(path) {
        return Err(PlanCommandError::FileNotFound {
            path: plan_filename.to_string(),
            source: std::io::Error::new(std::io::ErrorKind::NotFound, "File not found"),
        });
    }

    // Check if it's a file
    if !fs.is_file(path) {
        return Err(PlanCommandError::InvalidFileFormat {
            path: plan_filename.to_string(),
            reason: "Path points to a directory, not a file".to_string(),
        });
    }

    // Read file content for further validation
    let content = match fs.read_to_string(path) {
        Ok(content) => content,
        Err(e) => {
            match e {
                crate::error::SwissArmyHammerError::Io(io_err) => {
                    // Check both the error kind and the message for permission denied
                    // Since with_io_context wraps errors with ErrorKind::Other, we need
                    // to also check the error message content
                    let is_permission_denied = io_err.kind()
                        == std::io::ErrorKind::PermissionDenied
                        || io_err.to_string().contains("Permission denied");

                    // Check for "is a directory" errors
                    let is_directory = io_err.to_string().contains("Is a directory");

                    return Err(if is_permission_denied {
                        PlanCommandError::PermissionDenied {
                            path: plan_filename.to_string(),
                            source: io_err,
                        }
                    } else if is_directory {
                        // Use the existing error type from the comprehensive error system
                        PlanCommandError::InvalidFileFormat {
                            path: plan_filename.to_string(),
                            reason: format!("Path is a directory, not a file: {io_err}"),
                        }
                    } else {
                        PlanCommandError::InvalidFileFormat {
                            path: plan_filename.to_string(),
                            reason: format!("Cannot read file: {io_err}"),
                        }
                    });
                }
                _ => {
                    return Err(PlanCommandError::InvalidFileFormat {
                        path: plan_filename.to_string(),
                        reason: format!("Unexpected error reading file: {e}"),
                    });
                }
            }
        }
    };

    // Check file size
    let size = content.len() as u64;
    if size > config.max_file_size {
        return Err(PlanCommandError::FileTooLarge {
            path: plan_filename.to_string(),
            size,
        });
    }

    // Check if file is empty or only whitespace
    if size < config.min_file_size || content.trim().is_empty() {
        return Err(PlanCommandError::EmptyPlanFile {
            path: plan_filename.to_string(),
        });
    }

    // Check UTF-8 validity (already done by read_to_string, but explicit check)
    if content.contains('\0') {
        return Err(PlanCommandError::InvalidFileFormat {
            path: plan_filename.to_string(),
            reason: "File contains null bytes - may be binary".to_string(),
        });
    }

    // Try to canonicalize the path
    let canonical_path = match path.canonicalize() {
        Ok(canonical_path) => canonical_path,
        Err(_) => {
            // If canonicalization fails, use the original path
            // This can happen in some test environments
            path.to_path_buf()
        }
    };

    Ok(ValidatedPlanFile {
        path: canonical_path,
        content,
        size,
    })
}

/// Validate that the issues directory is accessible and writable
pub fn validate_issues_directory() -> Result<PathBuf, PlanCommandError> {
    let fs_utils = FileSystemUtils::new();
    let fs = fs_utils.fs();

    let issues_dir = Path::new("./issues");

    // Check if directory exists, create if it doesn't
    if !fs.exists(issues_dir) {
        match fs.create_dir_all(issues_dir) {
            Ok(()) => {}
            Err(e) => {
                return Err(PlanCommandError::IssuesDirectoryNotWritable {
                    path: issues_dir.display().to_string(),
                    source: match e {
                        crate::error::SwissArmyHammerError::Io(io_err) => io_err,
                        _ => std::io::Error::new(
                            std::io::ErrorKind::Other,
                            format!("Failed to create issues directory: {e}"),
                        ),
                    },
                });
            }
        }
    } else if !fs.is_dir(issues_dir) {
        return Err(PlanCommandError::IssuesDirectoryNotWritable {
            path: issues_dir.display().to_string(),
            source: std::io::Error::new(
                std::io::ErrorKind::NotADirectory,
                "Issues path exists but is not a directory",
            ),
        });
    }

    // Test writability by creating a temporary file
    let test_file = issues_dir.join(".write_test");
    match fs.write(&test_file, "test") {
        Ok(()) => {
            // Clean up test file
            let _ = fs.remove_file(&test_file);
        }
        Err(e) => {
            return Err(PlanCommandError::IssuesDirectoryNotWritable {
                path: issues_dir.display().to_string(),
                source: match e {
                    crate::error::SwissArmyHammerError::Io(io_err) => io_err,
                    _ => std::io::Error::new(
                        std::io::ErrorKind::PermissionDenied,
                        format!("Cannot write to issues directory: {e}"),
                    ),
                },
            });
        }
    }

    // Return canonicalized path
    match issues_dir.canonicalize() {
        Ok(canonical_path) => Ok(canonical_path),
        Err(_) => Ok(issues_dir.to_path_buf()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fs_utils::tests::MockFileSystem;
    use std::sync::Arc;

    #[test]
    fn test_validate_plan_file_success() {
        let mock_fs = Arc::new(MockFileSystem::new());

        // Set up a valid plan file
        let path = Path::new("test_plan.md");
        let content = "# Test Plan\n\nThis is a test plan with some content.";

        mock_fs
            .write(path, content)
            .expect("Failed to write test file");

        let config = PlanValidationConfig::default();
        let result = validate_plan_file_with_fs("test_plan.md", &config, mock_fs.as_ref());

        assert!(result.is_ok());
        let validated_file = result.unwrap();
        assert_eq!(validated_file.content, content);
        assert_eq!(validated_file.size, content.len() as u64);
    }

    #[test]
    fn test_validate_plan_file_not_found() {
        let mock_fs = Arc::new(MockFileSystem::new());

        let config = PlanValidationConfig::default();
        let result = validate_plan_file_with_fs("nonexistent.md", &config, mock_fs.as_ref());

        assert!(result.is_err());
        match result.unwrap_err() {
            PlanCommandError::FileNotFound { path, .. } => {
                assert_eq!(path, "nonexistent.md");
            }
            _ => panic!("Expected FileNotFound error"),
        }
    }

    #[test]
    fn test_validate_plan_file_directory() {
        let mock_fs = Arc::new(MockFileSystem::new());

        // Set up a directory instead of a file
        let dir_path = Path::new("test_directory");
        mock_fs
            .create_dir_all(dir_path)
            .expect("Failed to create test directory");

        let config = PlanValidationConfig::default();
        let result = validate_plan_file_with_fs("test_directory", &config, mock_fs.as_ref());

        assert!(result.is_err());
        match result.unwrap_err() {
            PlanCommandError::InvalidFileFormat { path, reason } => {
                assert_eq!(path, "test_directory");
                assert!(reason.contains("directory"));
            }
            _ => panic!("Expected InvalidFileFormat error"),
        }
    }

    #[test]
    fn test_validate_plan_file_empty() {
        let mock_fs = Arc::new(MockFileSystem::new());

        // Set up an empty file
        let path = Path::new("empty.md");
        mock_fs.write(path, "").expect("Failed to write empty file");

        let config = PlanValidationConfig::default();
        let result = validate_plan_file_with_fs("empty.md", &config, mock_fs.as_ref());

        assert!(result.is_err());
        match result.unwrap_err() {
            PlanCommandError::EmptyPlanFile { path } => {
                assert_eq!(path, "empty.md");
            }
            _ => panic!("Expected EmptyPlanFile error"),
        }
    }

    #[test]
    fn test_validate_plan_file_whitespace_only() {
        let mock_fs = Arc::new(MockFileSystem::new());

        // Set up a file with only whitespace
        let path = Path::new("whitespace.md");
        mock_fs
            .write(path, "   \n\t  \n  ")
            .expect("Failed to write whitespace file");

        let config = PlanValidationConfig::default();
        let result = validate_plan_file_with_fs("whitespace.md", &config, mock_fs.as_ref());

        assert!(result.is_err());
        match result.unwrap_err() {
            PlanCommandError::EmptyPlanFile { path } => {
                assert_eq!(path, "whitespace.md");
            }
            _ => panic!("Expected EmptyPlanFile error"),
        }
    }

    #[test]
    fn test_validate_plan_file_too_large() {
        let mock_fs = Arc::new(MockFileSystem::new());

        // Create a large content string
        let large_content = "x".repeat(100);
        let path = Path::new("large.md");
        mock_fs
            .write(path, &large_content)
            .expect("Failed to write large file");

        // Use a small max size for testing
        let config = PlanValidationConfig {
            max_file_size: 50, // Smaller than our content
            min_file_size: 1,
        };

        let result = validate_plan_file_with_fs("large.md", &config, mock_fs.as_ref());

        assert!(result.is_err());
        match result.unwrap_err() {
            PlanCommandError::FileTooLarge { path, size } => {
                assert_eq!(path, "large.md");
                assert_eq!(size, large_content.len() as u64);
            }
            _ => panic!("Expected FileTooLarge error"),
        }
    }

    #[test]
    fn test_validate_plan_file_with_null_bytes() {
        let mock_fs = Arc::new(MockFileSystem::new());

        // Set up a file with null bytes (simulating binary content)
        let path = Path::new("binary.md");
        let content = "Valid content\0with null byte";
        mock_fs
            .write(path, content)
            .expect("Failed to write binary file");

        let config = PlanValidationConfig::default();
        let result = validate_plan_file_with_fs("binary.md", &config, mock_fs.as_ref());

        assert!(result.is_err());
        match result.unwrap_err() {
            PlanCommandError::InvalidFileFormat { path, reason } => {
                assert_eq!(path, "binary.md");
                assert!(reason.contains("null bytes"));
            }
            _ => panic!("Expected InvalidFileFormat error"),
        }
    }

    #[test]
    fn test_plan_validation_config_default() {
        let config = PlanValidationConfig::default();
        assert_eq!(config.max_file_size, 10 * 1024 * 1024);
        assert_eq!(config.min_file_size, 1);
    }

    #[test]
    fn test_plan_validation_config_custom() {
        let config = PlanValidationConfig {
            max_file_size: 1024,
            min_file_size: 10,
        };
        assert_eq!(config.max_file_size, 1024);
        assert_eq!(config.min_file_size, 10);
    }
}
