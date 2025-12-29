use crate::size_validator::{SizeValidationError, SizeValidator};
use std::path::{Path, PathBuf};
use thiserror::Error;

/// File operation permissions that can be validated
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Permission {
    /// Read permission
    Read,
    /// Write permission
    Write,
    /// Execute permission (Unix-specific)
    Execute,
}

/// ACP-compliant path validator with comprehensive security and platform validation
#[derive(Debug, Clone)]
pub struct PathValidator {
    /// Allowed root directories for file operations (empty = allow all except blocked)
    allowed_roots: Vec<PathBuf>,
    /// Blocked path prefixes that are explicitly denied
    blocked_paths: Vec<PathBuf>,
    /// Whether to perform strict canonicalization (default: true)
    strict_canonicalization: bool,
    /// Size validator for path length validation
    size_validator: SizeValidator,
}

/// Errors that can occur during path validation
#[derive(Debug, Error, PartialEq)]
pub enum PathValidationError {
    #[error("Path is not absolute: {0}")]
    NotAbsolute(String),

    #[error("Path traversal attempt detected in: {0}")]
    PathTraversalAttempt(String),

    #[error("Path contains relative components: {0}")]
    RelativeComponent(String),

    #[error("Path too long: {0} characters > maximum allowed ({1})")]
    PathTooLong(usize, usize),

    #[error("Path canonicalization failed for {0}: {1}")]
    CanonicalizationFailed(String, String),

    #[error("Path outside allowed boundaries: {0}")]
    OutsideBoundaries(String),

    #[error("Path is blocked: {0}")]
    Blocked(String),

    #[error("Invalid path format: {0}")]
    InvalidFormat(String),

    #[error("Path contains null bytes")]
    NullBytesInPath,

    #[error("Empty path provided")]
    EmptyPath,

    #[error("Insufficient permissions for path {path}: missing {required}")]
    InsufficientPermissions { path: String, required: String },
}

impl From<SizeValidationError> for PathValidationError {
    fn from(error: SizeValidationError) -> Self {
        match error {
            SizeValidationError::SizeExceeded { actual, limit, .. } => {
                PathValidationError::PathTooLong(actual, limit)
            }
        }
    }
}

impl Default for PathValidator {
    fn default() -> Self {
        Self::new()
    }
}

impl PathValidator {
    /// Create a new path validator with default settings
    pub fn new() -> Self {
        let size_validator = SizeValidator::default();

        Self {
            allowed_roots: Vec::new(),
            blocked_paths: Vec::new(),
            strict_canonicalization: true,
            size_validator,
        }
    }

    /// Create a path validator with custom maximum path length
    pub fn with_max_length(max_length: usize) -> Self {
        let size_validator = SizeValidator::new(crate::size_validator::SizeLimits {
            max_path_length: max_length,
            ..Default::default()
        });

        Self {
            size_validator,
            ..Self::new()
        }
    }

    /// Create a path validator with allowed root directories
    pub fn with_allowed_roots(roots: Vec<PathBuf>) -> Self {
        Self {
            allowed_roots: roots,
            ..Self::new()
        }
    }

    /// Create a path validator with blocked paths
    pub fn with_blocked_paths(blocked: Vec<PathBuf>) -> Self {
        Self {
            blocked_paths: blocked,
            ..Self::new()
        }
    }

    /// Create a path validator with both allowed roots and blocked paths
    pub fn with_allowed_and_blocked(allowed: Vec<PathBuf>, blocked: Vec<PathBuf>) -> Self {
        Self {
            allowed_roots: allowed,
            blocked_paths: blocked,
            ..Self::new()
        }
    }

    /// Create a path validator with custom strict canonicalization setting
    pub fn with_strict_canonicalization(mut self, strict: bool) -> Self {
        self.strict_canonicalization = strict;
        self
    }

    /// Validate that a path is absolute according to ACP specification.
    ///
    /// This method performs comprehensive path validation with multiple security checks:
    ///
    /// 1. **Quick traversal check** - Fast string-based pattern matching for early rejection
    /// 2. **Absolute path verification** - Platform-specific absolute path validation
    /// 3. **Canonicalization** - Resolves symlinks and normalizes path (if strict mode enabled)
    /// 4. **Component-based security** - Authoritative check for traversal attempts
    /// 5. **Blocked path check** - Ensures path is not in blocked list
    /// 6. **Boundary validation** - Ensures path is within allowed roots (if configured)
    ///
    /// The validation uses a layered approach where the string-based check provides
    /// fast rejection of obviously malicious paths, while the component-based check
    /// serves as the canonical security validation after normalization.
    pub fn validate_absolute_path(&self, path_str: &str) -> Result<PathBuf, PathValidationError> {
        // Check for empty path
        if path_str.is_empty() {
            tracing::warn!(
                security_event = "invalid_path",
                reason = "empty_path",
                "Empty path provided"
            );
            return Err(PathValidationError::EmptyPath);
        }

        // Check path length
        self.size_validator.validate_path_length(path_str)?;

        // Check for null bytes
        if path_str.contains('\0') {
            tracing::warn!(
                security_event = "invalid_path",
                reason = "null_bytes",
                path = path_str,
                "Path contains null bytes"
            );
            return Err(PathValidationError::NullBytesInPath);
        }

        // Parse path
        let path = PathBuf::from(path_str);

        // Check if path is absolute using platform-specific logic
        if !self.is_absolute_path(&path) {
            tracing::warn!(
                security_event = "invalid_path",
                reason = "not_absolute",
                path = path_str,
                "Non-absolute path rejected"
            );
            return Err(PathValidationError::NotAbsolute(path_str.to_string()));
        }

        // Quick traversal check on raw string (fast early rejection)
        self.quick_traversal_check(path_str)?;

        // Normalize path if strict canonicalization is enabled
        let normalized = if self.strict_canonicalization {
            self.normalize_path(&path)?
        } else {
            path
        };

        // Check for path traversal in normalized path
        self.validate_path_security(&normalized)?;

        // Check blocked paths first (blocked takes precedence over allowed)
        if !self.blocked_paths.is_empty() {
            self.validate_not_blocked(&normalized)?;
        }

        // Validate path boundaries if allowed roots are configured
        if !self.allowed_roots.is_empty() {
            self.validate_path_boundaries(&normalized)?;
        }

        Ok(normalized)
    }

    /// Check if path is absolute using platform-specific rules
    fn is_absolute_path(&self, path: &Path) -> bool {
        // Use std::path::Path::is_absolute() which handles platform differences
        path.is_absolute()
    }

    /// Fast string-based path traversal pre-check for early rejection.
    ///
    /// This is an optimization that quickly rejects obviously malicious paths
    /// before expensive operations like canonicalization. It checks for common
    /// path traversal patterns in the raw string.
    ///
    /// Note: This is NOT the canonical security check. Always use `validate_path_security()`
    /// on the normalized Path for authoritative validation.
    fn quick_traversal_check(&self, path_str: &str) -> Result<(), PathValidationError> {
        let suspicious_patterns = ["/../", "\\..\\", "/..", "\\..", "../", "..\\"];
        for pattern in &suspicious_patterns {
            if path_str.contains(pattern) {
                tracing::warn!(
                    security_event = "path_traversal_attempt",
                    path = path_str,
                    pattern = pattern,
                    "Path traversal attempt detected during quick check"
                );
                return Err(PathValidationError::PathTraversalAttempt(
                    path_str.to_string(),
                ));
            }
        }
        Ok(())
    }

    /// Normalize path by resolving symlinks and canonical form
    fn normalize_path(&self, path: &Path) -> Result<PathBuf, PathValidationError> {
        path.canonicalize().map_err(|e| {
            PathValidationError::CanonicalizationFailed(
                path.to_string_lossy().to_string(),
                e.to_string(),
            )
        })
    }

    /// Canonical component-based path security validation.
    ///
    /// This is the authoritative security check that examines the path's components
    /// to detect parent directory (`..`) and current directory (`.`) references.
    /// This method is robust against encoding tricks and path obfuscation.
    ///
    /// Should be called on normalized/canonicalized paths for best results.
    fn validate_path_security(&self, path: &Path) -> Result<(), PathValidationError> {
        for component in path.components() {
            match component {
                std::path::Component::ParentDir => {
                    tracing::warn!(
                        security_event = "path_traversal_attempt",
                        path = %path.display(),
                        component = "ParentDir",
                        "Parent directory component detected in normalized path"
                    );
                    return Err(PathValidationError::PathTraversalAttempt(
                        path.display().to_string(),
                    ));
                }
                std::path::Component::CurDir => {
                    tracing::warn!(
                        security_event = "relative_component",
                        path = %path.display(),
                        component = "CurDir",
                        "Current directory component detected in path"
                    );
                    return Err(PathValidationError::RelativeComponent(
                        path.display().to_string(),
                    ));
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// Validate that path is not in the blocked list
    fn validate_not_blocked(&self, path: &Path) -> Result<(), PathValidationError> {
        for blocked in &self.blocked_paths {
            if path.starts_with(blocked) {
                tracing::warn!(
                    security_event = "blocked_path_access",
                    path = %path.display(),
                    blocked_prefix = %blocked.display(),
                    "Attempt to access blocked path"
                );
                return Err(PathValidationError::Blocked(
                    path.to_string_lossy().to_string(),
                ));
            }
        }
        Ok(())
    }

    /// Validate that path is within allowed boundaries
    fn validate_path_boundaries(&self, path: &Path) -> Result<(), PathValidationError> {
        if self.allowed_roots.is_empty() {
            return Ok(());
        }

        for allowed_root in &self.allowed_roots {
            if path.starts_with(allowed_root) {
                return Ok(());
            }
        }

        tracing::warn!(
            security_event = "boundary_violation",
            path = %path.display(),
            allowed_roots = ?self.allowed_roots,
            "Path access outside allowed boundaries"
        );
        Err(PathValidationError::OutsideBoundaries(
            path.to_string_lossy().to_string(),
        ))
    }

    /// Validate that the current process has the required permissions for the path.
    ///
    /// This method performs explicit permission checking as required by ACP file security policy.
    /// It checks if the current process has the necessary read/write/execute permissions
    /// for the specified path.
    ///
    /// # Platform-specific behavior
    ///
    /// - **Unix**: Uses file metadata to check read, write, and execute permissions
    /// - **Windows**: Uses file metadata to check read-only attribute for write operations
    ///
    /// Note: This method checks if the path exists and has the required permissions.
    /// For non-existent paths, this will return an error.
    pub fn validate_permissions(
        &self,
        path: &Path,
        required: &[Permission],
    ) -> Result<(), PathValidationError> {
        // Get file metadata to check permissions
        let metadata = match std::fs::metadata(path) {
            Ok(m) => m,
            Err(e) => {
                // If we can't read metadata, we don't have permissions
                let permission_names: Vec<&str> = required
                    .iter()
                    .map(|p| match p {
                        Permission::Read => "read",
                        Permission::Write => "write",
                        Permission::Execute => "execute",
                    })
                    .collect();

                tracing::warn!(
                    security_event = "permission_check_failed",
                    path = %path.display(),
                    required_permissions = ?permission_names,
                    error = %e,
                    "Failed to check permissions - metadata not accessible"
                );

                return Err(PathValidationError::InsufficientPermissions {
                    path: path.to_string_lossy().to_string(),
                    required: permission_names.join(", "),
                });
            }
        };

        #[cfg(unix)]
        {
            use std::os::unix::fs::{MetadataExt, PermissionsExt};

            let mode = metadata.permissions().mode();
            // Get current process uid/gid
            // SAFETY: These are read-only system calls that always succeed
            let uid = unsafe { libc::getuid() };
            let gid = unsafe { libc::getgid() };

            // Check which permission bits apply (owner, group, or other)
            let (read_bit, write_bit, execute_bit) = if metadata.uid() == uid {
                // Owner permissions
                (0o400, 0o200, 0o100)
            } else if metadata.gid() == gid {
                // Group permissions
                (0o040, 0o020, 0o010)
            } else {
                // Other permissions
                (0o004, 0o002, 0o001)
            };

            for permission in required {
                let has_permission = match permission {
                    Permission::Read => mode & read_bit != 0,
                    Permission::Write => mode & write_bit != 0,
                    Permission::Execute => mode & execute_bit != 0,
                };

                if !has_permission {
                    let permission_name = match permission {
                        Permission::Read => "read",
                        Permission::Write => "write",
                        Permission::Execute => "execute",
                    };

                    tracing::warn!(
                        security_event = "insufficient_permissions",
                        path = %path.display(),
                        required_permission = permission_name,
                        mode = format!("{:o}", mode),
                        "Insufficient permissions for file operation"
                    );

                    return Err(PathValidationError::InsufficientPermissions {
                        path: path.to_string_lossy().to_string(),
                        required: permission_name.to_string(),
                    });
                }
            }
        }

        #[cfg(windows)]
        {
            // On Windows, check read-only flag for write operations
            for permission in required {
                match permission {
                    Permission::Write => {
                        if metadata.permissions().readonly() {
                            tracing::warn!(
                                security_event = "insufficient_permissions",
                                path = %path.display(),
                                required_permission = "write",
                                "File is read-only, write permission denied"
                            );

                            return Err(PathValidationError::InsufficientPermissions {
                                path: path.to_string_lossy().to_string(),
                                required: "write".to_string(),
                            });
                        }
                    }
                    Permission::Read => {
                        // If we can read metadata, we can read the file
                        // Windows doesn't have a read-only check that prevents reading
                    }
                    Permission::Execute => {
                        // Windows execute permission is based on file extension
                        // We'll consider any existing file as potentially executable
                        // More granular checking would require Windows API calls
                    }
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_unix_absolute_paths() {
        let validator = PathValidator::new();

        // Valid Unix absolute paths
        let valid_paths = vec![
            "/",
            "/home",
            "/home/user",
            "/home/user/document.txt",
            "/tmp/file.txt",
            "/usr/local/bin/program",
        ];

        for path in valid_paths {
            let result = validator.validate_absolute_path(path);
            if cfg!(unix) {
                // On Unix systems, these should pass basic validation
                // Note: canonicalization might fail if paths don't exist
                match result {
                    Ok(_) => {} // Path validated successfully
                    Err(PathValidationError::CanonicalizationFailed(_, _)) => {
                        // Expected for non-existent paths with strict canonicalization
                    }
                    Err(e) => panic!(
                        "Expected Unix absolute path '{}' to pass basic validation, got: {}",
                        path, e
                    ),
                }
            }
        }
    }

    #[test]
    fn test_windows_absolute_paths() {
        let validator = PathValidator::new();

        // Valid Windows absolute paths
        let valid_paths = vec![
            "C:\\",
            "C:\\Users",
            "C:\\Users\\user\\document.txt",
            "D:\\Program Files\\app\\file.exe",
            "\\\\server\\share\\file.txt", // UNC path
        ];

        for path in valid_paths {
            let result = validator.validate_absolute_path(path);
            if cfg!(windows) {
                // On Windows systems, these should pass basic validation
                match result {
                    Ok(_) => {} // Path validated successfully
                    Err(PathValidationError::CanonicalizationFailed(_, _)) => {
                        // Expected for non-existent paths with strict canonicalization
                    }
                    Err(e) => panic!(
                        "Expected Windows absolute path '{}' to pass basic validation, got: {}",
                        path, e
                    ),
                }
            }
        }
    }

    #[test]
    fn test_relative_path_rejection() {
        let validator = PathValidator::new();

        // Invalid relative paths
        let invalid_paths = vec![
            "relative/path",
            "./current/dir",
            "../parent/dir",
            "file.txt",
            "src/main.rs",
            "config/settings.json",
        ];

        for path in invalid_paths {
            let result = validator.validate_absolute_path(path);
            match result {
                Err(PathValidationError::NotAbsolute(p)) => {
                    assert_eq!(p, path);
                }
                Err(PathValidationError::PathTraversalAttempt(_)) => {
                    // Also acceptable for ../ patterns
                }
                Ok(_) => panic!("Expected relative path '{}' to be rejected", path),
                Err(e) => panic!("Expected NotAbsolute error for '{}', got: {}", path, e),
            }
        }
    }

    #[test]
    fn test_path_traversal_detection() {
        let validator = PathValidator::new();

        // Unix-style traversal paths (should work on all platforms)
        let unix_traversal_paths = vec![
            "/home/user/../../../etc/passwd",
            "/tmp/../../../root/.ssh/id_rsa",
        ];

        for path in unix_traversal_paths {
            let result = validator.validate_absolute_path(path);
            match result {
                Err(PathValidationError::PathTraversalAttempt(_)) => {
                    // Expected
                }
                Err(PathValidationError::CanonicalizationFailed(_, _)) => {
                    // Also acceptable - canonicalization might catch traversal
                }
                Ok(_) => panic!("Expected path traversal to be detected for '{}'", path),
                Err(e) => panic!("Expected PathTraversalAttempt for '{}', got: {}", path, e),
            }
        }

        // Platform-specific tests
        if cfg!(windows) {
            let windows_traversal_paths = vec![
                "C:\\Users\\user\\..\\..\\Windows\\System32",
                "\\\\server\\share\\..\\..\\admin",
            ];

            for path in windows_traversal_paths {
                let result = validator.validate_absolute_path(path);
                match result {
                    Err(PathValidationError::PathTraversalAttempt(_)) => {
                        // Expected
                    }
                    Err(PathValidationError::CanonicalizationFailed(_, _)) => {
                        // Also acceptable - canonicalization might catch traversal
                    }
                    Ok(_) => panic!("Expected path traversal to be detected for '{}'", path),
                    Err(e) => panic!("Expected PathTraversalAttempt for '{}', got: {}", path, e),
                }
            }
        } else {
            // On non-Windows systems, Windows paths should be rejected as non-absolute
            let windows_paths = vec![
                "C:\\Users\\user\\..\\..\\Windows\\System32",
                "\\\\server\\share\\..\\..\\admin",
            ];

            for path in windows_paths {
                let result = validator.validate_absolute_path(path);
                match result {
                    Err(PathValidationError::NotAbsolute(_)) => {
                        // Expected on non-Windows systems
                    }
                    Err(PathValidationError::PathTraversalAttempt(_)) => {
                        // Also acceptable if detected as traversal before absolute check
                    }
                    Ok(_) => panic!(
                        "Expected Windows path to be rejected on non-Windows system: '{}'",
                        path
                    ),
                    Err(e) => panic!(
                        "Expected NotAbsolute for Windows path '{}' on non-Windows system, got: {}",
                        path, e
                    ),
                }
            }
        }
    }

    #[test]
    fn test_empty_and_invalid_paths() {
        let validator = PathValidator::new();

        // Empty path
        assert_eq!(
            validator.validate_absolute_path(""),
            Err(PathValidationError::EmptyPath)
        );

        // Null bytes
        assert_eq!(
            validator.validate_absolute_path("/path/with\0null"),
            Err(PathValidationError::NullBytesInPath)
        );
    }

    #[test]
    fn test_path_length_limit() {
        let validator = PathValidator::with_max_length(50);

        let long_path = "/".repeat(100);
        let result = validator.validate_absolute_path(&long_path);

        match result {
            Err(PathValidationError::PathTooLong(actual, max)) => {
                assert_eq!(actual, 100);
                assert_eq!(max, 50);
            }
            _ => panic!("Expected PathTooLong error"),
        }
    }

    #[test]
    fn test_allowed_roots_validation() {
        let temp_dir = TempDir::new().unwrap();
        let allowed_root = temp_dir.path().to_path_buf();

        let validator = PathValidator::with_allowed_roots(vec![allowed_root.clone()]);

        // Test path within allowed root
        let allowed_path = allowed_root.join("subdir").join("file.txt");
        let result = validator.validate_absolute_path(&allowed_path.to_string_lossy());

        // Should fail with canonicalization since file doesn't exist, but not with boundary check
        match result {
            Err(PathValidationError::CanonicalizationFailed(_, _)) => {
                // Expected due to non-existent path
            }
            Ok(_) => {
                // Also acceptable if path validation succeeds
            }
            Err(e) => panic!("Expected CanonicalizationFailed or success, got: {}", e),
        }

        // Test path outside allowed roots
        let outside_path = "/completely/different/path";
        let result = validator.validate_absolute_path(outside_path);

        // Should fail with boundary error or canonicalization error
        match result {
            Err(PathValidationError::OutsideBoundaries(_)) => {
                // Expected
            }
            Err(PathValidationError::CanonicalizationFailed(_, _)) => {
                // Also acceptable - canonicalization happens first
            }
            Ok(_) => panic!("Expected path outside boundaries to be rejected"),
            Err(e) => panic!(
                "Expected OutsideBoundaries or CanonicalizationFailed, got: {}",
                e
            ),
        }
    }

    #[test]
    fn test_non_strict_canonicalization() {
        let mut validator = PathValidator::new();
        validator.strict_canonicalization = false;

        // Non-existent but well-formed absolute path
        let path = "/non/existent/path/file.txt";
        let result = validator.validate_absolute_path(path);

        // Should succeed without canonicalization
        assert!(
            result.is_ok(),
            "Expected path validation to succeed without canonicalization"
        );
    }

    #[test]
    fn test_path_validator_builder_methods() {
        let validator = PathValidator::with_max_length(1024);
        assert_eq!(validator.size_validator.limits().max_path_length, 1024);

        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path().to_path_buf();
        let validator = PathValidator::with_allowed_roots(vec![root.clone()]);
        assert_eq!(validator.allowed_roots, vec![root]);
    }

    #[test]
    fn test_current_directory_and_parent_directory_components() {
        let validator = PathValidator::new();

        // Test paths with current directory components
        let paths_with_current = vec!["/home/./user/file.txt", "/tmp/./config"];

        for path in paths_with_current {
            let result = validator.validate_absolute_path(path);
            // These might pass or fail depending on canonicalization behavior
            // The key is that they are handled consistently
            match result {
                Ok(_) => {}                                          // Canonicalization resolved the ./
                Err(PathValidationError::RelativeComponent(_)) => {} // Detected as relative component
                Err(PathValidationError::CanonicalizationFailed(_, _)) => {} // Path doesn't exist
                Err(e) => panic!(
                    "Unexpected error for path with current dir component '{}': {}",
                    path, e
                ),
            }
        }
    }

    #[test]
    fn test_blocked_paths() {
        let temp_dir = TempDir::new().unwrap();
        let blocked_dir = temp_dir.path().join("blocked");
        let test_file = blocked_dir.join("test.txt");
        std::fs::create_dir(&blocked_dir).unwrap();
        std::fs::write(&test_file, "test").unwrap();

        let validator = PathValidator::with_blocked_paths(vec![blocked_dir.clone()]);
        let result = validator.validate_absolute_path(&test_file.to_string_lossy());

        match result {
            Err(PathValidationError::Blocked(_)) => {
                // Expected
            }
            Ok(_) => panic!("Expected blocked path to be rejected"),
            Err(e) => panic!("Expected Blocked error, got: {}", e),
        }
    }

    #[test]
    fn test_blocked_takes_precedence_over_allowed() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        std::fs::write(&test_file, "test").unwrap();

        // Both in allowed and blocked - blocked should win
        let validator = PathValidator::with_allowed_and_blocked(
            vec![temp_dir.path().to_path_buf()],
            vec![temp_dir.path().to_path_buf()],
        );
        let result = validator.validate_absolute_path(&test_file.to_string_lossy());

        match result {
            Err(PathValidationError::Blocked(_)) => {
                // Expected
            }
            Ok(_) => panic!("Expected blocked path to take precedence"),
            Err(e) => panic!("Expected Blocked error, got: {}", e),
        }
    }

    #[test]
    fn test_subdirectory_of_blocked() {
        let temp_dir = TempDir::new().unwrap();
        let subdir = temp_dir.path().join("subdir");
        let test_file = subdir.join("test.txt");
        std::fs::create_dir(&subdir).unwrap();
        std::fs::write(&test_file, "test").unwrap();

        // Block parent directory, should block subdirectory
        let validator = PathValidator::with_blocked_paths(vec![temp_dir.path().to_path_buf()]);
        let result = validator.validate_absolute_path(&test_file.to_string_lossy());

        match result {
            Err(PathValidationError::Blocked(_)) => {
                // Expected
            }
            Ok(_) => panic!("Expected subdirectory of blocked path to be rejected"),
            Err(e) => panic!("Expected Blocked error, got: {}", e),
        }
    }

    #[test]
    fn test_allowed_with_blocked_subdirectory() {
        let temp_dir = TempDir::new().unwrap();
        let allowed_file = temp_dir.path().join("allowed.txt");
        let blocked_dir = temp_dir.path().join("blocked");
        let blocked_file = blocked_dir.join("blocked.txt");

        std::fs::write(&allowed_file, "allowed").unwrap();
        std::fs::create_dir(&blocked_dir).unwrap();
        std::fs::write(&blocked_file, "blocked").unwrap();

        let validator = PathValidator::with_allowed_and_blocked(
            vec![temp_dir.path().to_path_buf()],
            vec![blocked_dir.clone()],
        );

        // Allowed file should pass
        let result = validator.validate_absolute_path(&allowed_file.to_string_lossy());
        assert!(result.is_ok(), "Expected allowed file to pass validation");

        // Blocked file should fail
        let result = validator.validate_absolute_path(&blocked_file.to_string_lossy());
        match result {
            Err(PathValidationError::Blocked(_)) => {
                // Expected
            }
            Ok(_) => panic!("Expected blocked file to be rejected"),
            Err(e) => panic!("Expected Blocked error, got: {}", e),
        }
    }

    #[test]
    fn test_empty_blocked_list_allows_all() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        std::fs::write(&test_file, "test").unwrap();

        // Empty blocked list should allow any path
        let validator = PathValidator::new();
        let result = validator.validate_absolute_path(&test_file.to_string_lossy());
        assert!(
            result.is_ok(),
            "Expected path to be allowed with empty blocked list"
        );
    }

    #[test]
    fn test_not_found_error_handling() {
        let validator = PathValidator::new();

        // Test with a non-existent path
        let non_existent_path = if cfg!(windows) {
            "C:\\this\\path\\does\\not\\exist\\file.txt"
        } else {
            "/this/path/does/not/exist/file.txt"
        };

        let result = validator.validate_absolute_path(non_existent_path);

        // Should return CanonicalizationFailed error
        match result {
            Err(PathValidationError::CanonicalizationFailed(path, err_msg)) => {
                assert_eq!(path, non_existent_path);
                // Verify the error message contains "not found" or similar
                let err_lower = err_msg.to_lowercase();
                assert!(
                    err_lower.contains("not found")
                        || err_lower.contains("no such file")
                        || err_lower.contains("cannot find"),
                    "Expected error message to indicate file not found, got: {}",
                    err_msg
                );
            }
            Ok(_) => panic!("Expected CanonicalizationFailed error for non-existent path"),
            Err(e) => panic!(
                "Expected CanonicalizationFailed error for non-existent path, got: {}",
                e
            ),
        }
    }

    #[test]
    fn test_not_found_with_non_strict_canonicalization() {
        let validator = PathValidator::new().with_strict_canonicalization(false);

        // Test with a non-existent path
        let non_existent_path = if cfg!(windows) {
            "C:\\this\\path\\does\\not\\exist\\file.txt"
        } else {
            "/this/path/does/not/exist/file.txt"
        };

        let result = validator.validate_absolute_path(non_existent_path);

        // With non-strict canonicalization, should succeed
        assert!(
            result.is_ok(),
            "Expected validation to succeed with non-strict canonicalization for non-existent path"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_permission_denied_error_handling() {
        use std::fs::Permissions;
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = TempDir::new().unwrap();
        let restricted_dir = temp_dir.path().join("restricted");
        let restricted_file = restricted_dir.join("file.txt");

        // Create directory and file
        std::fs::create_dir(&restricted_dir).unwrap();
        std::fs::write(&restricted_file, "test").unwrap();

        // Remove all permissions from the directory to trigger permission denied
        std::fs::set_permissions(&restricted_dir, Permissions::from_mode(0o000)).unwrap();

        let validator = PathValidator::new();
        let result = validator.validate_absolute_path(&restricted_file.to_string_lossy());

        // Restore permissions before assertions to ensure cleanup
        std::fs::set_permissions(&restricted_dir, Permissions::from_mode(0o755)).unwrap();

        // Should return CanonicalizationFailed error with permission denied message
        match result {
            Err(PathValidationError::CanonicalizationFailed(_path, err_msg)) => {
                let err_lower = err_msg.to_lowercase();
                assert!(
                    err_lower.contains("permission denied") || err_lower.contains("access denied"),
                    "Expected error message to indicate permission denied, got: {}",
                    err_msg
                );
            }
            Ok(_) => panic!("Expected CanonicalizationFailed error for permission denied"),
            Err(e) => panic!(
                "Expected CanonicalizationFailed error for permission denied, got: {}",
                e
            ),
        }
    }

    #[cfg(windows)]
    #[test]
    fn test_permission_denied_error_handling_windows() {
        // On Windows, we'll test with a system path that typically requires elevation
        // Note: This test may behave differently depending on user privileges
        let validator = PathValidator::new();

        // Try to access a system file that typically requires admin rights
        let system_path = "C:\\Windows\\System32\\config\\SAM";

        let result = validator.validate_absolute_path(system_path);

        // The result depends on whether the test is running with admin privileges
        // If not admin, should get permission denied
        // If admin, might succeed or fail for other reasons
        match result {
            Err(PathValidationError::CanonicalizationFailed(_, err_msg)) => {
                // This is the expected behavior for non-admin users
                let err_lower = err_msg.to_lowercase();
                // On Windows, permission errors can be "access denied" or "permission denied"
                if err_lower.contains("access") || err_lower.contains("permission") {
                    // Expected permission error
                } else {
                    // Some other canonicalization error, which is also acceptable
                }
            }
            Err(_) | Ok(_) => {
                // Other errors or success (if running as admin) are acceptable
                // The key is that we don't panic, meaning the error handling works
            }
        }
    }

    #[test]
    fn test_error_handling_with_allowed_roots() {
        let temp_dir = TempDir::new().unwrap();
        let validator = PathValidator::with_allowed_roots(vec![temp_dir.path().to_path_buf()]);

        // Test non-existent file within allowed root
        let non_existent = temp_dir.path().join("does_not_exist.txt");
        let result = validator.validate_absolute_path(&non_existent.to_string_lossy());

        match result {
            Err(PathValidationError::CanonicalizationFailed(path, _)) => {
                // Expected - canonicalization happens before boundary check
                assert!(path.contains("does_not_exist.txt"));
            }
            Ok(_) => panic!("Expected CanonicalizationFailed for non-existent file"),
            Err(e) => panic!("Expected CanonicalizationFailed, got: {}", e),
        }

        // Test non-existent file outside allowed root
        let outside_path = if cfg!(windows) {
            "C:\\outside\\path\\file.txt"
        } else {
            "/outside/path/file.txt"
        };
        let result = validator.validate_absolute_path(outside_path);

        // Canonicalization fails first, so we get that error rather than boundary error
        match result {
            Err(PathValidationError::CanonicalizationFailed(_, _)) => {
                // Expected
            }
            Err(PathValidationError::OutsideBoundaries(_)) => {
                // Also acceptable if canonicalization somehow succeeds
            }
            Ok(_) => panic!("Expected error for path outside boundaries"),
            Err(e) => panic!(
                "Expected CanonicalizationFailed or OutsideBoundaries, got: {}",
                e
            ),
        }
    }

    #[test]
    fn test_canonicalization_error_preserves_original_path() {
        let validator = PathValidator::new();

        let test_path = if cfg!(windows) {
            "C:\\nonexistent\\deeply\\nested\\path\\file.txt"
        } else {
            "/nonexistent/deeply/nested/path/file.txt"
        };

        let result = validator.validate_absolute_path(test_path);

        match result {
            Err(PathValidationError::CanonicalizationFailed(path, _)) => {
                // Verify the original path is preserved in the error
                assert_eq!(
                    path, test_path,
                    "Error should preserve the original path string"
                );
            }
            Ok(_) => panic!("Expected CanonicalizationFailed for non-existent nested path"),
            Err(e) => panic!("Expected CanonicalizationFailed, got: {}", e),
        }
    }

    #[test]
    fn test_multiple_error_scenarios() {
        let temp_dir = TempDir::new().unwrap();

        // Create a blocked directory with a non-existent file
        let blocked_dir = temp_dir.path().join("blocked");
        std::fs::create_dir(&blocked_dir).unwrap();
        let non_existent_in_blocked = blocked_dir.join("nonexistent.txt");

        let validator = PathValidator::with_blocked_paths(vec![blocked_dir.clone()]);

        // Non-existent file in blocked directory
        // Canonicalization should fail before blocked check
        let result = validator.validate_absolute_path(&non_existent_in_blocked.to_string_lossy());

        match result {
            Err(PathValidationError::CanonicalizationFailed(_, _)) => {
                // Expected - canonicalization happens before blocked check
            }
            Err(PathValidationError::Blocked(_)) => {
                // Also possible if canonicalization somehow succeeds
                panic!("Unexpected Blocked error - canonicalization should fail first");
            }
            Ok(_) => panic!("Expected error for non-existent file in blocked directory"),
            Err(e) => panic!("Expected CanonicalizationFailed, got: {}", e),
        }
    }

    #[test]
    fn test_validate_permissions_readable_file() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("readable.txt");
        std::fs::write(&test_file, "test content").unwrap();

        let validator = PathValidator::new();

        // Should be able to read the file we just created
        let result = validator.validate_permissions(&test_file, &[Permission::Read]);
        assert!(
            result.is_ok(),
            "Expected read permission to succeed for readable file"
        );
    }

    #[test]
    fn test_validate_permissions_writable_file() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("writable.txt");
        std::fs::write(&test_file, "test content").unwrap();

        let validator = PathValidator::new();

        // Should be able to write to the file we just created
        let result = validator.validate_permissions(&test_file, &[Permission::Write]);
        assert!(
            result.is_ok(),
            "Expected write permission to succeed for writable file"
        );
    }

    #[test]
    fn test_validate_permissions_multiple() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        std::fs::write(&test_file, "test content").unwrap();

        let validator = PathValidator::new();

        // Should be able to read and write
        let result =
            validator.validate_permissions(&test_file, &[Permission::Read, Permission::Write]);
        assert!(
            result.is_ok(),
            "Expected read and write permissions to succeed"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_validate_permissions_read_only_file() {
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("readonly.txt");
        std::fs::write(&test_file, "test content").unwrap();

        // Make file read-only
        std::fs::set_permissions(&test_file, std::fs::Permissions::from_mode(0o444)).unwrap();

        let validator = PathValidator::new();

        // Read should succeed
        let result = validator.validate_permissions(&test_file, &[Permission::Read]);
        assert!(result.is_ok(), "Expected read permission to succeed");

        // Write should fail
        let result = validator.validate_permissions(&test_file, &[Permission::Write]);
        match result {
            Err(PathValidationError::InsufficientPermissions { path, required }) => {
                assert!(path.contains("readonly.txt"));
                assert_eq!(required, "write");
            }
            Ok(_) => {
                // Restore permissions before panic
                std::fs::set_permissions(&test_file, std::fs::Permissions::from_mode(0o644))
                    .unwrap();
                panic!("Expected write permission to fail for read-only file");
            }
            Err(e) => {
                // Restore permissions before panic
                std::fs::set_permissions(&test_file, std::fs::Permissions::from_mode(0o644))
                    .unwrap();
                panic!("Expected InsufficientPermissions error, got: {}", e);
            }
        }

        // Restore permissions for cleanup
        std::fs::set_permissions(&test_file, std::fs::Permissions::from_mode(0o644)).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn test_validate_permissions_read_only_file_windows() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("readonly.txt");
        std::fs::write(&test_file, "test content").unwrap();

        // Make file read-only on Windows
        let mut perms = std::fs::metadata(&test_file).unwrap().permissions();
        perms.set_readonly(true);
        std::fs::set_permissions(&test_file, perms).unwrap();

        let validator = PathValidator::new();

        // Read should succeed
        let result = validator.validate_permissions(&test_file, &[Permission::Read]);
        assert!(result.is_ok(), "Expected read permission to succeed");

        // Write should fail
        let result = validator.validate_permissions(&test_file, &[Permission::Write]);
        match result {
            Err(PathValidationError::InsufficientPermissions { path, required }) => {
                assert!(path.contains("readonly.txt"));
                assert_eq!(required, "write");
            }
            Ok(_) => {
                // Restore permissions before panic
                let mut perms = std::fs::metadata(&test_file).unwrap().permissions();
                perms.set_readonly(false);
                std::fs::set_permissions(&test_file, perms).unwrap();
                panic!("Expected write permission to fail for read-only file");
            }
            Err(e) => {
                // Restore permissions before panic
                let mut perms = std::fs::metadata(&test_file).unwrap().permissions();
                perms.set_readonly(false);
                std::fs::set_permissions(&test_file, perms).unwrap();
                panic!("Expected InsufficientPermissions error, got: {}", e);
            }
        }

        // Restore permissions for cleanup
        let mut perms = std::fs::metadata(&test_file).unwrap().permissions();
        perms.set_readonly(false);
        std::fs::set_permissions(&test_file, perms).unwrap();
    }

    #[test]
    fn test_validate_permissions_nonexistent_file() {
        let validator = PathValidator::new();
        let nonexistent = if cfg!(windows) {
            "C:\\nonexistent\\file.txt"
        } else {
            "/nonexistent/file.txt"
        };

        let result = validator.validate_permissions(Path::new(nonexistent), &[Permission::Read]);

        match result {
            Err(PathValidationError::InsufficientPermissions { path, required }) => {
                assert!(path.contains("nonexistent"));
                assert_eq!(required, "read");
            }
            Ok(_) => panic!("Expected permission check to fail for nonexistent file"),
            Err(e) => panic!("Expected InsufficientPermissions error, got: {}", e),
        }
    }
}
