//! Coverage tests: Configuration types.

use claude_agent::config;
use std::path::PathBuf;

/// Name that the validation tests give a valid transport.
const TEST_NAME: &str = "test";

/// Name of the stdio server that the name and round trip tests build.
const TEST_SERVER_NAME: &str = "test_server";

/// Command that the stdio transport tests run.
const ECHO_COMMAND: &str = "echo";

/// Transport type tag of an HTTP transport.
const HTTP_TRANSPORT_TYPE: &str = "http";

/// Transport type tag of an SSE transport.
const SSE_TRANSPORT_TYPE: &str = "sse";

/// Valid HTTP URL that the HTTP transport tests use.
const LOCALHOST_URL: &str = "http://localhost";

/// Valid SSE URL that the SSE transport tests use.
const LOCALHOST_EVENTS_URL: &str = "http://localhost/events";

/// Name of the HTTP server in the config tests.
const HTTP_SERVER_NAME: &str = "http_server";

/// URL that the HTTP config tests use.
const LOCALHOST_8080_URL: &str = "http://localhost:8080";

/// Name of the SSE server in the config tests.
const SSE_SERVER_NAME: &str = "sse_server";

/// Header value that the config tests use.
const HEADER_VALUE: &str = "val";

/// URL with a scheme that validation rejects.
const INVALID_FTP_URL: &str = "ftp://invalid";

/// Environment variable name that the equality test uses.
const ENV_VAR_NAME: &str = "KEY";

/// Environment variable value that the equality test uses.
const ENV_VAR_VALUE: &str = "VAL";

/// Header name that the equality test uses.
const AUTH_HEADER_NAME: &str = "Auth";

/// Header value that the equality test uses.
const BEARER_TOKEN_VALUE: &str = "Bearer xyz";

/// Recording file path that the config tests use.
const RECORDING_PATH: &str = "/tmp/recording.json";

// ============================================================================
// Config tests
// ============================================================================

#[test]
fn test_mcp_server_config_stdio_name() {
    let config = config::McpServerConfig::Stdio(config::StdioTransport {
        name: TEST_SERVER_NAME.to_string(),
        command: TEST_NAME.to_string(),
        args: vec![],
        env: vec![],
        cwd: None,
    });
    assert_eq!(config.name(), TEST_SERVER_NAME);
    assert_eq!(config.transport_type(), "stdio");
}

#[test]
fn test_mcp_server_config_http_name() {
    let config = config::McpServerConfig::Http(config::HttpTransport {
        transport_type: HTTP_TRANSPORT_TYPE.to_string(),
        name: HTTP_SERVER_NAME.to_string(),
        url: LOCALHOST_8080_URL.to_string(),
        headers: vec![],
    });
    assert_eq!(config.name(), HTTP_SERVER_NAME);
    assert_eq!(config.transport_type(), HTTP_TRANSPORT_TYPE);
}

#[test]
fn test_mcp_server_config_sse_name() {
    let config = config::McpServerConfig::Sse(config::SseTransport {
        transport_type: SSE_TRANSPORT_TYPE.to_string(),
        name: SSE_SERVER_NAME.to_string(),
        url: "http://localhost:9090/events".to_string(),
        headers: vec![],
    });
    assert_eq!(config.name(), SSE_SERVER_NAME);
    assert_eq!(config.transport_type(), SSE_TRANSPORT_TYPE);
}

#[test]
fn test_stdio_transport_validate_success() {
    let config = config::StdioTransport {
        name: TEST_NAME.to_string(),
        command: ECHO_COMMAND.to_string(),
        args: vec!["hello".to_string()],
        env: vec![],
        cwd: None,
    };
    assert!(config.validate().is_ok());
}

#[test]
fn test_stdio_transport_validate_empty_name() {
    let config = config::StdioTransport {
        name: "".to_string(),
        command: ECHO_COMMAND.to_string(),
        args: vec![],
        env: vec![],
        cwd: None,
    };
    assert!(config.validate().is_err());
}

#[test]
fn test_stdio_transport_validate_empty_command() {
    let config = config::StdioTransport {
        name: TEST_NAME.to_string(),
        command: "".to_string(),
        args: vec![],
        env: vec![],
        cwd: None,
    };
    assert!(config.validate().is_err());
}

#[test]
fn test_stdio_transport_validate_empty_env_name() {
    let config = config::StdioTransport {
        name: TEST_NAME.to_string(),
        command: ECHO_COMMAND.to_string(),
        args: vec![],
        env: vec![config::EnvVariable {
            name: "".to_string(),
            value: HEADER_VALUE.to_string(),
        }],
        cwd: None,
    };
    assert!(config.validate().is_err());
}

#[test]
fn test_http_transport_validate_success() {
    let config = config::HttpTransport {
        transport_type: HTTP_TRANSPORT_TYPE.to_string(),
        name: TEST_NAME.to_string(),
        url: LOCALHOST_8080_URL.to_string(),
        headers: vec![],
    };
    assert!(config.validate().is_ok());
}

#[test]
fn test_http_transport_validate_https() {
    let config = config::HttpTransport {
        transport_type: HTTP_TRANSPORT_TYPE.to_string(),
        name: TEST_NAME.to_string(),
        url: "https://api.example.com".to_string(),
        headers: vec![],
    };
    assert!(config.validate().is_ok());
}

#[test]
fn test_http_transport_validate_empty_name() {
    let config = config::HttpTransport {
        transport_type: HTTP_TRANSPORT_TYPE.to_string(),
        name: "".to_string(),
        url: LOCALHOST_URL.to_string(),
        headers: vec![],
    };
    assert!(config.validate().is_err());
}

#[test]
fn test_http_transport_validate_empty_url() {
    let config = config::HttpTransport {
        transport_type: HTTP_TRANSPORT_TYPE.to_string(),
        name: TEST_NAME.to_string(),
        url: "".to_string(),
        headers: vec![],
    };
    assert!(config.validate().is_err());
}

#[test]
fn test_http_transport_validate_bad_url() {
    let config = config::HttpTransport {
        transport_type: HTTP_TRANSPORT_TYPE.to_string(),
        name: TEST_NAME.to_string(),
        url: INVALID_FTP_URL.to_string(),
        headers: vec![],
    };
    assert!(config.validate().is_err());
}

#[test]
fn test_http_transport_validate_empty_header_name() {
    let config = config::HttpTransport {
        transport_type: HTTP_TRANSPORT_TYPE.to_string(),
        name: TEST_NAME.to_string(),
        url: LOCALHOST_URL.to_string(),
        headers: vec![config::HttpHeader {
            name: "".to_string(),
            value: HEADER_VALUE.to_string(),
        }],
    };
    assert!(config.validate().is_err());
}

#[test]
fn test_sse_transport_validate_success() {
    let config = config::SseTransport {
        transport_type: SSE_TRANSPORT_TYPE.to_string(),
        name: TEST_NAME.to_string(),
        url: LOCALHOST_EVENTS_URL.to_string(),
        headers: vec![],
    };
    assert!(config.validate().is_ok());
}

#[test]
fn test_sse_transport_validate_https() {
    let config = config::SseTransport {
        transport_type: SSE_TRANSPORT_TYPE.to_string(),
        name: TEST_NAME.to_string(),
        url: "https://api.example.com/events".to_string(),
        headers: vec![],
    };
    assert!(config.validate().is_ok());
}

#[test]
fn test_sse_transport_validate_empty_name() {
    let config = config::SseTransport {
        transport_type: SSE_TRANSPORT_TYPE.to_string(),
        name: "".to_string(),
        url: LOCALHOST_EVENTS_URL.to_string(),
        headers: vec![],
    };
    assert!(config.validate().is_err());
}

#[test]
fn test_sse_transport_validate_empty_url() {
    let config = config::SseTransport {
        transport_type: SSE_TRANSPORT_TYPE.to_string(),
        name: TEST_NAME.to_string(),
        url: "".to_string(),
        headers: vec![],
    };
    assert!(config.validate().is_err());
}

#[test]
fn test_sse_transport_validate_bad_url() {
    let config = config::SseTransport {
        transport_type: SSE_TRANSPORT_TYPE.to_string(),
        name: TEST_NAME.to_string(),
        url: INVALID_FTP_URL.to_string(),
        headers: vec![],
    };
    assert!(config.validate().is_err());
}

#[test]
fn test_mcp_server_config_validate_delegates_to_transport() {
    let config = config::McpServerConfig::Stdio(config::StdioTransport {
        name: TEST_NAME.to_string(),
        command: ECHO_COMMAND.to_string(),
        args: vec![],
        env: vec![],
        cwd: None,
    });
    assert!(config.validate().is_ok());

    let config = config::McpServerConfig::Http(config::HttpTransport {
        transport_type: HTTP_TRANSPORT_TYPE.to_string(),
        name: TEST_NAME.to_string(),
        url: LOCALHOST_URL.to_string(),
        headers: vec![],
    });
    assert!(config.validate().is_ok());

    let config = config::McpServerConfig::Sse(config::SseTransport {
        transport_type: SSE_TRANSPORT_TYPE.to_string(),
        name: TEST_NAME.to_string(),
        url: LOCALHOST_EVENTS_URL.to_string(),
        headers: vec![],
    });
    assert!(config.validate().is_ok());
}

#[test]
fn test_config_serialization_roundtrip() {
    let config = config::McpServerConfig::Stdio(config::StdioTransport {
        name: TEST_SERVER_NAME.to_string(),
        command: "node".to_string(),
        args: vec!["server.js".to_string()],
        env: vec![config::EnvVariable {
            name: "PORT".to_string(),
            value: "3000".to_string(),
        }],
        cwd: Some("/app".to_string()),
    });
    let json = serde_json::to_string(&config).unwrap();
    let deserialized: config::McpServerConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(serde_json::to_string(&deserialized).unwrap(), json);
}

#[test]
fn test_env_variable_eq() {
    let a = claude_agent::config::EnvVariable {
        name: ENV_VAR_NAME.to_string(),
        value: ENV_VAR_VALUE.to_string(),
    };
    let b = claude_agent::config::EnvVariable {
        name: ENV_VAR_NAME.to_string(),
        value: ENV_VAR_VALUE.to_string(),
    };
    assert_eq!(a, b);
}

#[test]
fn test_http_header_eq() {
    let a = config::HttpHeader {
        name: AUTH_HEADER_NAME.to_string(),
        value: BEARER_TOKEN_VALUE.to_string(),
    };
    let b = config::HttpHeader {
        name: AUTH_HEADER_NAME.to_string(),
        value: BEARER_TOKEN_VALUE.to_string(),
    };
    assert_eq!(a, b);
}

#[test]
fn test_claude_agent_mode_default() {
    let mode = config::ClaudeAgentMode::default();
    assert_eq!(mode, config::ClaudeAgentMode::Normal);
}

#[test]
fn test_claude_agent_mode_record() {
    let mode = config::ClaudeAgentMode::Record {
        output_path: PathBuf::from(RECORDING_PATH),
    };
    assert!(matches!(mode, config::ClaudeAgentMode::Record { .. }));
}

#[test]
fn test_claude_agent_mode_playback() {
    let mode = config::ClaudeAgentMode::Playback {
        input_path: PathBuf::from(RECORDING_PATH),
    };
    assert!(matches!(mode, config::ClaudeAgentMode::Playback { .. }));
}

#[test]
fn test_stdio_transport_validate_working_directory_none() {
    let config = config::StdioTransport {
        name: TEST_NAME.to_string(),
        command: ECHO_COMMAND.to_string(),
        args: vec![],
        env: vec![],
        cwd: None,
    };
    let security = config::SecurityConfig {
        allowed_file_patterns: vec![],
        forbidden_paths: vec![],
        require_permission_for: vec![],
    };
    assert!(config.validate_working_directory(&security).is_ok());
}

#[test]
fn test_stdio_transport_validate_working_directory_empty() {
    let config = config::StdioTransport {
        name: TEST_NAME.to_string(),
        command: ECHO_COMMAND.to_string(),
        args: vec![],
        env: vec![],
        cwd: Some("".to_string()),
    };
    let security = config::SecurityConfig {
        allowed_file_patterns: vec![],
        forbidden_paths: vec![],
        require_permission_for: vec![],
    };
    assert!(config.validate_working_directory(&security).is_err());
}

#[test]
fn test_stdio_transport_validate_working_directory_relative() {
    let config = config::StdioTransport {
        name: TEST_NAME.to_string(),
        command: ECHO_COMMAND.to_string(),
        args: vec![],
        env: vec![],
        cwd: Some("relative/path".to_string()),
    };
    let security = config::SecurityConfig {
        allowed_file_patterns: vec![],
        forbidden_paths: vec![],
        require_permission_for: vec![],
    };
    assert!(config.validate_working_directory(&security).is_err());
}

#[test]
fn test_stdio_transport_validate_working_directory_valid() {
    let config = config::StdioTransport {
        name: TEST_NAME.to_string(),
        command: ECHO_COMMAND.to_string(),
        args: vec![],
        env: vec![],
        cwd: Some("/tmp".to_string()),
    };
    let security = config::SecurityConfig {
        allowed_file_patterns: vec![],
        forbidden_paths: vec![],
        require_permission_for: vec![],
    };
    assert!(config.validate_working_directory(&security).is_ok());
}
