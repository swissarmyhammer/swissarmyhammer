//! Coverage tests: File locations, editor state, collected responses and the agent version.

use claude_agent::tool_types::*;
use serde_json::json;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

/// Tool name that the file location tests pass.
const GLOB_TOOL_NAME: &str = "glob";

/// Tool name that the location tests use.
const READ_TOOL_NAME: &str = "read";

/// JSON key that carries a file path.
const PATH_KEY: &str = "path";

/// Glob pattern that the location tests use.
const RUST_GLOB_PATTERN: &str = "*.rs";

/// Text that the debug test uses.
const HELLO_TEXT: &str = "hello";

/// Session id that the request test uses.
const TEST_SESSION_ID: &str = "test-session-id";

// ============================================================================
// File location extraction tests (additional coverage)
// ============================================================================

#[test]
fn test_extract_file_locations_glob_patterns() {
    let args = json!({
        "patterns": ["src/**/*.rs", "tests/**/*.rs"]
    });
    let locations = ToolCallReport::extract_file_locations(GLOB_TOOL_NAME, &args);
    assert_eq!(locations.len(), 2);
}

#[test]
fn test_extract_file_locations_string_argument() {
    let args = json!("/home/user/file.txt");
    let locations = ToolCallReport::extract_file_locations(READ_TOOL_NAME, &args);
    assert_eq!(locations.len(), 1);
}

#[test]
fn test_extract_file_locations_string_not_path() {
    let args = json!("just a string without path chars");
    let locations = ToolCallReport::extract_file_locations(READ_TOOL_NAME, &args);
    assert!(locations.is_empty());
}

#[test]
fn test_extract_file_locations_with_glob_wildcards() {
    // Test glob patterns are recognized as file paths via extract_file_locations
    let args = json!({(PATH_KEY): RUST_GLOB_PATTERN});
    let locations = ToolCallReport::extract_file_locations(GLOB_TOOL_NAME, &args);
    assert_eq!(locations.len(), 1);
    assert!(locations[0].path.contains(RUST_GLOB_PATTERN));
}

#[test]
fn test_extract_file_locations_with_bracket_glob() {
    let args = json!({(PATH_KEY): "test[0-9].txt"});
    let locations = ToolCallReport::extract_file_locations(GLOB_TOOL_NAME, &args);
    assert_eq!(locations.len(), 1);
}

// ============================================================================
// EditorState tests (additional coverage)
// ============================================================================

#[tokio::test]
async fn test_editor_state_manager_default() {
    let manager = claude_agent::editor_state::EditorStateManager::default();
    assert_eq!(manager.cache_size().await, 0);
}

#[tokio::test]
async fn test_editor_state_manager_with_cache_duration() {
    let manager = claude_agent::editor_state::EditorStateManager::with_cache_duration(
        Duration::from_secs(60),
    );
    assert_eq!(manager.cache_size().await, 0);
}

#[tokio::test]
async fn test_editor_state_update_buffers_with_unavailable() {
    let manager = claude_agent::editor_state::EditorStateManager::new();
    let path1 = PathBuf::from("/test/file1.rs");

    // First cache a buffer
    let buffer = claude_agent::editor_state::EditorBuffer {
        path: path1.clone(),
        content: "content".to_string(),
        modified: true,
        last_modified: SystemTime::now(),
        encoding: "UTF-8".to_string(),
    };
    manager.cache_buffer(path1.clone(), buffer).await;
    assert_eq!(manager.cache_size().await, 1);

    // Then mark it as unavailable via update
    let response = claude_agent::editor_state::EditorBufferResponse {
        buffers: HashMap::new(),
        unavailable_paths: vec![path1],
    };
    manager.update_buffers_from_response(response).await;
    assert_eq!(manager.cache_size().await, 0);
}

// ============================================================================
// CollectedResponse and CreateAgentConfig tests
// ============================================================================

#[test]
fn test_collected_response_debug() {
    let resp = claude_agent::CollectedResponse {
        content: HELLO_TEXT.to_string(),
        stop_reason: agent_client_protocol::schema::StopReason::EndTurn,
        cache_usage: None,
    };
    let debug = format!("{:?}", resp);
    assert!(debug.contains(HELLO_TEXT));
}

#[test]
fn test_create_agent_config_builder() {
    let config = claude_agent::CreateAgentConfig::builder()
        .ephemeral(true)
        .build();
    assert!(config.ephemeral);
    assert!(config.mcp_servers.is_empty());
}

#[test]
fn test_create_agent_config_builder_default() {
    let config = claude_agent::CreateAgentConfig::builder().build();
    assert!(!config.ephemeral);
    assert!(config.mcp_servers.is_empty());
}

#[test]
fn test_prompt_request_creation() {
    // Verify we can construct ACP PromptRequest objects
    let session_id = agent_client_protocol::schema::SessionId::new(TEST_SESSION_ID);
    let request = agent_client_protocol::schema::PromptRequest::new(
        session_id,
        vec![agent_client_protocol::schema::ContentBlock::Text(
            agent_client_protocol::schema::TextContent::new("test prompt".to_string()),
        )],
    );
    assert_eq!(request.session_id.to_string(), TEST_SESSION_ID);
}

// ============================================================================
// Agent version constant test
// ============================================================================

#[test]
fn test_version_constant() {
    assert_eq!(claude_agent::VERSION, env!("CARGO_PKG_VERSION"));
}
