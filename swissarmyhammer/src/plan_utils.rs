//! Plan command utilities
//!
//! This module provides specialized utilities for the plan command including
//! comprehensive file validation with enhanced error handling and user guidance.

use crate::fs_utils::{FileSystem, FileSystemUtils};
use std::path::{Path, PathBuf};
use thiserror::Error as ThisError;

/// Plan command specific errors
#[derive(Debug, ThisError)]
#[non_exhaustive]
pub enum PlanCommandError {
    /// Plan file not found
    #[error("Plan file not found: {path}")]
    FileNotFound {
        /// The file path that was not found
        path: String,
        #[source]
        /// The underlying IO error
        source: std::io::Error,
    },

    /// Permission denied accessing plan file
    #[error("Permission denied accessing plan file: {path}")]
    PermissionDenied {
        /// The file path that could not be accessed
        path: String,
        #[source]
        /// The underlying IO error
        source: std::io::Error,
    },

    /// Invalid plan file format
    #[error("Invalid plan file format: {path}\nReason: {reason}")]
    InvalidFileFormat {
        /// The file path with invalid format
        path: String,
        /// The reason why the file format is invalid
        reason: String,
    },

    /// Workflow execution failed for plan
    #[error("Workflow execution failed for plan: {plan_filename}")]
    WorkflowExecutionFailed {
        /// The plan filename that failed workflow execution
        plan_filename: String,
        #[source]
        /// The underlying workflow error
        source: WorkflowError,
    },

    /// Issue creation failed during planning
    #[error("Issue creation failed during planning")]
    IssueCreationFailed {
        /// The plan filename during which issue creation failed
        plan_filename: String,
        #[source]
        /// The underlying error that caused issue creation to fail
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    /// Plan file is empty or contains no valid content
    #[error("Plan file is empty or contains no valid content: {path}")]
    EmptyPlanFile {
        /// The path of the empty plan file
        path: String,
    },

    /// Plan file too large to process
    #[error("Plan file too large to process: {path} ({size} bytes)")]
    FileTooLarge {
        /// The path of the oversized file
        path: String,
        /// The size of the file in bytes
        size: u64,
    },

    /// Issues directory is not writable
    #[error("Issues directory is not writable")]
    IssuesDirectoryNotWritable {
        /// The path of the issues directory
        path: String,
        #[source]
        /// The underlying IO error
        source: std::io::Error,
    },

    /// Specification file has insufficient content
    #[error("Specification file has insufficient content: {path} ({length} characters)")]
    InsufficientContent {
        /// The path of the specification file
        path: String,
        /// The length of the content in characters
        length: usize,
    },
}

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

/// Validate specification content for planning suitability
fn validate_specification_content(content: &str, path: &str) -> Result<(), PlanCommandError> {
    let trimmed_content = content.trim();

    // Check for minimum content length (50 characters for meaningful content)
    if trimmed_content.len() < 50 {
        return Err(PlanCommandError::InsufficientContent {
            path: path.to_string(),
            length: trimmed_content.len(),
        });
    }

    // Look for common specification sections (case-insensitive)
    let content_lower = content.to_lowercase();
    let has_overview = content_lower.contains("overview")
        || content_lower.contains("goal")
        || content_lower.contains("purpose")
        || content_lower.contains("summary");

    let has_requirements = content_lower.contains("requirements")
        || content_lower.contains("specification")
        || content_lower.contains("features")
        || content_lower.contains("acceptance criteria")
        || content_lower.contains("tasks")
        || content_lower.contains("implementation");

    // Log warning for potentially missing sections, but don't block execution
    if !has_overview && !has_requirements {
        tracing::warn!(
            "Specification '{}' may benefit from adding overview/goal and requirements sections",
            path
        );

        // This is just a warning - we still allow the file to be processed
        // In the future, this could become a UnsuitableForPlanning error if needed
    }

    Ok(())
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

    // Validate specification content for planning suitability
    validate_specification_content(&content, plan_filename)?;

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

    #[test]
    fn test_validate_specification_content_valid() {
        let content = "# Feature Specification\n\n## Overview\n\nThis is a comprehensive specification for a new feature.\n\n## Requirements\n\n- Requirement 1\n- Requirement 2";
        let result = validate_specification_content(content, "test.md");
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_specification_content_insufficient_content() {
        let content = "# Short";
        let result = validate_specification_content(content, "test.md");

        assert!(result.is_err());
        match result.unwrap_err() {
            PlanCommandError::InsufficientContent { path, length } => {
                assert_eq!(path, "test.md");
                assert_eq!(length, 7); // "# Short".len()
            }
            _ => panic!("Expected InsufficientContent error"),
        }
    }

    #[test]
    fn test_validate_specification_content_with_overview() {
        let content = "# Project Plan\n\n## Goal\n\nThis project aims to implement new functionality that will improve user experience.";
        let result = validate_specification_content(content, "test.md");
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_specification_content_with_requirements() {
        let content = "# Implementation Plan\n\n## Requirements\n\n1. Must support user authentication\n2. Should integrate with existing APIs\n3. Performance requirements include sub-second response times";
        let result = validate_specification_content(content, "test.md");
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_specification_content_case_insensitive() {
        let content = "# FEATURE SPECIFICATION\n\nThis document contains an OVERVIEW of the new feature and outlines the REQUIREMENTS for implementation.";
        let result = validate_specification_content(content, "test.md");
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_specification_content_various_section_names() {
        // Test different variations of section names
        let test_cases = vec![
            "# Plan\n\n## Purpose\nThis defines the purpose of our work and what we want to achieve with this implementation.",
            "# Specification\n\n## Summary\nThis summarizes our approach and outlines the key components of the solution.",
            "# Document\n\n## Features\nThese are the features we need to implement in our application.",
            "# Plan\n\n## Implementation\nThis describes how we'll implement the solution with detailed steps.",
            "# Document\n\n## Acceptance Criteria\nThese are the acceptance criteria that define when the work is complete.",
            "# Plan\n\n## Tasks\nThese are the specific tasks that need to be completed for this project.",
        ];

        for content in test_cases {
            let result = validate_specification_content(content, "test.md");
            assert!(result.is_ok(), "Failed for content: {content}");
        }
    }

    #[test]
    fn test_validate_plan_file_with_content_validation() {
        let mock_fs = Arc::new(MockFileSystem::new());

        // Test insufficient content
        let path = Path::new("short.md");
        let short_content = "# Short";
        mock_fs
            .write(path, short_content)
            .expect("Failed to write short file");

        let config = PlanValidationConfig::default();
        let result = validate_plan_file_with_fs("short.md", &config, mock_fs.as_ref());

        assert!(result.is_err());
        match result.unwrap_err() {
            PlanCommandError::InsufficientContent { path, length } => {
                assert_eq!(path, "short.md");
                assert_eq!(length, 7);
            }
            _ => panic!("Expected InsufficientContent error"),
        }
    }

    #[test]
    fn test_validate_plan_file_content_validation_integration() {
        let mock_fs = Arc::new(MockFileSystem::new());

        // Test a valid specification file
        let path = Path::new("good-spec.md");
        let content = "# Feature Implementation Plan\n\n## Overview\n\nThis specification outlines the implementation of a new user authentication feature.\n\n## Requirements\n\n1. Users must be able to register with email and password\n2. System should support password reset functionality\n3. Integration with existing user database is required\n\n## Implementation Details\n\nThe implementation will follow these steps...";

        mock_fs
            .write(path, content)
            .expect("Failed to write good specification file");

        let config = PlanValidationConfig::default();
        let result = validate_plan_file_with_fs("good-spec.md", &config, mock_fs.as_ref());

        assert!(result.is_ok());
        let validated_file = result.unwrap();
        assert_eq!(validated_file.content, content);
        assert_eq!(validated_file.size, content.len() as u64);
    }
}
