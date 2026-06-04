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

/// Validate directory permissions for session operations
fn validate_directory_permissions(path: &Path) -> SessionSetupResult<()> {
    let denied = || SessionSetupError::WorkingDirectoryPermissionDenied {
        path: path.to_path_buf(),
        required_permissions: vec!["read".to_string(), "execute".to_string()],
    };

    // Readability: the directory must list its entries.
    fs::read_dir(path).map_err(|_| denied())?;

    // Traversability (the Unix execute bit). We must NOT probe this by
    // `set_current_dir` — that mutates the *process-global* working directory,
    // which races every other thread/test reading `current_dir()` (and left the
    // process pointed at a since-deleted temp dir, surfacing as `Os NotFound`
    // panics across the suite). Check the mode bits directly instead, leaving
    // global state untouched.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(path)
            .map_err(|_| denied())?
            .permissions()
            .mode();
        // Any of owner/group/other execute (0o111) implies traverse access.
        // This mirrors the previous `set_current_dir` probe without the
        // root-bypasses-permissions caveat, which is irrelevant for a check
        // whose only job is a fast, non-mutating sanity gate.
        if mode & 0o111 == 0 {
            return Err(denied());
        }
    }

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

    /// The validator must not mutate the process-global working directory.
    ///
    /// Regression for the cross-test `Os NotFound` storm: the old
    /// `validate_directory_permissions` probed traverse access with
    /// `set_current_dir(path)` and a broken restore, permanently leaving the
    /// process CWD pointed at the validated temp dir. Once that temp dir was
    /// dropped, every concurrent `std::env::current_dir()` in the suite
    /// panicked with `Os NotFound`. Validation is now non-mutating, so the
    /// CWD is unchanged before and after the call.
    #[test]
    fn test_validate_working_directory_does_not_change_process_cwd() {
        let before = std::env::current_dir().unwrap();
        let temp_dir = TempDir::new().unwrap();

        validate_working_directory(temp_dir.path()).unwrap();

        let after = std::env::current_dir().unwrap();
        assert_eq!(
            before, after,
            "validate_working_directory must not mutate the process CWD",
        );
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
