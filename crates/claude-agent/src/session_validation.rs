//! Session setup validation functions for ACP compliance
//!
//! This module provides validation functions for session setup operations
//! ensuring all ACP requirements are met with comprehensive error handling.

use crate::session_errors::{SessionSetupError, SessionSetupResult};
use crate::url_validation;
use reqwest::Url;
use std::fs;
use std::path::Path;

/// Validate a working directory path for session setup
///
/// Performs comprehensive validation according to ACP requirements:
/// - Path must be absolute
/// - Directory must exist
/// - Directory must be accessible (readable and executable)
/// - Path must not contain invalid characters
/// - Network paths are not supported
pub fn validate_working_directory(path: &Path) -> SessionSetupResult<()> {
    // ACP requires comprehensive error handling for session setup:
    // 1. Clear, actionable error messages for clients
    // 2. Appropriate JSON-RPC error codes
    // 3. Structured error data for programmatic handling
    // 4. Graceful degradation where possible
    // 5. Proper cleanup of partial session state on failures

    // Check if path is absolute
    if !path.is_absolute() {
        return Err(SessionSetupError::WorkingDirectoryNotAbsolute {
            provided_path: path.to_path_buf(),
            requirement: "absolute_path".to_string(),
            example: if cfg!(windows) {
                "C:\\Users\\username\\project".to_string()
            } else {
                "/home/username/project".to_string()
            },
        });
    }

    // Check for network paths (Windows UNC paths or other network indicators)
    let path_str = path.to_string_lossy();
    if path_str.starts_with("\\\\") || path_str.starts_with("//") {
        return Err(SessionSetupError::WorkingDirectoryNetworkPath {
            path: path.to_path_buf(),
            suggestion: "Copy files to a local directory and use the local path instead"
                .to_string(),
        });
    }

    // Check for invalid characters
    let invalid_chars = find_invalid_path_characters(&path_str);
    if !invalid_chars.is_empty() {
        return Err(SessionSetupError::WorkingDirectoryInvalidPath {
            path: path.to_path_buf(),
            invalid_chars,
        });
    }

    // Check if directory exists
    if !path.exists() {
        return Err(SessionSetupError::WorkingDirectoryNotFound {
            path: path.to_path_buf(),
        });
    }

    // Check if it's actually a directory
    if !path.is_dir() {
        return Err(SessionSetupError::WorkingDirectoryNotFound {
            path: path.to_path_buf(),
        });
    }

    // Check permissions
    validate_directory_permissions(path)?;

    Ok(())
}

/// Validate directory permissions for session operations.
///
/// Probes readability via `read_dir` and traversability (execute permission)
/// via a side-effect-free `access(2)` check — never by mutating the
/// process-global CWD, which every concurrent thread doing relative-path I/O
/// or spawning a process would observe. The underlying OS error is preserved
/// as the returned error's `source` so callers can distinguish EACCES from
/// EIO, ELOOP, or a directory deleted between checks.
fn validate_directory_permissions(path: &Path) -> SessionSetupResult<()> {
    let denied = |source: std::io::Error| SessionSetupError::WorkingDirectoryPermissionDenied {
        path: path.to_path_buf(),
        required_permissions: vec!["read".to_string(), "execute".to_string()],
        source: std::sync::Arc::new(source),
    };

    fs::read_dir(path).map_err(&denied)?;
    probe_directory_traversal(path).map_err(denied)
}

/// Check that the process can traverse (search) `path` without side effects.
///
/// On Unix, execute permission on a directory grants traversal; `access(2)`
/// with `X_OK` asks the kernel directly, honoring the effective uid/gid and
/// ACLs that a raw permission-bit inspection would miss.
#[cfg(unix)]
fn probe_directory_traversal(path: &Path) -> std::io::Result<()> {
    use std::os::unix::ffi::OsStrExt;

    let c_path = std::ffi::CString::new(path.as_os_str().as_bytes())
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "path contains NUL"))?;
    // SAFETY: `c_path` is a valid NUL-terminated string that outlives the call.
    if unsafe { libc::access(c_path.as_ptr(), libc::X_OK) } == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

/// On Windows a successful `read_dir` already implies the directory is
/// traversable, so there is nothing further to probe.
#[cfg(not(unix))]
fn probe_directory_traversal(_path: &Path) -> std::io::Result<()> {
    Ok(())
}

/// Find invalid characters in a path string
fn find_invalid_path_characters(path_str: &str) -> Vec<String> {
    let mut invalid_chars = Vec::new();

    // Common invalid characters across platforms
    let forbidden_chars = if cfg!(windows) {
        // Windows has more restrictive path character rules
        vec!['<', '>', ':', '"', '|', '?', '*', '\0']
    } else {
        // Unix-like systems only forbid null character in paths
        vec!['\0']
    };

    for &forbidden in &forbidden_chars {
        if path_str.contains(forbidden) {
            invalid_chars.push(format!("'{}'", forbidden));
        }
    }

    // Check for control characters
    for ch in path_str.chars() {
        if ch.is_control() && ch != '\n' && ch != '\t' {
            invalid_chars.push(format!("control character U+{:04X}", ch as u32));
        }
    }

    invalid_chars.sort();
    invalid_chars.dedup();
    invalid_chars
}

/// Validate MCP server configuration before attempting connection
pub fn validate_mcp_server_config(
    server_config: &crate::config::McpServerConfig,
) -> SessionSetupResult<()> {
    match server_config {
        crate::config::McpServerConfig::Stdio(stdio_config) => {
            validate_mcp_stdio_config(stdio_config)
        }
        crate::config::McpServerConfig::Http(http_config) => validate_mcp_http_config(http_config),
        crate::config::McpServerConfig::Sse(sse_config) => validate_mcp_sse_config(sse_config),
    }
}

/// Validate STDIO MCP server configuration
fn validate_mcp_stdio_config(config: &crate::config::StdioTransport) -> SessionSetupResult<()> {
    let command_path = Path::new(&config.command);

    // Check if command is absolute or relative
    if command_path.is_absolute() {
        // For absolute paths, check if executable exists
        if !command_path.exists() {
            return Err(SessionSetupError::McpServerExecutableNotFound {
                server_name: config.name.clone(),
                command: command_path.to_path_buf(),
                suggestion: "Check that the server executable is installed and the path is correct"
                    .to_string(),
            });
        }
    } else {
        // For relative paths, we'll let the process spawn handle the PATH resolution
        // since we can't easily check PATH without additional dependencies
    }

    // Validate working directory if specified
    if let Some(cwd_str) = &config.cwd {
        let path = Path::new(cwd_str);
        validate_working_directory(path)?;
    }

    Ok(())
}

/// Validate HTTP MCP server configuration
fn validate_mcp_http_config(config: &crate::config::HttpTransport) -> SessionSetupResult<()> {
    // Validate URL format
    let parsed_url =
        Url::parse(&config.url).map_err(|_| SessionSetupError::McpServerConnectionFailed {
            server_name: config.name.clone(),
            error: "Invalid URL format".to_string(),
            transport_type: "http".to_string(),
        })?;

    // Validate URL scheme - HTTP requires http or https
    if !url_validation::is_allowed_scheme(&parsed_url, &["http", "https"]) {
        return Err(SessionSetupError::McpServerConnectionFailed {
            server_name: config.name.clone(),
            error: format!(
                "Invalid URL scheme '{}', expected 'http' or 'https'",
                parsed_url.scheme()
            ),
            transport_type: "http".to_string(),
        });
    }

    Ok(())
}

/// Validate SSE MCP server configuration
fn validate_mcp_sse_config(config: &crate::config::SseTransport) -> SessionSetupResult<()> {
    // Validate URL format
    let parsed_url =
        Url::parse(&config.url).map_err(|_| SessionSetupError::McpServerConnectionFailed {
            server_name: config.name.clone(),
            error: "Invalid URL format".to_string(),
            transport_type: "sse".to_string(),
        })?;

    // Validate URL scheme - SSE requires http or https
    if !url_validation::is_allowed_scheme(&parsed_url, &["http", "https"]) {
        return Err(SessionSetupError::McpServerConnectionFailed {
            server_name: config.name.clone(),
            error: format!(
                "Invalid URL scheme '{}', expected 'http' or 'https'",
                parsed_url.scheme()
            ),
            transport_type: "sse".to_string(),
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_validate_working_directory_absolute_valid() {
        let temp_dir = TempDir::new().unwrap();
        let result = validate_working_directory(temp_dir.path());
        assert!(result.is_ok());
    }

    /// Validation must be side-effect free: the permission probe must never
    /// mutate the process CWD (not even transiently restored — a CWD parked
    /// in a session directory poisons every concurrent relative-path
    /// operation and process spawn, and once the directory is deleted, every
    /// later one).
    #[test]
    fn test_validate_working_directory_leaves_process_cwd_untouched() {
        let original = std::env::current_dir().expect("process cwd must be readable");
        let temp_dir = TempDir::new().unwrap();

        validate_working_directory(temp_dir.path()).expect("temp dir must validate");

        assert_eq!(
            std::env::current_dir().expect("process cwd must be readable"),
            original,
            "validation must not mutate the process CWD"
        );
    }

    /// A directory the process can read but not traverse (no execute bit) is
    /// rejected with `WorkingDirectoryPermissionDenied`, and the underlying
    /// OS error is preserved on the error chain (`source`) so diagnostics can
    /// distinguish EACCES from EIO, ELOOP, or a directory deleted mid-check.
    #[cfg(unix)]
    #[test]
    fn test_validate_working_directory_no_execute_preserves_source() {
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = TempDir::new().unwrap();
        let dir = temp_dir.path().join("no-exec");
        std::fs::create_dir(&dir).unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o600)).unwrap();

        let error =
            validate_working_directory(&dir).expect_err("a non-traversable dir must be rejected");
        assert!(
            matches!(
                error,
                SessionSetupError::WorkingDirectoryPermissionDenied { .. }
            ),
            "expected WorkingDirectoryPermissionDenied, got: {error:?}"
        );
        assert!(
            std::error::Error::source(&error).is_some(),
            "the underlying OS error must be preserved as the error source"
        );

        // Restore permissions so TempDir cleanup can remove the directory.
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
    }

    #[test]
    fn test_validate_working_directory_relative_path() {
        let relative_path = Path::new("./relative/path");
        let result = validate_working_directory(relative_path);

        assert!(result.is_err());
        if let Err(SessionSetupError::WorkingDirectoryNotAbsolute { .. }) = result {
            // Expected error type
        } else {
            panic!("Expected WorkingDirectoryNotAbsolute error");
        }
    }

    #[test]
    fn test_validate_working_directory_nonexistent() {
        let nonexistent_path = Path::new("/nonexistent/directory");
        let result = validate_working_directory(nonexistent_path);

        assert!(result.is_err());
        if let Err(SessionSetupError::WorkingDirectoryNotFound { .. }) = result {
            // Expected error type
        } else {
            panic!("Expected WorkingDirectoryNotFound error");
        }
    }

    #[test]
    fn test_validate_working_directory_network_path() {
        let network_path = if cfg!(windows) {
            Path::new("\\\\server\\share\\path")
        } else {
            Path::new("//server/share/path")
        };

        let result = validate_working_directory(network_path);
        assert!(result.is_err());
        if let Err(SessionSetupError::WorkingDirectoryNetworkPath { .. }) = result {
            // Expected error type
        } else {
            panic!("Expected WorkingDirectoryNetworkPath error");
        }
    }

    #[test]
    fn test_find_invalid_path_characters() {
        if cfg!(windows) {
            let invalid_path = "C:\\test<path>with|invalid:chars";
            let invalid_chars = find_invalid_path_characters(invalid_path);
            assert!(!invalid_chars.is_empty());
            assert!(invalid_chars.contains(&"'<'".to_string()));
            assert!(invalid_chars.contains(&"'>'".to_string()));
        }
    }

    #[test]
    fn test_validate_mcp_stdio_config_nonexistent_command() {
        let config = crate::config::StdioTransport {
            name: "test-server".to_string(),
            command: "/nonexistent/command".to_string(),
            args: vec![],
            env: vec![],
            cwd: None,
        };

        let server_config = crate::config::McpServerConfig::Stdio(config);
        let result = validate_mcp_server_config(&server_config);

        assert!(result.is_err());
        if let Err(SessionSetupError::McpServerExecutableNotFound { .. }) = result {
            // Expected error type
        } else {
            panic!("Expected McpServerExecutableNotFound error");
        }
    }

    #[test]
    fn test_validate_mcp_http_config_invalid_url() {
        let config = crate::config::HttpTransport {
            transport_type: "http".to_string(),
            name: "test-server".to_string(),
            url: "not-a-valid-url".to_string(),
            headers: vec![],
        };

        let server_config = crate::config::McpServerConfig::Http(config);
        let result = validate_mcp_server_config(&server_config);

        assert!(result.is_err());
        if let Err(SessionSetupError::McpServerConnectionFailed { .. }) = result {
            // Expected error type
        } else {
            panic!("Expected McpServerConnectionFailed error");
        }
    }
}
