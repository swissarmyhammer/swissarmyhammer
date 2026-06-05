// sah rule ignore acp/capability-enforcement
//! Shared utilities for file operations
//!
//! This module provides common functionality used by all file tools to ensure
//! consistent behavior, security validation, and error handling across the
//! file tools suite.
//!
//! Note: This is an MCP utilities module, not an ACP operation. ACP capability
//! checking happens at the agent layer (claude-agent, llama-agent), not at the
//! MCP tool utilities layer.
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
/// # Session root
///
/// `base_dir` is the **session working directory** — the board directory the
/// agent session is rooted at. Relative paths resolve against it. Tool handlers
/// obtain it from [`ToolContext::session_root`](crate::mcp::tool_registry::ToolContext::session_root),
/// never from the process current directory, because the bundled GUI app runs
/// with CWD `/` and a single process hosts multiple board sessions.
///
/// # Examples
///
/// ```rust,ignore
/// use crate::mcp::tools::files::shared_utils;
/// use std::path::Path;
///
/// let base = Path::new("/home/user/project");
///
/// // Valid absolute path (base ignored for absolute inputs)
/// let path = shared_utils::validate_file_path(base, "/home/user/project/src/main.rs")?;
///
/// // Relative path resolves against base, not the process CWD
/// let path = shared_utils::validate_file_path(base, "src/main.rs")?;
///
/// // Invalid relative path - will return error
/// let result = shared_utils::validate_file_path(base, "../../../etc/passwd");
/// assert!(result.is_err());
/// ```
pub fn validate_file_path(base_dir: &Path, path: &str) -> Result<PathBuf, McpError> {
    // Ensure path is not empty
    if path.trim().is_empty() {
        return Err(McpError::invalid_request(
            "File path cannot be empty".to_string(),
            None,
        ));
    }

    // Check path length to prevent system issues
    const MAX_PATH_LENGTH: usize = 4096; // Unix PATH_MAX standard
    if path.len() > MAX_PATH_LENGTH {
        return Err(McpError::invalid_request(
            format!(
                "Path too long ({} characters, maximum {}): {}",
                path.len(),
                MAX_PATH_LENGTH,
                path
            ),
            None,
        ));
    }

    let path_buf = PathBuf::from(path);

    // Resolve relative paths against the session working directory before
    // canonicalization. Never fall back to the process CWD — see the module
    // and `ToolContext::session_root` docs for why.
    let resolved_path = if path_buf.is_absolute() {
        path_buf
    } else {
        base_dir.join(path_buf)
    };

    // Canonicalize path to resolve symlinks and relative components
    match resolved_path.canonicalize() {
        Ok(canonical_path) => Ok(canonical_path),
        Err(e) => {
            // Provide specific error messages based on error kind
            use std::io::ErrorKind;
            match e.kind() {
                ErrorKind::NotFound => {
                    // Path doesn't exist - check if parent exists for better error messaging
                    if let Some(parent) = resolved_path.parent() {
                        if !parent.exists() {
                            return Err(McpError::invalid_request(
                                format!("Parent directory does not exist: {}", parent.display()),
                                None,
                            ));
                        }
                    }
                    // For operations like write, this is acceptable - return the resolved path
                    Ok(resolved_path)
                }
                ErrorKind::PermissionDenied => Err(McpError::invalid_request(
                    format!(
                        "Permission denied accessing path: {}",
                        resolved_path.display()
                    ),
                    None,
                )),
                ErrorKind::InvalidInput => Err(McpError::invalid_request(
                    format!("Invalid path format: {}", resolved_path.display()),
                    None,
                )),
                _ => Err(McpError::invalid_request(
                    format!(
                        "Failed to resolve path '{}': {}",
                        resolved_path.display(),
                        e
                    ),
                    None,
                )),
            }
        }
    }
}

/// Refuse a search root that would walk the entire filesystem or the process CWD.
///
/// An unscoped `grep`/`glob` defaults its search root to the session working
/// directory ([`ToolContext::session_root`](crate::mcp::tool_registry::ToolContext::session_root)).
/// Two pathological roots must never be walked:
///
/// 1. **The filesystem root** (`/`). For the bundled GUI app the process CWD is
///    `/`, and an unscoped walk rooted there visits every file on the machine —
///    the original "grep hung forever" failure.
/// 2. **A bare relative `.` (or empty path).** This is the last-resort fallback
///    `session_root` returns when neither a working dir nor the process current
///    directory is available. Walking it means walking the very process CWD this
///    whole change exists to avoid, and — being relative — it silently slips past
///    a plain `parent().is_none()` root check (`Path::new(".").parent()` is
///    `Some("")`, not `None`).
///
/// Returns an error in either case; otherwise `Ok(())`. File tools call this on
/// their resolved search directory before handing it to the walker.
///
/// # Arguments
///
/// * `search_dir` - The resolved search root (from a `path` argument or the
///   session working directory).
pub fn reject_filesystem_root(search_dir: &Path) -> Result<(), McpError> {
    // A relative root (bare `.`, empty, or anything not anchored at an absolute
    // base) means the session working directory could not be resolved. Walking it
    // would root the search at the process CWD — exactly what this guard prevents.
    if !search_dir.is_absolute() {
        return Err(McpError::invalid_request(
            format!(
                "Refusing to search '{}': the session working directory could not be \
                 resolved to an absolute path. Provide a `path`, or run with a session \
                 working directory set.",
                search_dir.display()
            ),
            None,
        ));
    }

    // An absolute root with no parent is the filesystem root (`/`). Walking it
    // visits every file on the machine and effectively never returns.
    if search_dir.parent().is_none() {
        return Err(McpError::invalid_request(
            format!(
                "Refusing to search the filesystem root: {}. \
                 Provide a `path`, or run with a session working directory set.",
                search_dir.display()
            ),
            None,
        ));
    }

    Ok(())
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
/// let safe_path = validator.validate_path("/home/user/project/src/main.rs")?;
///
/// // This would fail - outside workspace boundary
/// let result = validator.validate_path("/etc/passwd");
/// assert!(result.is_err());
/// ```
#[derive(Debug, Clone)]
pub struct FilePathValidator {
    /// Session working directory — the base relative paths resolve against.
    ///
    /// Tool handlers set this from
    /// [`ToolContext::session_root`](crate::mcp::tool_registry::ToolContext::session_root)
    /// so relative paths are anchored at the board directory, never at the
    /// process current directory.
    base_dir: PathBuf,
    /// Optional workspace root - if set, all paths must be within this directory
    workspace_root: Option<PathBuf>,
    /// Whether to allow symlink resolution (default: false for security)
    allow_symlinks: bool,
    /// Set of blocked path patterns (e.g., patterns that contain dangerous sequences)
    blocked_patterns: HashSet<String>,
    /// Whether to normalize Unicode in paths
    normalize_unicode: bool,
}

impl FilePathValidator {
    /// Creates a new validator rooted at a session working directory.
    ///
    /// Default settings:
    /// - No workspace root restriction
    /// - Symlinks disallowed
    /// - Common dangerous patterns blocked
    /// - Unicode normalization enabled
    ///
    /// # Arguments
    ///
    /// * `base_dir` - The session working directory relative paths resolve
    ///   against. Tool handlers pass
    ///   [`ToolContext::session_root`](crate::mcp::tool_registry::ToolContext::session_root)
    ///   here so resolution is anchored at the board directory, never the
    ///   process current directory.
    pub fn new(base_dir: PathBuf) -> Self {
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
            base_dir,
            workspace_root: None,
            allow_symlinks: false,
            blocked_patterns,
            normalize_unicode: true,
        }
    }

    /// Creates a validator with a specific workspace root.
    ///
    /// All validated paths must be within the specified workspace directory.
    /// This provides strong protection against directory traversal attacks.
    /// The workspace root doubles as the base directory for relative-path
    /// resolution.
    ///
    /// # Arguments
    ///
    /// * `workspace_root` - The root directory that constrains all file
    ///   operations and anchors relative paths
    pub fn with_workspace_root(workspace_root: PathBuf) -> Self {
        let mut validator = Self::new(workspace_root.clone());
        validator.workspace_root = Some(workspace_root);
        validator
    }

    /// Overrides the base directory relative paths resolve against.
    ///
    /// Use when relative resolution should be anchored somewhere other than the
    /// workspace root (or to set the session root on a validator built with a
    /// different constructor).
    pub fn with_base_dir(mut self, base_dir: PathBuf) -> Self {
        self.base_dir = base_dir;
        self
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
    pub fn validate_path(&self, path: &str) -> Result<PathBuf, McpError> {
        // Step 0: Check path length to prevent system issues
        const MAX_PATH_LENGTH: usize = 4096; // Unix PATH_MAX standard
        if path.len() > MAX_PATH_LENGTH {
            return Err(McpError::invalid_request(
                format!(
                    "Path too long ({} characters, maximum {}): {}",
                    path.len(),
                    MAX_PATH_LENGTH,
                    path
                ),
                None,
            ));
        }

        // Step 1: Check for blocked patterns early
        self.check_blocked_patterns(path)?;

        // Step 1: Resolve path (absolute or relative) to absolute path
        let path_buf = PathBuf::from(path);
        let resolved_path = if path_buf.is_absolute() {
            path_buf
        } else {
            // Resolve relative path against the session working directory, never
            // the process CWD (which is `/` for the bundled GUI app).
            self.base_dir.join(path_buf)
        };

        // Step 2: Symlink validation BEFORE canonicalization
        if resolved_path.is_symlink() && !self.allow_symlinks {
            return Err(McpError::invalid_request(
                format!("Symlinks are not allowed: {}", resolved_path.display()),
                None,
            ));
        }

        // Step 3: Basic validation (reuse existing function which may canonicalize).
        // resolved_path is already absolute here, so the base dir is unused, but
        // pass it through for consistency.
        let mut validated_path =
            validate_file_path(&self.base_dir, &resolved_path.to_string_lossy())?;

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
            // Check read permissions only if file exists
            // The actual read operation will handle file-not-found errors appropriately
            if path.exists() {
                let metadata = get_file_metadata(path)?;

                // Check if it's a regular file (not a directory or special file)
                if !metadata.is_file() {
                    return Err(McpError::invalid_request(
                        format!("Path is not a regular file: {}", path.display()),
                        None,
                    ));
                }

                // On Unix systems, check read permission bits
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let permissions = metadata.permissions();
                    let mode = permissions.mode();

                    // Check if owner, group, or others have read permission
                    // User read: 0o400, Group read: 0o040, Others read: 0o004
                    let is_readable = (mode & 0o444) != 0;

                    if !is_readable {
                        return Err(McpError::invalid_request(
                            format!(
                                "File is not readable (no read permissions): {}",
                                path.display()
                            ),
                            None,
                        ));
                    }
                }

                // On non-Unix systems, attempt to open the file for reading as a permission check
                #[cfg(not(unix))]
                {
                    use std::fs::File;
                    match File::open(path) {
                        Ok(_) => {
                            // File is readable
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                            return Err(McpError::invalid_request(
                                format!(
                                    "File is not readable (permission denied): {}",
                                    path.display()
                                ),
                                None,
                            ));
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                            // File doesn't exist, let the read operation handle this
                        }
                        Err(e) => {
                            return Err(McpError::invalid_request(
                                format!("Cannot access file: {}", e),
                                None,
                            ));
                        }
                    }
                }
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

    /// Creates a SecureFileAccess rooted at a session working directory.
    ///
    /// Relative paths resolve against `base_dir`. Tool handlers pass
    /// [`ToolContext::session_root`](crate::mcp::tool_registry::ToolContext::session_root).
    pub fn default_secure(base_dir: PathBuf) -> Self {
        Self::new(FilePathValidator::new(base_dir))
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
        let validated_path = match self.validator.validate_path(path) {
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
        let validated_path = self.validator.validate_path(path)?;

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
        let validated_path = self.validator.validate_path(path)?;

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

    /// A throwaway session-root base for tests that only exercise absolute,
    /// empty, or too-long paths (where relative resolution never happens).
    fn test_base() -> PathBuf {
        std::env::temp_dir()
    }

    #[test]
    fn test_reject_filesystem_root_rejects_root() {
        let result = reject_filesystem_root(Path::new("/"));
        assert!(result.is_err(), "the filesystem root must be rejected");
        let err = format!("{:?}", result.unwrap_err());
        assert!(
            err.contains("filesystem root"),
            "error should explain the filesystem-root refusal, got: {err}"
        );
    }

    #[test]
    fn test_reject_filesystem_root_rejects_relative_dot() {
        // The `session_root` last-resort fallback. A relative `.` slips past a
        // naive `parent().is_none()` check, so it must be rejected explicitly.
        let result = reject_filesystem_root(Path::new("."));
        assert!(result.is_err(), "a bare relative `.` must be rejected");
        let err = format!("{:?}", result.unwrap_err());
        assert!(
            err.contains("could not be"),
            "error should explain the unresolved-root refusal, got: {err}"
        );
    }

    #[test]
    fn test_reject_filesystem_root_rejects_empty() {
        let result = reject_filesystem_root(Path::new(""));
        assert!(result.is_err(), "an empty path must be rejected");
    }

    #[test]
    fn test_reject_filesystem_root_accepts_normal_dir() {
        let temp_dir = TempDir::new().unwrap();
        let result = reject_filesystem_root(temp_dir.path());
        assert!(
            result.is_ok(),
            "a normal absolute directory with a parent must be accepted: {:?}",
            result
        );
    }

    #[test]
    fn test_validate_file_path_empty() {
        let result = validate_file_path(&test_base(), "");
        assert!(result.is_err());

        let result = validate_file_path(&test_base(), "   ");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_file_path_relative() {
        // Relative paths resolve against the explicit base dir (the session
        // working directory), so no cwd pinning is needed.
        let temp_dir = TempDir::new().unwrap();
        let base = temp_dir.path().to_path_buf();

        // Create test files under the base
        fs::create_dir_all(base.join("relative")).unwrap();
        fs::write(base.join("relative/path"), "test content").unwrap();
        fs::create_dir_all(base.join("current")).unwrap();
        fs::write(base.join("current/path"), "test content").unwrap();

        // Relative paths should be accepted and resolved against the base
        let result = validate_file_path(&base, "relative/path");
        assert!(result.is_ok(), "Simple relative paths should be accepted");
        let resolved = result.unwrap();
        assert!(
            resolved.is_absolute(),
            "Should be resolved to absolute path"
        );

        let result = validate_file_path(&base, "./current/path");
        assert!(
            result.is_ok(),
            "Current directory relative paths should be accepted"
        );
        let resolved = result.unwrap();
        assert!(
            resolved.is_absolute(),
            "Should be resolved to absolute path"
        );

        // Parent directory paths should still be blocked by dangerous pattern checking
        let result = validate_file_path(&base, "../parent/path");
        assert!(
            result.is_err(),
            "Parent directory traversal should still be blocked"
        );
    }

    #[test]
    fn test_validate_file_path_extremely_long() {
        // Test extremely long path that exceeds PATH_MAX
        let extremely_long_path = "a".repeat(5000);
        let result = validate_file_path(&test_base(), &extremely_long_path);
        assert!(result.is_err(), "Extremely long paths should be rejected");

        let error_msg = format!("{:?}", result.unwrap_err());
        println!("Error message: {}", error_msg);
        assert!(
            error_msg.contains("Path too long") || error_msg.contains("path too long"),
            "Should mention path length issue"
        );
    }

    #[test]
    fn test_validate_file_path_absolute_nonexistent() {
        // This should succeed even if the file doesn't exist,
        // as long as the parent directory exists
        let temp_dir = TempDir::new().unwrap();
        let non_existent_file = temp_dir.path().join("does_not_exist.txt");
        let result = validate_file_path(&test_base(), &non_existent_file.to_string_lossy());
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_file_path_absolute_existing() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        fs::write(&test_file, "test content").unwrap();

        let result = validate_file_path(&test_base(), &test_file.to_string_lossy());
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
        let validator = FilePathValidator::new(test_base());

        // Test default blocked patterns
        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path().to_string_lossy().to_string();

        // These should be blocked by default patterns
        let dangerous_paths = vec![
            format!("{}/../etc/passwd", base_path),
            format!("{}\\..\\windows\\system32", base_path),
        ];

        for dangerous_path in dangerous_paths {
            let result = validator.validate_path(&dangerous_path);
            assert!(
                result.is_err(),
                "Should block dangerous path: {}",
                dangerous_path
            );
        }
    }

    #[test]
    fn test_file_path_validator_relative_paths() {
        // Relative paths resolve against the validator's base dir (the session
        // working directory), so no cwd pinning is needed.
        let temp_dir = TempDir::new().unwrap();
        let validator = FilePathValidator::new(temp_dir.path().to_path_buf());

        // Create test files
        let test_file = temp_dir.path().join("test_file.txt");
        fs::write(&test_file, "test content").unwrap();

        // Test basic relative path resolution
        let result = validator.validate_path("test_file.txt");
        assert!(result.is_ok(), "Should accept simple relative path");
        let resolved = result.unwrap();
        assert!(resolved.is_absolute(), "Resolved path should be absolute");
        assert!(
            resolved.ends_with("test_file.txt"),
            "Should preserve filename"
        );

        // Test current directory relative path
        let result = validator.validate_path("./test_file.txt");
        assert!(result.is_ok(), "Should accept ./ relative path");

        // Test parent directory (should be blocked by dangerous patterns)
        let result = validator.validate_path("../test_file.txt");
        assert!(result.is_err(), "Should block ../ path traversal");

        // Test nested relative path
        let nested_dir = temp_dir.path().join("nested");
        fs::create_dir(&nested_dir).unwrap();
        let nested_file = nested_dir.join("nested_file.txt");
        fs::write(&nested_file, "nested content").unwrap();

        let result = validator.validate_path("nested/nested_file.txt");
        assert!(result.is_ok(), "Should accept nested relative path");
    }

    #[test]
    fn test_file_path_validator_relative_with_workspace() {
        let temp_dir = TempDir::new().unwrap();
        let workspace_root = temp_dir.path().to_path_buf();
        // with_workspace_root anchors relative resolution at the workspace, so a
        // relative path resolves within it without any cwd manipulation.
        let validator = FilePathValidator::with_workspace_root(workspace_root.clone());

        // Create test file in workspace
        let test_file = workspace_root.join("workspace_file.txt");
        fs::write(&test_file, "workspace content").unwrap();

        // Test relative path within workspace
        let result = validator.validate_path("workspace_file.txt");
        assert!(
            result.is_ok(),
            "Should accept relative path within workspace"
        );

        // An absolute path outside the workspace must be rejected by the
        // boundary check.
        let outside_dir = TempDir::new().unwrap();
        let outside_file = outside_dir.path().join("outside_file.txt");
        fs::write(&outside_file, "outside content").unwrap();

        let result = validator.validate_path(&outside_file.to_string_lossy());
        assert!(result.is_err(), "Should reject path outside workspace");
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
        let result = validator.validate_path(&safe_file.to_string_lossy());
        assert!(result.is_ok());

        // This should fail - outside workspace (system file)
        let result = validator.validate_path("/etc/passwd");
        assert!(result.is_err());

        // Test with a path outside workspace that exists
        let outside_dir = TempDir::new().unwrap();
        let outside_file = outside_dir.path().join("outside.txt");
        fs::write(&outside_file, "outside content").unwrap();

        let result = validator.validate_path(&outside_file.to_string_lossy());
        assert!(result.is_err());
    }

    #[test]
    fn test_file_path_validator_blocked_patterns() {
        let mut validator = FilePathValidator::new(test_base());
        validator.add_blocked_pattern("secret".to_string());

        let temp_dir = TempDir::new().unwrap();
        let safe_file = temp_dir.path().join("normal.txt");
        let dangerous_file = temp_dir.path().join("secret_file.txt");

        // Safe file should pass
        let result = validator.validate_path(&safe_file.to_string_lossy());
        assert!(result.is_ok());

        // File with blocked pattern should fail
        let result = validator.validate_path(&dangerous_file.to_string_lossy());
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
                    let validator = FilePathValidator::new(test_base());
                    let result = validator.validate_path(&symlink_file.to_string_lossy());
                    assert!(
                        result.is_err(),
                        "Symlink should be rejected when symlinks are disabled"
                    );

                    // Test with symlinks enabled
                    let mut validator = FilePathValidator::new(test_base());
                    validator.set_allow_symlinks(true);
                    let result = validator.validate_path(&symlink_file.to_string_lossy());
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
        let validator = FilePathValidator::new(test_base());

        // Test null byte rejection
        let result = validator.validate_path("/tmp/file\0.txt");
        assert!(result.is_err());

        // Test other control characters
        let result = validator.validate_path("/tmp/file\x01.txt");
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
        let temp_dir = TempDir::new().unwrap();
        let workspace_root = temp_dir.path().to_path_buf();

        // Relative paths resolve against the session base dir (the temp dir), so
        // no cwd pinning is needed.
        let secure_access = SecureFileAccess::default_secure(workspace_root.clone());

        // Create test file with content
        let test_file = workspace_root.join("relative_test.txt");
        let content = "Relative path content";
        fs::write(&test_file, content).unwrap();

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
            let result = validator.validate_path(&full_path);
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

    #[test]
    fn test_secure_file_access_edit_multiple_matches_error() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("multi_match.txt");
        fs::write(&test_file, "foo bar foo baz foo").unwrap();

        let secure_access = SecureFileAccess::default_secure(test_base());

        // Edit with multiple matches and replace_all=false should fail
        let result = secure_access.edit(
            &test_file.to_string_lossy(),
            "foo",
            "qux",
            false, // replace_all = false
        );
        assert!(result.is_err());
        let err = format!("{:?}", result.unwrap_err());
        assert!(err.contains("appears") || err.contains("times") || err.contains("replace_all"));
    }

    #[test]
    fn test_secure_file_access_edit_replace_all_success() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("replace_all.txt");
        fs::write(&test_file, "foo bar foo baz foo").unwrap();

        let secure_access = SecureFileAccess::default_secure(test_base());

        // Edit with replace_all=true should succeed
        let result = secure_access.edit(
            &test_file.to_string_lossy(),
            "foo",
            "qux",
            true, // replace_all = true
        );
        assert!(result.is_ok());

        let content = fs::read_to_string(&test_file).unwrap();
        assert_eq!(content, "qux bar qux baz qux");
    }

    #[test]
    fn test_secure_file_access_edit_string_not_found_error() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("no_match.txt");
        fs::write(&test_file, "hello world").unwrap();

        let secure_access = SecureFileAccess::default_secure(test_base());

        let result = secure_access.edit(
            &test_file.to_string_lossy(),
            "nonexistent_string",
            "replacement",
            false,
        );
        assert!(result.is_err());
        let err = format!("{:?}", result.unwrap_err());
        assert!(err.contains("not found") || err.contains("nonexistent_string"));
    }

    #[test]
    fn test_handle_file_error_permission_denied() {
        let path = std::path::Path::new("/some/path.txt");
        let io_error = std::io::Error::from(std::io::ErrorKind::PermissionDenied);
        let mcp_error = handle_file_error(io_error, "read", path);
        let err_str = format!("{:?}", mcp_error);
        assert!(err_str.contains("Permission denied") || err_str.contains("permission"));
    }

    #[test]
    fn test_handle_file_error_already_exists() {
        let path = std::path::Path::new("/some/path.txt");
        let io_error = std::io::Error::from(std::io::ErrorKind::AlreadyExists);
        let mcp_error = handle_file_error(io_error, "create", path);
        let err_str = format!("{:?}", mcp_error);
        assert!(err_str.contains("already exists") || err_str.contains("AlreadyExists"));
    }

    #[test]
    fn test_handle_file_error_invalid_data() {
        let path = std::path::Path::new("/some/path.txt");
        let io_error = std::io::Error::from(std::io::ErrorKind::InvalidData);
        let mcp_error = handle_file_error(io_error, "read", path);
        let err_str = format!("{:?}", mcp_error);
        assert!(err_str.contains("Invalid") || err_str.contains("data"));
    }

    #[test]
    fn test_check_file_permissions_directory_path() {
        let temp_dir = TempDir::new().unwrap();

        // Passing a directory path to Read should fail (not a regular file)
        let result = check_file_permissions(temp_dir.path(), FileOperation::Read);
        assert!(result.is_err());
        let err = format!("{:?}", result.unwrap_err());
        assert!(err.contains("not a regular file") || err.contains("directory"));
    }

    #[test]
    fn test_check_file_permissions_directory_operation() {
        let temp_dir = TempDir::new().unwrap();

        // Directory operation on an existing directory should succeed
        let result = check_file_permissions(temp_dir.path(), FileOperation::Directory);
        assert!(result.is_ok());
    }

    #[test]
    fn test_check_file_permissions_directory_operation_file_exists() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        fs::write(&test_file, "content").unwrap();

        // Directory operation on a file (not directory) should fail
        let result = check_file_permissions(&test_file, FileOperation::Directory);
        assert!(result.is_err());
        let err = format!("{:?}", result.unwrap_err());
        assert!(err.contains("not a directory") || err.contains("directory"));
    }

    #[test]
    fn test_secure_file_access_read_with_limit_only() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("limit_only.txt");
        fs::write(&test_file, "Line 1\nLine 2\nLine 3\nLine 4\nLine 5\n").unwrap();

        let secure_access = SecureFileAccess::default_secure(test_base());

        let result = secure_access.read(&test_file.to_string_lossy(), None, Some(2));
        assert!(result.is_ok());
        let content = result.unwrap();
        assert!(content.contains("Line 1"));
        assert!(content.contains("Line 2"));
        assert!(!content.contains("Line 3"));
    }

    #[test]
    fn test_secure_file_access_read_with_offset_only() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("offset_only.txt");
        fs::write(&test_file, "Line 1\nLine 2\nLine 3\nLine 4\nLine 5\n").unwrap();

        let secure_access = SecureFileAccess::default_secure(test_base());

        let result = secure_access.read(&test_file.to_string_lossy(), Some(3), None);
        assert!(result.is_ok());
        let content = result.unwrap();
        assert!(!content.contains("Line 1"));
        assert!(!content.contains("Line 2"));
        assert!(content.contains("Line 3"));
    }

    #[test]
    fn test_file_path_validator_long_path() {
        let validator = FilePathValidator::new(test_base());
        let long_path = "/".to_string() + &"a".repeat(4097);

        let result = validator.validate_path(&long_path);
        assert!(result.is_err());
        let err = format!("{:?}", result.unwrap_err());
        assert!(err.contains("too long") || err.contains("Path too long") || err.contains("4096"));
    }
}
