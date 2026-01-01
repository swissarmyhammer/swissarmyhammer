//! Centralized test utilities for the llama-agent crate
//!
//! This module provides simple, consolidated test utility functions
//! to replace duplicated code throughout the codebase.

use crate::types::*;
use serde_json::json;
use std::path::PathBuf;
use std::time::SystemTime;

/// Create a basic test session with no messages, tools, or MCP servers.
///
/// This utility function creates a minimal Session for testing validation logic
/// and other functionality that requires a session but doesn't need specific content.
///
/// # Returns
///
/// A `Session` with:
/// - A new unique SessionId
/// - Empty message history
/// - No available tools or MCP servers  
/// - Current timestamp for created_at and updated_at
/// - Empty compaction history
///
/// # Usage
///
/// ```rust
/// let session = create_empty_session();
/// assert_eq!(session.messages.len(), 0);
/// // Use session for validation testing...
/// ```
pub fn create_empty_session() -> Session {
    Session {
        cwd: PathBuf::from("/tmp"),
        id: SessionId::new(),
        messages: vec![],
        mcp_servers: vec![],
        available_tools: vec![],
        available_prompts: vec![],
        created_at: SystemTime::now(),
        updated_at: SystemTime::now(),
        compaction_history: Vec::new(),
        transcript_path: None,
        context_state: None,
        todos: Vec::new(),
        available_commands: Vec::new(),
        current_mode: None,
        client_capabilities: None,
        cached_message_count: 0,
        cached_token_count: 0,
    }
}

// For now, just provide the basic create_empty_session function
// Additional utilities can be added as needed when implementing the full builders

/// Create a test message with the specified content and User role.
///
/// This utility function creates a Message for testing purposes with
/// sensible defaults for all required fields.
///
/// # Arguments
///
/// * `content` - The message content string
///
/// # Returns
///
/// A `Message` with:
/// - MessageRole::User
/// - The provided content
/// - No tool_call_id or tool_name
/// - Current timestamp
///
/// # Usage
///
/// ```rust
/// let message = create_test_message("Hello, world!");
/// assert_eq!(message.role, MessageRole::User);
/// assert_eq!(message.content, "Hello, world!");
/// ```
pub fn create_test_message(content: &str) -> Message {
    Message {
        role: MessageRole::User,
        content: content.to_string(),
        tool_call_id: None,
        tool_name: None,
        timestamp: SystemTime::now(),
    }
}

/// Create a test session with a single message.
///
/// This utility function creates a Session with one message for testing
/// scenarios that need a session with message history.
///
/// # Arguments
///
/// * `content` - The message content string
///
/// # Returns
///
/// A `Session` with:
/// - A new unique SessionId
/// - One User message with the provided content
/// - No available tools or MCP servers
/// - Current timestamp for created_at and updated_at
/// - Empty compaction history
///
/// # Usage
///
/// ```rust
/// let session = create_session_with_message("Hello");
/// assert_eq!(session.messages.len(), 1);
/// ```
pub fn create_session_with_message(content: &str) -> Session {
    Session {
        cwd: PathBuf::from("/tmp"),
        id: SessionId::new(),
        messages: vec![create_test_message(content)],
        mcp_servers: vec![],
        available_tools: vec![],
        available_prompts: vec![],
        created_at: SystemTime::now(),
        updated_at: SystemTime::now(),
        compaction_history: Vec::new(),
        transcript_path: None,
        context_state: None,
        todos: Vec::new(),
        available_commands: Vec::new(),
        current_mode: None,
        client_capabilities: None,
        cached_message_count: 0,
        cached_token_count: 0,
    }
}

/// Create a test session with multiple messages.
///
/// This utility function creates a Session with the provided messages for testing
/// scenarios that need a session with specific message history.
///
/// # Arguments
///
/// * `messages` - Vector of Message objects
///
/// # Returns
///
/// A `Session` with:
/// - A new unique SessionId
/// - The provided message history
/// - No available tools or MCP servers
/// - Current timestamp for created_at and updated_at
/// - Empty compaction history
///
/// # Usage
///
/// ```rust
/// let messages = vec![create_test_message("Hello"), create_test_message("World")];
/// let session = create_session_with_messages(messages);
/// assert_eq!(session.messages.len(), 2);
/// ```
pub fn create_session_with_messages(messages: Vec<Message>) -> Session {
    Session {
        cwd: PathBuf::from("/tmp"),
        id: SessionId::new(),
        messages,
        mcp_servers: vec![],
        available_tools: vec![],
        available_prompts: vec![],
        created_at: SystemTime::now(),
        updated_at: SystemTime::now(),
        compaction_history: Vec::new(),
        transcript_path: None,
        context_state: None,
        todos: Vec::new(),
        available_commands: Vec::new(),
        current_mode: None,
        client_capabilities: None,
        cached_message_count: 0,
        cached_token_count: 0,
    }
}

/// Create a test session with available tools.
///
/// This utility function creates a Session with the provided tools for testing
/// scenarios that need tool validation.
///
/// # Arguments
///
/// * `tools` - Vector of ToolDefinition objects
///
/// # Returns
///
/// A `Session` with:
/// - A new unique SessionId
/// - Empty message history
/// - The provided available tools
/// - No MCP servers
/// - Current timestamp for created_at and updated_at
/// - Empty compaction history
///
/// # Usage
///
/// ```rust
/// let tools = vec![create_test_tool_definition("test_tool")];
/// let session = create_session_with_tools(tools);
/// assert_eq!(session.available_tools.len(), 1);
/// ```
pub fn create_session_with_tools(tools: Vec<ToolDefinition>) -> Session {
    Session {
        cwd: PathBuf::from("/tmp"),
        id: SessionId::new(),
        messages: vec![],
        mcp_servers: vec![],
        available_tools: tools,
        available_prompts: vec![],
        created_at: SystemTime::now(),
        updated_at: SystemTime::now(),
        compaction_history: Vec::new(),
        transcript_path: None,
        context_state: None,
        todos: Vec::new(),
        available_commands: Vec::new(),
        current_mode: None,
        client_capabilities: None,
        cached_message_count: 0,
        cached_token_count: 0,
    }
}

/// Create a test tool definition for testing scenarios.
///
/// This utility function creates a ToolDefinition for testing tool validation
/// with sensible defaults.
///
/// # Arguments
///
/// * `name` - The tool name
///
/// # Returns
///
/// A `ToolDefinition` with:
/// - The provided name
/// - A descriptive description
/// - Basic input parameter schema
/// - Default test server name
///
/// # Usage
///
/// ```rust
/// let tool = create_test_tool_definition("my_tool");
/// assert_eq!(tool.name, "my_tool");
/// ```
pub fn create_test_tool_definition(name: &str) -> ToolDefinition {
    ToolDefinition {
        name: name.to_string(),
        description: format!("Test tool {}", name),
        parameters: json!({
            "type": "object",
            "properties": {
                "input": {"type": "string"}
            }
        }),
        server_name: "test_server".to_string(),
    }
}

// Re-export the original Qwen generate function for compatibility
pub use super::test_utils::create_qwen_generate_summary_fn;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_empty_session() {
        let session = create_empty_session();
        assert_eq!(session.messages.len(), 0);
        assert_eq!(session.available_tools.len(), 0);
        assert_eq!(session.mcp_servers.len(), 0);
    }
}
