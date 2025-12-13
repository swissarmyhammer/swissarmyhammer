use crate::size_validator::{SizeValidationError, SizeValidator};
use std::path::{Path, PathBuf};
use thiserror::Error;

/// ACP-compliant path validator with comprehensive security and platform validation
#[derive(Debug, Clone)]
pub struct PathValidator {
    /// Allowed root directories for file operations
    allowed_roots: Vec<PathBuf>,
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

    #[error("Invalid path format: {0}")]
    InvalidFormat(String),

    #[error("Path contains null bytes")]
    NullBytesInPath,

    #[error("Empty path provided")]
    EmptyPath,
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
    /// 5. **Boundary validation** - Ensures path is within allowed roots (if configured)
    ///
    /// The validation uses a layered approach where the string-based check provides
    /// fast rejection of obviously malicious paths, while the component-based check
    /// serves as the canonical security validation after normalization.
    pub fn validate_absolute_path(&self, path_str: &str) -> Result<PathBuf, PathValidationError> {
        // Check for empty path
        if path_str.is_empty() {
            return Err(PathValidationError::EmptyPath);
        }

        // Check path length
        self.size_validator.validate_path_length(path_str)?;

        // Check for null bytes
        if path_str.contains('\0') {
            return Err(PathValidationError::NullBytesInPath);
        }

        // Parse path
        let path = PathBuf::from(path_str);

        // Check if path is absolute using platform-specific logic
        if !self.is_absolute_path(&path) {
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
                    return Err(PathValidationError::PathTraversalAttempt(
                        path.display().to_string(),
                    ));
                }
                std::path::Component::CurDir => {
                    return Err(PathValidationError::RelativeComponent(
                        path.display().to_string(),
                    ));
                }
                _ => {}
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

        Err(PathValidationError::OutsideBoundaries(
            path.to_string_lossy().to_string(),
        ))
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
}
