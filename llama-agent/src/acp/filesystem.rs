//! File operations for ACP
//!
//! This module handles file system operations exposed via ACP protocol.
//! It implements comprehensive security validation to prevent path traversal
//! and unauthorized file access.

use agent_client_protocol::{
    ReadTextFileRequest, ReadTextFileResponse, WriteTextFileRequest, WriteTextFileResponse,
};
use std::path::{Path, PathBuf};
use thiserror::Error;

use super::config::FilesystemSettings;
use super::session::AcpSessionState;

/// Errors that can occur during file system operations
#[derive(Debug, Error)]
pub enum FilesystemError {
    /// Path must be absolute
    #[error("Path must be absolute: {0}")]
    RelativePath(String),

    /// Path traversal attack detected
    #[error("Path traversal detected: {0}")]
    PathTraversal(String),

    /// Path is not in the allowed list
    #[error("Path not allowed: {0}")]
    NotAllowed(String),

    /// Path is in the blocked list
    #[error("Path is blocked: {0}")]
    Blocked(String),

    /// File exceeds maximum size limit
    #[error("File too large: {0} bytes (max: {1})")]
    FileTooLarge(u64, u64),

    /// Standard IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

impl From<FilesystemError> for agent_client_protocol::Error {
    fn from(error: FilesystemError) -> Self {
        match error {
            // Security violations are invalid params
            FilesystemError::RelativePath(path) => agent_client_protocol::Error::invalid_params()
                .data(format!("Path must be absolute: {}", path)),
            FilesystemError::PathTraversal(path) => agent_client_protocol::Error::invalid_params()
                .data(format!("Path traversal detected: {}", path)),
            FilesystemError::NotAllowed(path) => agent_client_protocol::Error::invalid_params()
                .data(format!("Path not allowed: {}", path)),
            FilesystemError::Blocked(path) => agent_client_protocol::Error::invalid_params()
                .data(format!("Path is blocked: {}", path)),
            FilesystemError::FileTooLarge(size, max) => {
                agent_client_protocol::Error::invalid_params().data(format!(
                    "File too large: {} bytes (max: {} bytes)",
                    size, max
                ))
            }

            // IO errors need more granular handling
            FilesystemError::Io(io_error) => match io_error.kind() {
                std::io::ErrorKind::NotFound => agent_client_protocol::Error::invalid_params()
                    .data(format!("File not found: {}", io_error)),
                std::io::ErrorKind::PermissionDenied => {
                    agent_client_protocol::Error::invalid_params()
                        .data(format!("Permission denied: {}", io_error))
                }
                std::io::ErrorKind::AlreadyExists => agent_client_protocol::Error::invalid_params()
                    .data(format!("File already exists: {}", io_error)),
                std::io::ErrorKind::InvalidInput | std::io::ErrorKind::InvalidData => {
                    agent_client_protocol::Error::invalid_params()
                        .data(format!("Invalid input: {}", io_error))
                }
                _ => agent_client_protocol::Error::internal_error()
                    .data(format!("IO error: {}", io_error)),
            },
        }
    }
}

/// Validates file paths against security policies
///
/// PathValidator enforces security restrictions on file system access:
/// - Only absolute paths are allowed
/// - Path traversal attempts are blocked via canonicalization
/// - Paths can be restricted to an allowed list
/// - Paths can be explicitly blocked
pub struct PathValidator {
    /// List of allowed path prefixes (empty = allow all)
    allowed_paths: Vec<PathBuf>,
    /// List of blocked path prefixes
    blocked_paths: Vec<PathBuf>,
}

impl PathValidator {
    /// Create a new path validator with allowed and blocked path lists
    ///
    /// # Arguments
    ///
    /// * `allowed_paths` - Paths that are allowed (empty = allow all except blocked)
    /// * `blocked_paths` - Paths that are explicitly blocked
    ///
    /// All paths are canonicalized if they exist, or normalized if they don't.
    ///
    /// # Panics
    ///
    /// Panics if any of the provided paths cannot be normalized
    pub fn new(allowed_paths: Vec<PathBuf>, blocked_paths: Vec<PathBuf>) -> Self {
        // Canonicalize/normalize all allowed and blocked paths upfront
        let allowed_paths = allowed_paths
            .into_iter()
            .map(|p| {
                if p.exists() {
                    p.canonicalize().unwrap_or_else(|_| {
                        panic!("Failed to canonicalize allowed path: {}", p.display())
                    })
                } else {
                    normalize_path(&p).unwrap_or_else(|_| {
                        panic!("Failed to normalize allowed path: {}", p.display())
                    })
                }
            })
            .collect();

        let blocked_paths = blocked_paths
            .into_iter()
            .map(|p| {
                if p.exists() {
                    p.canonicalize().unwrap_or_else(|_| {
                        panic!("Failed to canonicalize blocked path: {}", p.display())
                    })
                } else {
                    normalize_path(&p).unwrap_or_else(|_| {
                        panic!("Failed to normalize blocked path: {}", p.display())
                    })
                }
            })
            .collect();

        Self {
            allowed_paths,
            blocked_paths,
        }
    }

    /// Validate a path against security policies
    ///
    /// This method performs comprehensive security checks:
    /// 1. Ensures the path is absolute
    /// 2. Canonicalizes to resolve symlinks and `..` components
    /// 3. Checks against blocked list
    /// 4. Checks against allowed list (if configured)
    ///
    /// # Arguments
    ///
    /// * `path` - The path to validate
    ///
    /// # Returns
    ///
    /// The canonicalized path if valid
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Path is relative
    /// - Path doesn't exist or cannot be canonicalized (security measure)
    /// - Path is in blocked list
    /// - Path is not in allowed list (when allowed list is configured)
    pub fn validate(&self, path: &Path) -> Result<PathBuf, FilesystemError> {
        // Must be absolute
        if !path.is_absolute() {
            return Err(FilesystemError::RelativePath(path.display().to_string()));
        }

        // Canonicalize to resolve symlinks and .. components
        // This also ensures the path exists, which is important for security
        let normalized = path
            .canonicalize()
            .map_err(|_| FilesystemError::PathTraversal(path.display().to_string()))?;

        // Check blocked list first
        for blocked in &self.blocked_paths {
            if normalized.starts_with(blocked) {
                return Err(FilesystemError::Blocked(normalized.display().to_string()));
            }
        }

        // Check allowed list (if not empty)
        if !self.allowed_paths.is_empty() {
            let mut allowed = false;
            for allowed_path in &self.allowed_paths {
                if normalized.starts_with(allowed_path) {
                    allowed = true;
                    break;
                }
            }

            if !allowed {
                return Err(FilesystemError::NotAllowed(
                    normalized.display().to_string(),
                ));
            }
        }

        Ok(normalized)
    }
}

/// Normalize a path by resolving `.` and `..` components
///
/// This is used when a path doesn't exist yet but we need to validate it.
/// It prevents path traversal attacks while allowing legitimate paths.
fn normalize_path(path: &Path) -> Result<PathBuf, FilesystemError> {
    let mut components = Vec::new();

    for component in path.components() {
        match component {
            std::path::Component::Prefix(prefix) => {
                components.clear();
                components.push(std::path::Component::Prefix(prefix));
            }
            std::path::Component::RootDir => {
                components.clear();
                components.push(std::path::Component::RootDir);
            }
            std::path::Component::CurDir => {
                // Skip . components
            }
            std::path::Component::ParentDir => {
                // Remove the last normal component if present
                if let Some(last) = components.last() {
                    if matches!(last, std::path::Component::Normal(_)) {
                        components.pop();
                    } else {
                        // Can't go up from root or prefix
                        return Err(FilesystemError::PathTraversal(path.display().to_string()));
                    }
                } else {
                    // No components to go up from
                    return Err(FilesystemError::PathTraversal(path.display().to_string()));
                }
            }
            std::path::Component::Normal(name) => {
                components.push(std::path::Component::Normal(name));
            }
        }
    }

    let mut normalized = PathBuf::new();
    for component in components {
        normalized.push(component);
    }

    Ok(normalized)
}

/// File system operations handler for ACP
///
/// This struct provides secure file system operations with path validation
/// and size limits. All operations are validated against security policies
/// and client capabilities.
pub struct FilesystemOperations {
    validator: PathValidator,
    max_file_size: u64,
}

impl FilesystemOperations {
    /// Create a new filesystem operations handler
    ///
    /// # Arguments
    ///
    /// * `settings` - Filesystem configuration with path restrictions and limits
    pub fn new(settings: &FilesystemSettings) -> Self {
        Self {
            validator: PathValidator::new(
                settings.allowed_paths.clone(),
                settings.blocked_paths.clone(),
            ),
            max_file_size: settings.max_file_size,
        }
    }

    /// Read a text file with security checks and capability validation
    ///
    /// This method performs the following checks:
    /// 1. Verifies client has read_text_file capability
    /// 2. Validates the file path against security policies
    /// 3. Checks file size against configured limits
    /// 4. Reads the file content as UTF-8 text
    ///
    /// # Arguments
    ///
    /// * `session` - Current ACP session state with client capabilities
    /// * `req` - Read request containing the file path
    ///
    /// # Returns
    ///
    /// Response containing file content on success
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Client doesn't have read_text_file capability
    /// - Path validation fails (security violation)
    /// - File exceeds size limit
    /// - File doesn't exist or cannot be read
    /// - File content is not valid UTF-8
    pub async fn read_text_file(
        &self,
        session: &AcpSessionState,
        req: ReadTextFileRequest,
    ) -> Result<ReadTextFileResponse, FilesystemError> {
        // Check client capability
        if !session.client_capabilities.fs.read_text_file {
            return Err(FilesystemError::NotAllowed(
                "Client does not support read_text_file".to_string(),
            ));
        }

        // Validate path
        let path = Path::new(&req.path);
        let canonical = self.validator.validate(path)?;

        // Check file size
        let metadata = tokio::fs::metadata(&canonical).await?;
        if metadata.len() > self.max_file_size {
            return Err(FilesystemError::FileTooLarge(
                metadata.len(),
                self.max_file_size,
            ));
        }

        // Read file
        let content = tokio::fs::read_to_string(&canonical).await?;

        tracing::info!("Read {} bytes from {}", content.len(), canonical.display());

        Ok(ReadTextFileResponse::new(content))
    }

    /// Write a text file with security checks and capability validation
    ///
    /// This method performs atomic file writes with the following checks:
    /// 1. Verifies client has write_text_file capability
    /// 2. Validates the file path against security policies
    /// 3. Validates the parent directory exists
    /// 4. Writes content atomically using a temporary file
    ///
    /// Atomic writes prevent corruption by writing to a temporary file first,
    /// then renaming it to the target path. This ensures the file is never
    /// left in a partially-written state.
    ///
    /// # Arguments
    ///
    /// * `session` - Current ACP session state with client capabilities
    /// * `req` - Write request containing the file path and content
    ///
    /// # Returns
    ///
    /// Empty response on success
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Client doesn't have write_text_file capability
    /// - Path validation fails (security violation)
    /// - Parent directory doesn't exist
    /// - Write or rename operation fails
    pub async fn write_text_file(
        &self,
        session: &AcpSessionState,
        req: WriteTextFileRequest,
    ) -> Result<WriteTextFileResponse, FilesystemError> {
        // Check client capability
        if !session.client_capabilities.fs.write_text_file {
            return Err(FilesystemError::NotAllowed(
                "Client does not support write_text_file".to_string(),
            ));
        }

        // Validate path
        let path = Path::new(&req.path);

        // For writes, we need to validate the parent directory exists
        let parent = path
            .parent()
            .ok_or_else(|| FilesystemError::NotAllowed("Invalid path".to_string()))?;

        let canonical_parent = self.validator.validate(parent)?;
        let canonical = canonical_parent.join(
            path.file_name()
                .ok_or_else(|| FilesystemError::NotAllowed("Invalid filename".to_string()))?,
        );

        // Atomic write using temporary file
        let temp_path = canonical.with_extension("tmp");
        tokio::fs::write(&temp_path, &req.content).await?;
        tokio::fs::rename(&temp_path, &canonical).await?;

        tracing::info!(
            "Wrote {} bytes to {}",
            req.content.len(),
            canonical.display()
        );

        Ok(WriteTextFileResponse::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_reject_relative_path() {
        let validator = PathValidator::new(vec![], vec![]);
        let result = validator.validate(Path::new("relative/path"));
        assert!(matches!(result, Err(FilesystemError::RelativePath(_))));
    }

    #[test]
    fn test_accept_absolute_path() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        fs::write(&test_file, "test").unwrap();

        let validator = PathValidator::new(vec![], vec![]);
        let result = validator.validate(&test_file);
        assert!(result.is_ok());
    }

    #[test]
    fn test_blocked_path() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("blocked.txt");
        fs::write(&test_file, "test").unwrap();

        let validator = PathValidator::new(vec![], vec![temp_dir.path().to_path_buf()]);
        let result = validator.validate(&test_file);
        assert!(matches!(result, Err(FilesystemError::Blocked(_))));
    }

    #[test]
    fn test_allowed_path() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("allowed.txt");
        fs::write(&test_file, "test").unwrap();

        let validator = PathValidator::new(vec![temp_dir.path().to_path_buf()], vec![]);
        let result = validator.validate(&test_file);
        assert!(result.is_ok());
    }

    #[test]
    fn test_not_in_allowed_list() {
        let temp_dir = TempDir::new().unwrap();
        let allowed_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        fs::write(&test_file, "test").unwrap();

        let validator = PathValidator::new(vec![allowed_dir.path().to_path_buf()], vec![]);
        let result = validator.validate(&test_file);
        assert!(matches!(result, Err(FilesystemError::NotAllowed(_))));
    }

    #[test]
    fn test_path_traversal_via_symlink() {
        let temp_dir = TempDir::new().unwrap();
        let allowed_dir = temp_dir.path().join("allowed");
        let outside_dir = temp_dir.path().join("outside");
        let target_file = outside_dir.join("secret.txt");

        fs::create_dir(&allowed_dir).unwrap();
        fs::create_dir(&outside_dir).unwrap();
        fs::write(&target_file, "secret").unwrap();

        // Create symlink inside allowed directory pointing outside
        let symlink_path = allowed_dir.join("link");

        #[cfg(unix)]
        std::os::unix::fs::symlink(&target_file, &symlink_path).unwrap();

        #[cfg(windows)]
        std::os::windows::fs::symlink_file(&target_file, &symlink_path).unwrap();

        // Validator should catch that canonical path is outside allowed directory
        let validator = PathValidator::new(vec![allowed_dir.clone()], vec![]);
        let result = validator.validate(&symlink_path);

        // The symlink should resolve to outside the allowed directory
        assert!(matches!(result, Err(FilesystemError::NotAllowed(_))));
    }

    #[test]
    fn test_path_traversal_via_dotdot() {
        let temp_dir = TempDir::new().unwrap();
        let allowed_dir = temp_dir.path().join("allowed");
        let target_file = temp_dir.path().join("secret.txt");

        fs::create_dir(&allowed_dir).unwrap();
        fs::write(&target_file, "secret").unwrap();

        // Try to access file outside allowed directory using ..
        let traversal_path = allowed_dir.join("..").join("secret.txt");

        let validator = PathValidator::new(vec![allowed_dir.clone()], vec![]);
        let result = validator.validate(&traversal_path);

        // Should be rejected because canonical path is outside allowed directory
        assert!(matches!(result, Err(FilesystemError::NotAllowed(_))));
    }

    #[test]
    fn test_nonexistent_path_rejected() {
        let nonexistent = PathBuf::from("/this/path/does/not/exist/file.txt");

        let validator = PathValidator::new(vec![], vec![]);
        let result = validator.validate(&nonexistent);

        // Should fail during canonicalization
        assert!(matches!(result, Err(FilesystemError::PathTraversal(_))));
    }

    #[test]
    fn test_empty_allowed_list_allows_all() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        fs::write(&test_file, "test").unwrap();

        // Empty allowed list should allow any path (except blocked)
        let validator = PathValidator::new(vec![], vec![]);
        let result = validator.validate(&test_file);
        assert!(result.is_ok());
    }

    #[test]
    fn test_blocked_takes_precedence_over_allowed() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        fs::write(&test_file, "test").unwrap();

        // Both in allowed and blocked - blocked should win
        let validator = PathValidator::new(
            vec![temp_dir.path().to_path_buf()],
            vec![temp_dir.path().to_path_buf()],
        );
        let result = validator.validate(&test_file);
        assert!(matches!(result, Err(FilesystemError::Blocked(_))));
    }

    #[test]
    fn test_subdirectory_of_allowed() {
        let temp_dir = TempDir::new().unwrap();
        let subdir = temp_dir.path().join("subdir");
        let test_file = subdir.join("test.txt");
        fs::create_dir(&subdir).unwrap();
        fs::write(&test_file, "test").unwrap();

        // Allow parent directory, should allow subdirectory
        let validator = PathValidator::new(vec![temp_dir.path().to_path_buf()], vec![]);
        let result = validator.validate(&test_file);
        assert!(result.is_ok());
    }

    #[test]
    fn test_subdirectory_of_blocked() {
        let temp_dir = TempDir::new().unwrap();
        let subdir = temp_dir.path().join("subdir");
        let test_file = subdir.join("test.txt");
        fs::create_dir(&subdir).unwrap();
        fs::write(&test_file, "test").unwrap();

        // Block parent directory, should block subdirectory
        let validator = PathValidator::new(vec![], vec![temp_dir.path().to_path_buf()]);
        let result = validator.validate(&test_file);
        assert!(matches!(result, Err(FilesystemError::Blocked(_))));
    }

    #[test]
    fn test_filesystem_structures_serialization_camelcase() {
        use agent_client_protocol::SessionId;
        use std::path::PathBuf;

        // Test ReadTextFileRequest
        let session_id = SessionId::new("test-session".to_string());
        let read_req =
            ReadTextFileRequest::new(session_id.clone(), PathBuf::from("/path/to/file.txt"));
        let read_req_json = serde_json::to_value(&read_req).unwrap();
        let read_req_obj = read_req_json.as_object().unwrap();

        // Should use 'path', not something else - verify the structure exists
        assert!(
            read_req_obj.contains_key("path"),
            "ReadTextFileRequest should have 'path' field. Found keys: {:?}",
            read_req_obj.keys()
        );

        // Should use 'sessionId' in camelCase
        assert!(
            read_req_obj.contains_key("sessionId"),
            "ReadTextFileRequest should have 'sessionId' field in camelCase. Found keys: {:?}",
            read_req_obj.keys()
        );

        // Test ReadTextFileResponse
        let read_resp = ReadTextFileResponse::new("file content".to_string());
        let read_resp_json = serde_json::to_value(&read_resp).unwrap();
        let read_resp_obj = read_resp_json.as_object().unwrap();

        // Should use 'content', not something else - verify the structure exists
        assert!(
            read_resp_obj.contains_key("content"),
            "ReadTextFileResponse should have 'content' field. Found keys: {:?}",
            read_resp_obj.keys()
        );

        // Test WriteTextFileRequest
        let write_req = WriteTextFileRequest::new(
            session_id.clone(),
            PathBuf::from("/path/to/file.txt"),
            "new content".to_string(),
        );
        let write_req_json = serde_json::to_value(&write_req).unwrap();
        let write_req_obj = write_req_json.as_object().unwrap();

        // Should use 'path' and 'content'
        assert!(
            write_req_obj.contains_key("path"),
            "WriteTextFileRequest should have 'path' field. Found keys: {:?}",
            write_req_obj.keys()
        );
        assert!(
            write_req_obj.contains_key("content"),
            "WriteTextFileRequest should have 'content' field. Found keys: {:?}",
            write_req_obj.keys()
        );

        // Should use 'sessionId' in camelCase
        assert!(
            write_req_obj.contains_key("sessionId"),
            "WriteTextFileRequest should have 'sessionId' field in camelCase. Found keys: {:?}",
            write_req_obj.keys()
        );

        // Test WriteTextFileResponse - typically empty or minimal
        let write_resp = WriteTextFileResponse::new();
        let write_resp_json = serde_json::to_value(&write_resp).unwrap();

        // Verify it serializes without error (may be empty object or have standard fields)
        assert!(
            write_resp_json.is_object() || write_resp_json.is_null(),
            "WriteTextFileResponse should serialize to object or null"
        );
    }

    #[test]
    fn test_filesystem_error_from_trait() {
        // Test that From trait converts FilesystemError to agent_client_protocol::Error correctly

        // Test RelativePath
        let error: agent_client_protocol::Error =
            FilesystemError::RelativePath("relative/path".to_string()).into();
        assert_eq!(error.code, agent_client_protocol::ErrorCode::InvalidParams);
        assert!(error
            .data
            .as_ref()
            .unwrap()
            .as_str()
            .unwrap()
            .contains("Path must be absolute"));

        // Test PathTraversal
        let error: agent_client_protocol::Error =
            FilesystemError::PathTraversal("/etc/../../../etc/passwd".to_string()).into();
        assert_eq!(error.code, agent_client_protocol::ErrorCode::InvalidParams);
        assert!(error
            .data
            .as_ref()
            .unwrap()
            .as_str()
            .unwrap()
            .contains("Path traversal detected"));

        // Test NotAllowed
        let error: agent_client_protocol::Error =
            FilesystemError::NotAllowed("/blocked/path".to_string()).into();
        assert_eq!(error.code, agent_client_protocol::ErrorCode::InvalidParams);
        assert!(error
            .data
            .as_ref()
            .unwrap()
            .as_str()
            .unwrap()
            .contains("Path not allowed"));

        // Test Blocked
        let error: agent_client_protocol::Error =
            FilesystemError::Blocked("/blocked/path".to_string()).into();
        assert_eq!(error.code, agent_client_protocol::ErrorCode::InvalidParams);
        assert!(error
            .data
            .as_ref()
            .unwrap()
            .as_str()
            .unwrap()
            .contains("Path is blocked"));

        // Test FileTooLarge
        let error: agent_client_protocol::Error =
            FilesystemError::FileTooLarge(1000000, 500000).into();
        assert_eq!(error.code, agent_client_protocol::ErrorCode::InvalidParams);
        assert!(error
            .data
            .as_ref()
            .unwrap()
            .as_str()
            .unwrap()
            .contains("File too large"));

        // Test IO errors - NotFound
        let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let error: agent_client_protocol::Error = FilesystemError::Io(io_error).into();
        assert_eq!(error.code, agent_client_protocol::ErrorCode::InvalidParams);
        assert!(error
            .data
            .as_ref()
            .unwrap()
            .as_str()
            .unwrap()
            .contains("File not found"));

        // Test IO errors - PermissionDenied
        let io_error =
            std::io::Error::new(std::io::ErrorKind::PermissionDenied, "permission denied");
        let error: agent_client_protocol::Error = FilesystemError::Io(io_error).into();
        assert_eq!(error.code, agent_client_protocol::ErrorCode::InvalidParams);
        assert!(error
            .data
            .as_ref()
            .unwrap()
            .as_str()
            .unwrap()
            .contains("Permission denied"));

        // Test IO errors - AlreadyExists
        let io_error = std::io::Error::new(std::io::ErrorKind::AlreadyExists, "file exists");
        let error: agent_client_protocol::Error = FilesystemError::Io(io_error).into();
        assert_eq!(error.code, agent_client_protocol::ErrorCode::InvalidParams);
        assert!(error
            .data
            .as_ref()
            .unwrap()
            .as_str()
            .unwrap()
            .contains("File already exists"));

        // Test IO errors - Other (should be internal error)
        let io_error = std::io::Error::other("unknown error");
        let error: agent_client_protocol::Error = FilesystemError::Io(io_error).into();
        assert_eq!(error.code, agent_client_protocol::ErrorCode::InternalError);
        assert!(error
            .data
            .as_ref()
            .unwrap()
            .as_str()
            .unwrap()
            .contains("IO error"));
    }
}
