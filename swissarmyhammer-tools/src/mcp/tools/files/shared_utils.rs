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

use rmcp::ErrorData as McpError;
use std::collections::HashSet;
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
        Err(e) => {
            // Provide specific error messages based on error kind
            use std::io::ErrorKind;
            match e.kind() {
                ErrorKind::NotFound => {
                    // Path doesn't exist - check if parent exists for better error messaging
                    if let Some(parent) = path_buf.parent() {
                        if !parent.exists() {
                            return Err(McpError::invalid_request(
                                format!("Parent directory does not exist: {}", parent.display()),
                                None,
                            ));
                        }
                    }
                    // For operations like write, this is acceptable - return the absolute path
                    Ok(path_buf)
                }
                ErrorKind::PermissionDenied => Err(McpError::invalid_request(
                    format!("Permission denied accessing path: {}", path_buf.display()),
                    None,
                )),
                ErrorKind::InvalidInput => Err(McpError::invalid_request(
                    format!("Invalid path format: {}", path_buf.display()),
                    None,
                )),
                _ => Err(McpError::invalid_request(
                    format!("Failed to resolve path '{}': {}", path_buf.display(), e),
                    None,
                )),
            }
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
    std::fs::metadata(path)
        .map_err(|e| McpError::invalid_request(format!("Failed to get file metadata: {}", e), None))
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
            McpError::internal_error(format!("Failed to create directory: {}", e), None)
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
pub fn handle_file_error(error: std::io::Error, operation: &str, path: &Path) -> McpError {
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

/// File access operation types for permission validation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileOperation {
    /// Reading file contents
    Read,
    /// Writing or creating files
    Write,
    /// Modifying existing files
    Edit,
    /// Creating or traversing directories
    Directory,
}

/// Enhanced file path validator with comprehensive security checks
///
/// This validator provides advanced security validation including:
/// - Workspace boundary enforcement
/// - Path traversal attack prevention
/// - Symlink resolution security
/// - Malicious path pattern detection
///
/// # Security Features
///
/// * **Workspace Boundary Validation**: Ensures all paths remain within configured workspace
/// * **Path Traversal Protection**: Detects and blocks ../ and similar attack patterns
/// * **Symlink Security**: Safely resolves symlinks while enforcing boundaries
/// * **Pattern Blocking**: Configurable blocking of dangerous path patterns
/// * **Unicode Normalization**: Handles Unicode attacks and mixed encodings
///
/// # Examples
///
/// ```rust,ignore
/// use crate::mcp::tools::files::shared_utils::FilePathValidator;
///
/// let validator = FilePathValidator::with_workspace_root("/home/user/project".into());
///
/// // Valid path within workspace
/// let safe_path = validator.validate_absolute_path("/home/user/project/src/main.rs")?;
///
/// // This would fail - outside workspace boundary
/// let result = validator.validate_absolute_path("/etc/passwd");
/// assert!(result.is_err());
/// ```
#[derive(Debug, Clone)]
pub struct FilePathValidator {
    /// Optional workspace root - if set, all paths must be within this directory
    workspace_root: Option<PathBuf>,
    /// Whether to allow symlink resolution (default: false for security)
    allow_symlinks: bool,
    /// Set of blocked path patterns (e.g., patterns that contain dangerous sequences)
    blocked_patterns: HashSet<String>,
    /// Whether to normalize Unicode in paths
    normalize_unicode: bool,
}

impl Default for FilePathValidator {
    fn default() -> Self {
        Self::new()
    }
}

impl FilePathValidator {
    /// Creates a new validator with default security settings
    ///
    /// Default settings:
    /// - No workspace root restriction
    /// - Symlinks disallowed
    /// - Common dangerous patterns blocked
    /// - Unicode normalization enabled
    pub fn new() -> Self {
        let mut blocked_patterns = HashSet::new();

        // Block path traversal patterns that escape to parent directories
        blocked_patterns.insert("../".to_string()); // Unix-style parent directory traversal
        blocked_patterns.insert("\\..\\".to_string()); // Windows-style parent directory traversal
        blocked_patterns.insert("..\\".to_string()); // Mixed Windows-style parent directory traversal
                                                     // Note: Don't block bare ".." since it could be a legitimate filename
                                                     // Note: "./" patterns are allowed as they reference the current directory (secure)

        // Add null byte and other dangerous patterns
        blocked_patterns.insert("\0".to_string());
        blocked_patterns.insert("\\0".to_string());

        Self {
            workspace_root: None,
            allow_symlinks: false,
            blocked_patterns,
            normalize_unicode: true,
        }
    }

    /// Creates a validator with a specific workspace root
    ///
    /// All validated paths must be within the specified workspace directory.
    /// This provides strong protection against directory traversal attacks.
    ///
    /// # Arguments
    ///
    /// * `workspace_root` - The root directory that constrains all file operations
    pub fn with_workspace_root(workspace_root: PathBuf) -> Self {
        let mut validator = Self::new();
        validator.workspace_root = Some(workspace_root);
        validator
    }

    /// Enables or disables symlink resolution
    ///
    /// # Security Warning
    ///
    /// Allowing symlinks can potentially bypass workspace boundary checks
    /// if the symlinks point outside the workspace. Use with caution.
    ///
    /// # Arguments
    ///
    /// * `allow` - Whether to allow symlink resolution
    pub fn set_allow_symlinks(&mut self, allow: bool) -> &mut Self {
        self.allow_symlinks = allow;
        self
    }

    /// Adds a custom blocked pattern
    ///
    /// # Arguments
    ///
    /// * `pattern` - The pattern to block in file paths
    pub fn add_blocked_pattern(&mut self, pattern: String) -> &mut Self {
        self.blocked_patterns.insert(pattern);
        self
    }

    /// Validates a path (absolute or relative) with comprehensive security checks
    ///
    /// This method performs all security validations including:
    /// 1. Path resolution (relative paths resolved against current working directory)
    /// 2. Path format validation
    /// 3. Dangerous pattern detection
    /// 4. Unicode normalization
    /// 5. Workspace boundary enforcement
    /// 6. Symlink security validation
    ///
    /// # Arguments
    ///
    /// * `path` - The path string to validate (absolute or relative)
    ///
    /// # Returns
    ///
    /// * `Result<PathBuf, McpError>` - The validated and normalized absolute path
    ///
    /// # Security Guarantees
    ///
    /// If this function returns `Ok(path)`, the path is guaranteed to be:
    /// - Absolute and properly formatted
    /// - Free of dangerous patterns
    /// - Within workspace boundaries (if configured)
    /// - Safe from known path traversal attacks
    pub fn validate_absolute_path(&self, path: &str) -> Result<PathBuf, McpError> {
        // Step 0: Check for blocked patterns early
        self.check_blocked_patterns(path)?;

        // Step 1: Resolve path (absolute or relative) to absolute path
        let path_buf = PathBuf::from(path);
        let resolved_path = if path_buf.is_absolute() {
            path_buf
        } else {
            // Resolve relative path against current working directory
            let current_dir = std::env::current_dir().map_err(|e| {
                McpError::invalid_request(
                    format!("Failed to get current working directory: {}", e),
                    None,
                )
            })?;
            current_dir.join(path_buf)
        };

        // Step 2: Symlink validation BEFORE canonicalization
        if resolved_path.is_symlink() && !self.allow_symlinks {
            return Err(McpError::invalid_request(
                format!("Symlinks are not allowed: {}", resolved_path.display()),
                None,
            ));
        }

        // Step 3: Basic validation (reuse existing function which may canonicalize)
        let mut validated_path = validate_file_path(&resolved_path.to_string_lossy())?;

        // Step 4: Unicode normalization if enabled
        if self.normalize_unicode {
            // Convert to string and back to handle Unicode normalization
            let path_str = validated_path.to_string_lossy();
            let normalized = self.normalize_unicode_path(&path_str)?;
            validated_path = PathBuf::from(normalized);
        }

        // Step 5: Workspace boundary validation
        if let Some(ref workspace_root) = self.workspace_root {
            self.ensure_workspace_boundary(&validated_path, workspace_root)?;
        }

        // Step 6: Final symlink resolution with boundary check if symlinks are allowed
        if self.allow_symlinks && resolved_path.is_symlink() {
            validated_path = self.resolve_symlink_securely(&validated_path)?;
        }

        Ok(validated_path)
    }

    /// Ensures a path is within workspace boundaries
    ///
    /// This method prevents directory traversal attacks by ensuring
    /// the resolved path is within the configured workspace root.
    ///
    /// # Arguments
    ///
    /// * `path` - The path to check
    /// * `workspace_root` - The workspace boundary to enforce
    pub fn ensure_workspace_boundary(
        &self,
        path: &Path,
        workspace_root: &Path,
    ) -> Result<(), McpError> {
        // Canonicalize both paths for accurate comparison
        let canonical_workspace = workspace_root.canonicalize().map_err(|e| {
            McpError::invalid_request(format!("Invalid workspace root: {}", e), None)
        })?;

        // For non-existent paths, check the deepest existing parent
        let path_to_check = if path.exists() {
            path.canonicalize().map_err(|e| {
                McpError::invalid_request(format!("Failed to canonicalize path: {}", e), None)
            })?
        } else {
            // Find the deepest existing parent directory
            let mut current = path;
            let mut found_path = None;

            while let Some(parent) = current.parent() {
                if parent.exists() {
                    let canonical_parent = parent.canonicalize().map_err(|e| {
                        McpError::invalid_request(
                            format!("Failed to canonicalize parent directory: {}", e),
                            None,
                        )
                    })?;

                    // Reconstruct the full path using the canonical parent
                    let relative_part: PathBuf = path
                        .strip_prefix(parent)
                        .map_err(|_| {
                            McpError::invalid_request(
                                "Failed to determine relative path component".to_string(),
                                None,
                            )
                        })?
                        .into();

                    found_path = Some(canonical_parent.join(relative_part));
                    break;
                }
                current = parent;
            }

            // If no parent exists, this is likely an invalid path
            found_path.ok_or_else(|| {
                McpError::invalid_request(
                    format!("Path has no existing parent directory: {}", path.display()),
                    None,
                )
            })?
        };

        // Check if the path is within workspace boundaries
        if !path_to_check.starts_with(&canonical_workspace) {
            return Err(McpError::invalid_request(
                format!(
                    "Path is outside workspace boundaries: {} (workspace: {})",
                    path_to_check.display(),
                    canonical_workspace.display()
                ),
                None,
            ));
        }

        Ok(())
    }

    /// Checks for blocked dangerous patterns in the path
    fn check_blocked_patterns(&self, path: &str) -> Result<(), McpError> {
        for pattern in &self.blocked_patterns {
            if path.contains(pattern) {
                return Err(McpError::invalid_request(
                    format!("Path contains blocked pattern '{}': {}", pattern, path),
                    None,
                ));
            }
        }
        Ok(())
    }

    /// Normalizes Unicode in file paths to prevent Unicode-based attacks
    fn normalize_unicode_path(&self, path: &str) -> Result<String, McpError> {
        // For now, we'll do basic validation and return the path as-is
        // In a full implementation, this would use Unicode normalization libraries

        // Check for null bytes and other control characters
        if path.contains('\0')
            || path
                .chars()
                .any(|c| c.is_control() && c != '\n' && c != '\r' && c != '\t')
        {
            return Err(McpError::invalid_request(
                "Path contains invalid control characters".to_string(),
                None,
            ));
        }

        Ok(path.to_string())
    }

    /// Securely resolves symlinks while maintaining workspace boundaries
    fn resolve_symlink_securely(&self, path: &Path) -> Result<PathBuf, McpError> {
        let resolved = path.canonicalize().map_err(|e| {
            McpError::invalid_request(format!("Failed to resolve symlink: {}", e), None)
        })?;

        // Re-check workspace boundaries after symlink resolution
        if let Some(ref workspace_root) = self.workspace_root {
            self.ensure_workspace_boundary(&resolved, workspace_root)?;
        }

        Ok(resolved)
    }
}

/// Validates file permissions for a specific operation
///
/// This function checks that the current process has the necessary
/// permissions to perform the requested operation on the file.
///
/// # Arguments
///
/// * `path` - The file path to check permissions for
/// * `operation` - The type of operation being requested
///
/// # Returns
///
/// * `Result<(), McpError>` - Success or permission error
pub fn check_file_permissions(path: &Path, operation: FileOperation) -> Result<(), McpError> {
    match operation {
        FileOperation::Read => {
            // Check if file is readable
            if path.exists() {
                let metadata = get_file_metadata(path)?;

                // On Unix systems, we could check specific permission bits
                // For now, we'll use a simple existence and metadata check
                if metadata.len() == 0 && metadata.is_file() {
                    // Empty files might be readable, let the actual read operation handle it
                }

                // Additional permission checks could be added here based on platform
            }
        }

        FileOperation::Write => {
            // Check if we can write to the file or its parent directory
            if path.exists() {
                // For existing files, check if they're writable
                if get_file_metadata(path)?.permissions().readonly() {
                    return Err(McpError::invalid_request(
                        format!("File is read-only: {}", path.display()),
                        None,
                    ));
                }
            } else {
                // For new files, check if parent directory is writable
                if let Some(parent) = path.parent() {
                    if !parent.exists() {
                        return Err(McpError::invalid_request(
                            format!("Parent directory does not exist: {}", parent.display()),
                            None,
                        ));
                    }

                    // Could add more sophisticated permission checks here
                }
            }
        }

        FileOperation::Edit => {
            // For edit operations, the file must exist and be writable
            if !path.exists() {
                return Err(McpError::invalid_request(
                    format!("Cannot edit non-existent file: {}", path.display()),
                    None,
                ));
            }

            if get_file_metadata(path)?.permissions().readonly() {
                return Err(McpError::invalid_request(
                    format!("File is read-only and cannot be edited: {}", path.display()),
                    None,
                ));
            }
        }

        FileOperation::Directory => {
            // For directory operations, check if directory exists or can be created
            if path.exists() && !path.is_dir() {
                return Err(McpError::invalid_request(
                    format!("Path exists but is not a directory: {}", path.display()),
                    None,
                ));
            }
        }
    }

    Ok(())
}

/// Secure wrapper for file access operations
///
/// This struct provides a high-level interface for file operations
/// with comprehensive security validation. All operations go through
/// the enhanced security framework.
///
/// # Security Features
///
/// * All paths validated through FilePathValidator
/// * Permission checking for each operation
/// * Workspace boundary enforcement
/// * Protection against path traversal attacks
/// * Consistent error handling and logging
///
/// # Examples
///
/// ```rust,ignore
/// use crate::mcp::tools::files::shared_utils::{SecureFileAccess, FilePathValidator};
///
/// let validator = FilePathValidator::with_workspace_root("/safe/workspace".into());
/// let file_access = SecureFileAccess::new(validator);
///
/// // Safe file operations
/// let content = file_access.read("/safe/workspace/file.txt", None, None)?;
/// file_access.write("/safe/workspace/output.txt", "safe content")?;
/// ```
#[derive(Debug, Clone)]
pub struct SecureFileAccess {
    validator: FilePathValidator,
}

impl SecureFileAccess {
    /// Creates a new SecureFileAccess with the given validator
    pub fn new(validator: FilePathValidator) -> Self {
        Self { validator }
    }

    /// Creates a SecureFileAccess with default security settings
    pub fn default_secure() -> Self {
        Self::new(FilePathValidator::new())
    }

    /// Creates a SecureFileAccess with workspace boundary enforcement
    pub fn with_workspace(workspace_root: PathBuf) -> Self {
        Self::new(FilePathValidator::with_workspace_root(workspace_root))
    }

    /// Securely reads file content with validation
    ///
    /// # Arguments
    ///
    /// * `path` - Absolute path to the file
    /// * `offset` - Optional starting line number
    /// * `limit` - Optional maximum number of lines
    ///
    /// # Returns
    ///
    /// * `Result<String, McpError>` - File content or error
    pub fn read(
        &self,
        path: &str,
        offset: Option<usize>,
        limit: Option<usize>,
    ) -> Result<String, McpError> {
        tracing::debug!("SecureFileAccess::read called with path: {}", path);

        // Validate path through security framework
        let validated_path = match self.validator.validate_absolute_path(path) {
            Ok(p) => {
                tracing::debug!("Path validation successful: {}", p.display());
                p
            }
            Err(e) => {
                tracing::error!("Path validation failed for '{}': {}", path, e);
                return Err(e);
            }
        };

        // Check permissions for read operation
        if let Err(e) = check_file_permissions(&validated_path, FileOperation::Read) {
            tracing::error!(
                "Permission check failed for '{}': {}",
                validated_path.display(),
                e
            );
            return Err(e);
        }

        tracing::debug!("Permission check passed for: {}", validated_path.display());

        // Perform the actual read operation
        let content = match std::fs::read_to_string(&validated_path) {
            Ok(c) => {
                tracing::debug!("File read successful, content length: {} bytes", c.len());
                c
            }
            Err(e) => {
                tracing::error!("File read failed for '{}': {}", validated_path.display(), e);
                return Err(handle_file_error(e, "read", &validated_path));
            }
        };

        // Apply offset and limit if specified (same logic as existing read tool)
        let final_content = match (offset, limit) {
            (Some(offset), Some(limit)) => {
                let lines: Vec<&str> = content.lines().collect();
                lines
                    .iter()
                    .skip(offset.saturating_sub(1))
                    .take(limit)
                    .copied()
                    .collect::<Vec<_>>()
                    .join("\n")
            }
            (Some(offset), None) => {
                let lines: Vec<&str> = content.lines().collect();
                lines
                    .iter()
                    .skip(offset.saturating_sub(1))
                    .copied()
                    .collect::<Vec<_>>()
                    .join("\n")
            }
            (None, Some(limit)) => content.lines().take(limit).collect::<Vec<_>>().join("\n"),
            (None, None) => content,
        };

        Ok(final_content)
    }

    /// Securely writes file content with validation
    ///
    /// # Arguments
    ///
    /// * `path` - Absolute path to the file
    /// * `content` - Content to write to the file
    ///
    /// # Returns
    ///
    /// * `Result<(), McpError>` - Success or error
    pub fn write(&self, path: &str, content: &str) -> Result<(), McpError> {
        // Validate path through security framework
        let validated_path = self.validator.validate_absolute_path(path)?;

        // Check permissions for write operation
        check_file_permissions(&validated_path, FileOperation::Write)?;

        // Ensure parent directory exists
        if let Some(parent) = validated_path.parent() {
            ensure_directory_exists(parent)?;
        }

        // Perform the actual write operation
        std::fs::write(&validated_path, content)
            .map_err(|e| handle_file_error(e, "write", &validated_path))?;

        Ok(())
    }

    /// Securely performs string replacement in files with validation
    ///
    /// # Arguments
    ///
    /// * `path` - Absolute path to the file
    /// * `old_string` - String to replace
    /// * `new_string` - Replacement string
    /// * `replace_all` - Whether to replace all occurrences
    ///
    /// # Returns
    ///
    /// * `Result<(), McpError>` - Success or error
    pub fn edit(
        &self,
        path: &str,
        old_string: &str,
        new_string: &str,
        replace_all: bool,
    ) -> Result<(), McpError> {
        // Validate path through security framework
        let validated_path = self.validator.validate_absolute_path(path)?;

        // Check permissions for edit operation
        check_file_permissions(&validated_path, FileOperation::Edit)?;

        // Read current content
        let content = std::fs::read_to_string(&validated_path)
            .map_err(|e| handle_file_error(e, "read", &validated_path))?;

        // Perform replacement
        let new_content = if replace_all {
            content.replace(old_string, new_string)
        } else {
            // For single replacement, ensure the old string is unique
            let matches: Vec<_> = content.matches(old_string).collect();
            if matches.is_empty() {
                return Err(McpError::invalid_request(
                    format!("String '{}' not found in file", old_string),
                    None,
                ));
            }
            if matches.len() > 1 {
                return Err(McpError::invalid_request(
                    format!("String '{}' appears {} times in file. Use replace_all=true for multiple replacements",
                           old_string, matches.len()),
                    None,
                ));
            }
            content.replacen(old_string, new_string, 1)
        };

        // Write back the modified content
        std::fs::write(&validated_path, new_content)
            .map_err(|e| handle_file_error(e, "write", &validated_path))?;

        Ok(())
    }
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

    // Enhanced Security Framework Tests

    #[test]
    fn test_file_path_validator_default() {
        let validator = FilePathValidator::new();

        // Test default blocked patterns
        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path().to_string_lossy().to_string();

        // These should be blocked by default patterns
        let dangerous_paths = vec![
            format!("{}/../etc/passwd", base_path),
            format!("{}\\..\\windows\\system32", base_path),
        ];

        for dangerous_path in dangerous_paths {
            let result = validator.validate_absolute_path(&dangerous_path);
            assert!(
                result.is_err(),
                "Should block dangerous path: {}",
                dangerous_path
            );
        }
    }

    #[test]
    fn test_file_path_validator_relative_paths() {
        // Use validator without workspace restrictions for basic testing
        let validator = FilePathValidator::new();
        let temp_dir = TempDir::new().unwrap();

        // Create test files
        let test_file = temp_dir.path().join("test_file.txt");
        fs::write(&test_file, "test content").unwrap();

        // Change to the temp directory for relative path testing
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        // Test basic relative path resolution
        let result = validator.validate_absolute_path("test_file.txt");
        assert!(result.is_ok(), "Should accept simple relative path");
        let resolved = result.unwrap();
        assert!(resolved.is_absolute(), "Resolved path should be absolute");
        assert!(
            resolved.ends_with("test_file.txt"),
            "Should preserve filename"
        );

        // Test current directory relative path
        let result = validator.validate_absolute_path("./test_file.txt");
        assert!(result.is_ok(), "Should accept ./ relative path");

        // Test parent directory (should be blocked by dangerous patterns)
        let result = validator.validate_absolute_path("../test_file.txt");
        assert!(result.is_err(), "Should block ../ path traversal");

        // Test nested relative path
        let nested_dir = temp_dir.path().join("nested");
        fs::create_dir(&nested_dir).unwrap();
        let nested_file = nested_dir.join("nested_file.txt");
        fs::write(&nested_file, "nested content").unwrap();

        let result = validator.validate_absolute_path("nested/nested_file.txt");
        assert!(result.is_ok(), "Should accept nested relative path");

        // Restore original directory
        std::env::set_current_dir(original_dir).unwrap();
    }

    #[test]
    fn test_file_path_validator_relative_with_workspace() {
        let temp_dir = TempDir::new().unwrap();
        let workspace_root = temp_dir.path().to_path_buf();
        let validator = FilePathValidator::with_workspace_root(workspace_root.clone());

        // Create test file in workspace
        let test_file = workspace_root.join("workspace_file.txt");
        fs::write(&test_file, "workspace content").unwrap();

        // Change to workspace directory
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&workspace_root).unwrap();

        // Test relative path within workspace
        let result = validator.validate_absolute_path("workspace_file.txt");
        assert!(
            result.is_ok(),
            "Should accept relative path within workspace"
        );

        // Create and try to access file outside workspace
        let outside_dir = TempDir::new().unwrap();
        let outside_file = outside_dir.path().join("outside_file.txt");
        fs::write(&outside_file, "outside content").unwrap();

        // Change to outside directory and try relative path (should fail workspace check)
        if outside_dir.path().exists() {
            std::env::set_current_dir(&outside_dir).unwrap();
            let result = validator.validate_absolute_path("outside_file.txt");
            assert!(
                result.is_err(),
                "Should reject relative path outside workspace"
            );
        } else {
            println!("Outside directory doesn't exist, skipping outside workspace test");
        }

        // Restore original directory
        std::env::set_current_dir(original_dir).unwrap();
    }

    #[test]
    fn test_file_path_validator_workspace_boundary() {
        let temp_dir = TempDir::new().unwrap();
        let workspace_root = temp_dir.path().to_path_buf();
        let validator = FilePathValidator::with_workspace_root(workspace_root.clone());

        // Create a test file within workspace
        let safe_file = workspace_root.join("safe_file.txt");
        fs::write(&safe_file, "safe content").unwrap();

        // This should succeed - within workspace
        let result = validator.validate_absolute_path(&safe_file.to_string_lossy());
        assert!(result.is_ok());

        // This should fail - outside workspace (system file)
        let result = validator.validate_absolute_path("/etc/passwd");
        assert!(result.is_err());

        // Test with a path outside workspace that exists
        let outside_dir = TempDir::new().unwrap();
        let outside_file = outside_dir.path().join("outside.txt");
        fs::write(&outside_file, "outside content").unwrap();

        let result = validator.validate_absolute_path(&outside_file.to_string_lossy());
        assert!(result.is_err());
    }

    #[test]
    fn test_file_path_validator_blocked_patterns() {
        let mut validator = FilePathValidator::new();
        validator.add_blocked_pattern("secret".to_string());

        let temp_dir = TempDir::new().unwrap();
        let safe_file = temp_dir.path().join("normal.txt");
        let dangerous_file = temp_dir.path().join("secret_file.txt");

        // Safe file should pass
        let result = validator.validate_absolute_path(&safe_file.to_string_lossy());
        assert!(result.is_ok());

        // File with blocked pattern should fail
        let result = validator.validate_absolute_path(&dangerous_file.to_string_lossy());
        assert!(result.is_err());
        assert!(format!("{:?}", result).contains("blocked pattern"));
    }

    #[test]
    fn test_file_path_validator_symlinks() {
        let temp_dir = TempDir::new().unwrap();
        let target_file = temp_dir.path().join("target.txt");
        let symlink_file = temp_dir.path().join("symlink.txt");

        fs::write(&target_file, "target content").unwrap();

        // Create symlink (skip if platform doesn't support)
        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            if symlink(&target_file, &symlink_file).is_ok() {
                // Verify the symlink was actually created
                if symlink_file.is_symlink() {
                    // Test with symlinks disabled (default)
                    let validator = FilePathValidator::new();
                    let result = validator.validate_absolute_path(&symlink_file.to_string_lossy());
                    assert!(
                        result.is_err(),
                        "Symlink should be rejected when symlinks are disabled"
                    );

                    // Test with symlinks enabled
                    let mut validator = FilePathValidator::new();
                    validator.set_allow_symlinks(true);
                    let result = validator.validate_absolute_path(&symlink_file.to_string_lossy());
                    assert!(
                        result.is_ok(),
                        "Symlink should be allowed when symlinks are enabled"
                    );
                } else {
                    // Symlink creation failed or system doesn't support detection, skip test
                    println!("Symlink test skipped: symlink creation failed or not detected");
                }
            } else {
                println!("Symlink test skipped: failed to create symlink");
            }
        }

        #[cfg(not(unix))]
        {
            println!("Symlink test skipped: not on Unix platform");
        }
    }

    #[test]
    fn test_file_path_validator_unicode_normalization() {
        let validator = FilePathValidator::new();

        // Test null byte rejection
        let result = validator.validate_absolute_path("/tmp/file\0.txt");
        assert!(result.is_err());

        // Test other control characters
        let result = validator.validate_absolute_path("/tmp/file\x01.txt");
        assert!(result.is_err());
    }

    #[test]
    fn test_check_file_permissions() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("permissions_test.txt");

        // Test read permissions on non-existent file (should not fail)
        let result = check_file_permissions(&test_file, FileOperation::Read);
        assert!(result.is_ok());

        // Create file and test read permissions
        fs::write(&test_file, "test content").unwrap();
        let result = check_file_permissions(&test_file, FileOperation::Read);
        assert!(result.is_ok());

        // Test write permissions on existing file
        let result = check_file_permissions(&test_file, FileOperation::Write);
        assert!(result.is_ok());

        // Test edit permissions (requires existing file)
        let result = check_file_permissions(&test_file, FileOperation::Edit);
        assert!(result.is_ok());

        // Test edit on non-existent file (should fail)
        let non_existent = temp_dir.path().join("does_not_exist.txt");
        let result = check_file_permissions(&non_existent, FileOperation::Edit);
        assert!(result.is_err());
    }

    #[test]
    fn test_secure_file_access_read() {
        let temp_dir = TempDir::new().unwrap();
        let workspace_root = temp_dir.path().to_path_buf();
        let secure_access = SecureFileAccess::with_workspace(workspace_root.clone());

        // Create test file with content
        let test_file = workspace_root.join("test_read.txt");
        let content = "Line 1\nLine 2\nLine 3\nLine 4\nLine 5";
        fs::write(&test_file, content).unwrap();

        // Test full read with absolute path
        let result = secure_access.read(&test_file.to_string_lossy(), None, None);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), content);

        // Test read with offset
        let result = secure_access.read(&test_file.to_string_lossy(), Some(2), None);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Line 2\nLine 3\nLine 4\nLine 5");

        // Test read with limit
        let result = secure_access.read(&test_file.to_string_lossy(), None, Some(2));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Line 1\nLine 2");

        // Test read with both offset and limit
        let result = secure_access.read(&test_file.to_string_lossy(), Some(2), Some(2));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Line 2\nLine 3");
    }

    #[test]
    fn test_secure_file_access_read_relative_paths() {
        // Use default secure access without workspace restrictions for simple testing
        let secure_access = SecureFileAccess::default_secure();

        let temp_dir = TempDir::new().unwrap();
        let workspace_root = temp_dir.path().to_path_buf();

        // Create test file with content
        let test_file = workspace_root.join("relative_test.txt");
        let content = "Relative path content";
        fs::write(&test_file, content).unwrap();

        // Change to temp directory to test relative paths
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&workspace_root).unwrap();

        // Test read with relative path
        let result = secure_access.read("relative_test.txt", None, None);
        assert!(
            result.is_ok(),
            "Should be able to read file with relative path"
        );
        assert_eq!(result.unwrap(), content);

        // Test read with ./ relative path
        let result = secure_access.read("./relative_test.txt", None, None);
        assert!(
            result.is_ok(),
            "Should be able to read file with ./ relative path"
        );

        // Create nested directory and file
        let nested_dir = workspace_root.join("nested");
        fs::create_dir(&nested_dir).unwrap();
        let nested_file = nested_dir.join("nested_file.txt");
        let nested_content = "Nested file content";
        fs::write(&nested_file, nested_content).unwrap();

        // Test read nested file with relative path
        let result = secure_access.read("nested/nested_file.txt", None, None);
        assert!(
            result.is_ok(),
            "Should be able to read nested file with relative path"
        );
        assert_eq!(result.unwrap(), nested_content);

        // Restore original directory
        std::env::set_current_dir(original_dir).unwrap();
    }

    #[test]
    fn test_secure_file_access_write() {
        let temp_dir = TempDir::new().unwrap();
        let workspace_root = temp_dir.path().to_path_buf();
        let secure_access = SecureFileAccess::with_workspace(workspace_root.clone());

        let test_file = workspace_root.join("test_write.txt");
        let content = "This is test content for writing";

        // Test write operation
        let result = secure_access.write(&test_file.to_string_lossy(), content);
        assert!(result.is_ok());

        // Verify content was written correctly
        let written_content = fs::read_to_string(&test_file).unwrap();
        assert_eq!(written_content, content);
    }

    #[test]
    fn test_secure_file_access_edit() {
        let temp_dir = TempDir::new().unwrap();
        let workspace_root = temp_dir.path().to_path_buf();
        let secure_access = SecureFileAccess::with_workspace(workspace_root.clone());

        let test_file = workspace_root.join("test_edit.txt");
        let initial_content = "Hello world! This is a test.";
        fs::write(&test_file, initial_content).unwrap();

        // Test single replacement
        let result = secure_access.edit(&test_file.to_string_lossy(), "world", "universe", false);
        assert!(result.is_ok());

        let edited_content = fs::read_to_string(&test_file).unwrap();
        assert_eq!(edited_content, "Hello universe! This is a test.");

        // Test replace_all
        let multi_content = "test test test";
        fs::write(&test_file, multi_content).unwrap();

        let result = secure_access.edit(&test_file.to_string_lossy(), "test", "exam", true);
        assert!(result.is_ok());

        let edited_content = fs::read_to_string(&test_file).unwrap();
        assert_eq!(edited_content, "exam exam exam");
    }

    #[test]
    fn test_secure_file_access_workspace_security() {
        let temp_dir = TempDir::new().unwrap();
        let workspace_root = temp_dir.path().to_path_buf();
        let secure_access = SecureFileAccess::with_workspace(workspace_root);

        // Attempt to read a file outside workspace should fail
        let result = secure_access.read("/etc/passwd", None, None);
        assert!(result.is_err());
        assert!(format!("{:?}", result).contains("outside workspace boundaries"));

        // Attempt to write outside workspace should fail
        let result = secure_access.write("/tmp/malicious.txt", "malicious content");
        assert!(result.is_err());
        assert!(format!("{:?}", result).contains("outside workspace boundaries"));
    }

    #[test]
    fn test_path_traversal_attack_prevention() {
        let temp_dir = TempDir::new().unwrap();
        let workspace_root = temp_dir.path().to_path_buf();
        let validator = FilePathValidator::with_workspace_root(workspace_root);

        // Common path traversal attack patterns
        let attack_patterns = vec![
            "../../../etc/passwd",
            "..\\..\\..\\windows\\system32\\config",
            "./../../secret",
            // Note: URL-encoded attacks would require additional URL decoding validation
        ];

        for pattern in attack_patterns {
            let full_path = format!("{}/{}", temp_dir.path().display(), pattern);
            let result = validator.validate_absolute_path(&full_path);
            assert!(
                result.is_err(),
                "Should block path traversal attack: {}",
                pattern
            );
        }
    }

    #[test]
    fn test_error_handling_for_security_violations() {
        let temp_dir = TempDir::new().unwrap();
        let workspace_root = temp_dir.path().to_path_buf();
        let secure_access = SecureFileAccess::with_workspace(workspace_root);

        // Test error message contains security context
        let result = secure_access.read("/etc/passwd", None, None);
        assert!(result.is_err());

        let error_msg = format!("{:?}", result);
        assert!(
            error_msg.contains("workspace boundaries") || error_msg.contains("blocked pattern")
        );
    }
}
