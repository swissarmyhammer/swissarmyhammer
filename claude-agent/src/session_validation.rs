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
    // Try to read the directory
    match fs::read_dir(path) {
        Ok(_) => {
            // Directory is readable, now check if we can access it (execute permission)
            // On Unix systems, execute permission on a directory means we can traverse it
            match std::env::set_current_dir(path) {
                Ok(_) => {
                    // Restore the original directory
                    if let Ok(original_dir) = std::env::current_dir() {
                        let _ = std::env::set_current_dir(&original_dir);
                    }
                    Ok(())
                }
                Err(_) => Err(SessionSetupError::WorkingDirectoryPermissionDenied {
                    path: path.to_path_buf(),
                    required_permissions: vec!["read".to_string(), "execute".to_string()],
                }),
            }
        }
        Err(_) => Err(SessionSetupError::WorkingDirectoryPermissionDenied {
            path: path.to_path_buf(),
            required_permissions: vec!["read".to_string(), "execute".to_string()],
        }),
    }
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

/// Validate session ID format according to ACP requirements
///
/// Validates that the session ID follows the required format: raw ULID
///
/// # Examples
/// Valid: `01ARZ3NDEKTSV4RRFFQ69G5FAV`
/// Invalid: `sess_01ARZ3NDEKTSV4RRFFQ69G5FAV` (old format with prefix)
/// Invalid: `session_123` (invalid format)
pub fn validate_session_id(session_id: &str) -> SessionSetupResult<crate::session::SessionId> {
    // ACP requires consistent session ID format as raw ULID
    match crate::session::SessionId::parse(session_id) {
        Ok(id) => Ok(id),
        Err(_) => Err(SessionSetupError::InvalidSessionId {
            provided_id: session_id.to_string(),
            expected_format: "26-character ULID".to_string(),
            example: "01ARZ3NDEKTSV4RRFFQ69G5FAV".to_string(),
        }),
    }
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
    fn test_validate_session_id_valid() {
        let valid_id = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
        let result = validate_session_id(valid_id);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_session_id_raw_ulid() {
        let valid_id = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
        let result = validate_session_id(valid_id);

        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_session_id_invalid_ulid() {
        let invalid_id = "invalid-session-id";
        let result = validate_session_id(invalid_id);

        assert!(result.is_err());
        if let Err(SessionSetupError::InvalidSessionId { .. }) = result {
            // Expected error type
        } else {
            panic!("Expected InvalidSessionId error");
        }
    }

    #[test]
    fn test_validate_session_id_empty() {
        let invalid_id = "";
        let result = validate_session_id(invalid_id);

        assert!(result.is_err());
        if let Err(SessionSetupError::InvalidSessionId { .. }) = result {
            // Expected error type
        } else {
            panic!("Expected InvalidSessionId error");
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
