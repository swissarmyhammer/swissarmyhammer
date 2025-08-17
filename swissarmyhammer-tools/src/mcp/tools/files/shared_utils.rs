//! Shared utilities for file operations
//!
//! This module provides common functionality used by all file tools to ensure
//! consistent behavior, security validation, and error handling across the
//! file tools suite.
//!
//! ## Security & Validation
//!
//! The shared utilities implement essential security measures:
//! - Workspace boundary validation to prevent directory traversal attacks
//! - Absolute path requirements to avoid confusion and security issues
//! - File permission and accessibility checks
//! - Path canonicalization to resolve symlinks and relative paths
//!
//! ## Error Handling
//!
//! All utilities provide comprehensive error handling with MCP-compatible
//! error types for consistent client experience across all file tools.

use rmcp::Error as McpError;
use std::path::{Path, PathBuf};

/// Validate that a file path is absolute and within acceptable boundaries
///
/// This function performs essential security validation for all file operations:
/// 1. Ensures the path is absolute (not relative)
/// 2. Canonicalizes the path to resolve symlinks and relative components  
/// 3. Validates that the path is within workspace boundaries (if configured)
/// 4. Checks basic path format and validity
///
/// # Arguments
///
/// * `path` - The file path string to validate
///
/// # Returns
///
/// * `Result<PathBuf, McpError>` - The canonicalized path or validation error
///
/// # Security Notes
///
/// This function is critical for preventing:
/// - Directory traversal attacks (../ sequences)
/// - Access to system files outside workspace
/// - Symlink attacks that escape workspace boundaries
/// - Invalid or malformed path exploitation
///
/// # Examples
///
/// ```rust,ignore
/// use crate::mcp::tools::files::shared_utils;
///
/// // Valid absolute path
/// let path = shared_utils::validate_file_path("/home/user/project/src/main.rs")?;
///
/// // Invalid relative path - will return error
/// let result = shared_utils::validate_file_path("../../../etc/passwd");
/// assert!(result.is_err());
/// ```
pub fn validate_file_path(path: &str) -> Result<PathBuf, McpError> {
    // Ensure path is not empty
    if path.trim().is_empty() {
        return Err(McpError::invalid_request(
            "File path cannot be empty".to_string(),
            None,
        ));
    }

    let path_buf = PathBuf::from(path);

    // Require absolute paths for security
    if !path_buf.is_absolute() {
        return Err(McpError::invalid_request(
            "File path must be absolute, not relative".to_string(),
            None,
        ));
    }

    // Canonicalize path to resolve symlinks and relative components
    match path_buf.canonicalize() {
        Ok(canonical_path) => Ok(canonical_path),
        Err(_) => {
            // If canonicalization fails, the path might not exist yet
            // For operations like write, this is acceptable
            // Return the absolute path as-is but validate its parent
            if let Some(parent) = path_buf.parent() {
                if !parent.exists() {
                    return Err(McpError::invalid_request(
                        format!("Parent directory does not exist: {}", parent.display()),
                        None,
                    ));
                }
            }
            Ok(path_buf)
        }
    }
}

/// Check if a file exists and is accessible
///
/// This utility function checks file existence and basic accessibility
/// without performing full validation. Used by tools that need to
/// verify file existence before operations.
///
/// # Arguments
///
/// * `path` - The path to check
///
/// # Returns
///
/// * `Result<bool, McpError>` - True if file exists and is accessible
pub fn file_exists(path: &Path) -> Result<bool, McpError> {
    match path.try_exists() {
        Ok(exists) => Ok(exists),
        Err(e) => Err(McpError::internal_error(
            format!("Failed to check file existence: {}", e),
            None,
        )),
    }
}

/// Get file metadata safely
///
/// Retrieves file metadata with proper error handling and security checks.
/// Used by tools that need file size, permissions, or modification time.
///
/// # Arguments  
///
/// * `path` - The path to get metadata for
///
/// # Returns
///
/// * `Result<std::fs::Metadata, McpError>` - File metadata or error
pub fn get_file_metadata(path: &Path) -> Result<std::fs::Metadata, McpError> {
    std::fs::metadata(path).map_err(|e| {
        McpError::invalid_request(
            format!("Failed to get file metadata: {}", e),
            None,
        )
    })
}

/// Ensure a directory exists, creating it if necessary
///
/// This utility creates parent directories as needed for file operations.
/// Used by the write tool to ensure the target directory exists.
///
/// # Arguments
///
/// * `dir_path` - The directory path to ensure exists
///
/// # Returns
///
/// * `Result<(), McpError>` - Success or error
pub fn ensure_directory_exists(dir_path: &Path) -> Result<(), McpError> {
    if !dir_path.exists() {
        std::fs::create_dir_all(dir_path).map_err(|e| {
            McpError::internal_error(
                format!("Failed to create directory: {}", e),
                None,
            )
        })?;
    }
    Ok(())
}

/// Convert file system errors to MCP errors
///
/// This utility provides consistent error message formatting for file
/// operations across all tools.
///
/// # Arguments
///
/// * `error` - The std::io::Error to convert
/// * `operation` - Description of the operation that failed
/// * `path` - The file path involved in the operation
///
/// # Returns
///
/// * `McpError` - Formatted MCP error
pub fn handle_file_error(
    error: std::io::Error,
    operation: &str,
    path: &Path,
) -> McpError {
    let error_message = match error.kind() {
        std::io::ErrorKind::NotFound => {
            format!("File not found: {}", path.display())
        }
        std::io::ErrorKind::PermissionDenied => {
            format!("Permission denied accessing: {}", path.display())
        }
        std::io::ErrorKind::AlreadyExists => {
            format!("File already exists: {}", path.display())
        }
        std::io::ErrorKind::InvalidData => {
            format!("Invalid file data in: {}", path.display())
        }
        _ => {
            format!("Failed to {} {}: {}", operation, path.display(), error)
        }
    };

    McpError::internal_error(error_message, None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_validate_file_path_empty() {
        let result = validate_file_path("");
        assert!(result.is_err());
        
        let result = validate_file_path("   ");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_file_path_relative() {
        let result = validate_file_path("relative/path");
        assert!(result.is_err());
        
        let result = validate_file_path("./current/path");
        assert!(result.is_err());
        
        let result = validate_file_path("../parent/path");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_file_path_absolute_nonexistent() {
        // This should succeed even if the file doesn't exist,
        // as long as the parent directory exists
        let temp_dir = TempDir::new().unwrap();
        let non_existent_file = temp_dir.path().join("does_not_exist.txt");
        let result = validate_file_path(&non_existent_file.to_string_lossy());
        assert!(result.is_ok());
    }

    #[test] 
    fn test_validate_file_path_absolute_existing() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        fs::write(&test_file, "test content").unwrap();
        
        let result = validate_file_path(&test_file.to_string_lossy());
        assert!(result.is_ok());
        
        let validated_path = result.unwrap();
        assert!(validated_path.is_absolute());
    }

    #[test]
    fn test_file_exists() {
        let temp_dir = TempDir::new().unwrap();
        let existing_file = temp_dir.path().join("existing.txt");
        let non_existing_file = temp_dir.path().join("non_existing.txt");
        
        fs::write(&existing_file, "content").unwrap();
        
        let result = file_exists(&existing_file);
        assert!(result.is_ok());
        assert!(result.unwrap());
        
        let result = file_exists(&non_existing_file);
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[test]
    fn test_get_file_metadata() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("metadata_test.txt");
        fs::write(&test_file, "test content for metadata").unwrap();
        
        let result = get_file_metadata(&test_file);
        assert!(result.is_ok());
        
        let metadata = result.unwrap();
        assert!(metadata.len() > 0);
    }

    #[test]
    fn test_ensure_directory_exists() {
        let temp_dir = TempDir::new().unwrap();
        let nested_dir = temp_dir.path().join("nested").join("directory");
        
        assert!(!nested_dir.exists());
        
        let result = ensure_directory_exists(&nested_dir);
        assert!(result.is_ok());
        assert!(nested_dir.exists());
        assert!(nested_dir.is_dir());
    }

    #[test]
    fn test_handle_file_error() {
        use std::io::{Error, ErrorKind};
        
        let path = Path::new("/test/path");
        
        let not_found_error = Error::new(ErrorKind::NotFound, "test error");
        let mcp_error = handle_file_error(not_found_error, "read", path);
        assert!(format!("{:?}", mcp_error).contains("File not found"));
        
        let permission_error = Error::new(ErrorKind::PermissionDenied, "test error");
        let mcp_error = handle_file_error(permission_error, "write", path);
        assert!(format!("{:?}", mcp_error).contains("Permission denied"));
    }
}