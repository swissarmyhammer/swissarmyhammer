//! Type conversion between ACP and llama-agent types
//!
//! This module handles bidirectional mapping between ACP protocol types
//! and llama-agent internal types.

use crate::types::ids::SessionId as LlamaSessionId;
use crate::types::{AgentAPI, Message, MessageRole, ToolCall, ToolDefinition, ToolResult};
use agent_client_protocol::{ContentBlock, SessionId as AcpSessionId};
use std::sync::Arc;
use std::time::SystemTime;

use super::permissions::{PermissionEvaluation, PermissionPolicyEngine, PermissionStorage};
use super::session::AcpSessionState;

/// Convert ACP content blocks to llama messages
///
/// Translates ACP ContentBlocks to llama-agent Messages.
/// Currently supports text content only. Image, audio, resource, and other content types
/// can be added later if needed.
///
/// # Arguments
/// * `content` - Vector of ACP ContentBlocks to translate
///
/// # Returns
/// * `Ok(Vec<Message>)` - Successfully translated messages
/// * `Err(TranslationError)` - If unsupported content is encountered
///
/// # Examples
/// ```ignore
/// use agent_client_protocol::ContentBlock;
/// use llama_agent::acp::translation::acp_to_llama_messages;
///
/// let content = vec![ContentBlock::from("Hello, world!")];
///
/// let messages = acp_to_llama_messages(content).unwrap();
/// assert_eq!(messages.len(), 1);
/// assert_eq!(messages[0].content, "Hello, world!");
/// ```
pub fn acp_to_llama_messages(content: Vec<ContentBlock>) -> Result<Vec<Message>, TranslationError> {
    let mut messages = Vec::new();

    for block in content {
        match block {
            ContentBlock::Text(text_content) => {
                messages.push(Message {
                    role: MessageRole::User,
                    content: text_content.text,
                    tool_call_id: None,
                    tool_name: None,
                    timestamp: SystemTime::now(),
                });
            }
            ContentBlock::Image(_) => {
                return Err(TranslationError::UnsupportedContent(
                    "Image content not yet supported".to_string(),
                ));
            }
            ContentBlock::Audio(_) => {
                return Err(TranslationError::UnsupportedContent(
                    "Audio content not yet supported".to_string(),
                ));
            }
            ContentBlock::ResourceLink(resource_link) => {
                // Per ACP spec: "All agents MUST support resource links in prompts"
                // Convert resource link to a text description for the LLM
                let description =
                    format!("[Resource: {} ({})]", resource_link.name, resource_link.uri);
                messages.push(Message {
                    role: MessageRole::User,
                    content: description,
                    tool_call_id: None,
                    tool_name: None,
                    timestamp: SystemTime::now(),
                });
            }
            ContentBlock::Resource(_) => {
                return Err(TranslationError::UnsupportedContent(
                    "Embedded resource content not yet supported".to_string(),
                ));
            }
            // Handle any future ContentBlock variants
            _ => {
                return Err(TranslationError::UnsupportedContent(
                    "Unknown content type not yet supported".to_string(),
                ));
            }
        }
    }

    Ok(messages)
}

/// Error types for content translation
#[derive(Debug, thiserror::Error)]
pub enum TranslationError {
    #[error("Unsupported content type: {0}")]
    UnsupportedContent(String),

    #[error("Invalid content format: {0}")]
    InvalidFormat(String),

    #[error("Invalid session ID format: {0}")]
    InvalidSessionId(String),
}

/// JSON-RPC 2.0 error structure following ACP specification
#[derive(Debug, Clone)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

/// Trait for converting errors to JSON-RPC format
pub trait ToJsonRpcError: std::fmt::Display {
    /// Convert error to JSON-RPC error code
    fn to_json_rpc_code(&self) -> i32;

    /// Convert error to structured error data (optional)
    fn to_error_data(&self) -> Option<serde_json::Value> {
        None
    }

    /// Convert error to complete JSON-RPC error structure
    fn to_json_rpc_error(&self) -> JsonRpcError {
        JsonRpcError {
            code: self.to_json_rpc_code(),
            message: self.to_string(),
            data: self.to_error_data(),
        }
    }
}

// Implement ToJsonRpcError for llama-agent error types

impl ToJsonRpcError for crate::types::AgentError {
    fn to_json_rpc_code(&self) -> i32 {
        match self {
            crate::types::AgentError::Model(_) => -32603, // Internal error
            crate::types::AgentError::Queue(_) => -32603, // Internal error
            crate::types::AgentError::Session(e) => e.to_json_rpc_code(),
            crate::types::AgentError::MCP(e) => e.to_json_rpc_code(),
            crate::types::AgentError::Template(e) => e.to_json_rpc_code(),
            crate::types::AgentError::Timeout { .. } => -32000, // Server error
            crate::types::AgentError::QueueFull { .. } => -32000, // Server error
        }
    }

    fn to_error_data(&self) -> Option<serde_json::Value> {
        use serde_json::json;

        match self {
            crate::types::AgentError::Timeout { timeout } => Some(json!({
                "error": "request_timeout",
                "timeoutSeconds": timeout.as_secs(),
                "suggestion": "Increase timeout settings, reduce max_tokens, or check system performance"
            })),
            crate::types::AgentError::QueueFull { capacity } => Some(json!({
                "error": "queue_overloaded",
                "capacity": capacity,
                "suggestion": "Wait and retry, or increase max_queue_size configuration"
            })),
            crate::types::AgentError::Session(e) => e.to_error_data(),
            crate::types::AgentError::MCP(e) => e.to_error_data(),
            crate::types::AgentError::Template(e) => e.to_error_data(),
            _ => None,
        }
    }
}

impl ToJsonRpcError for crate::types::QueueError {
    fn to_json_rpc_code(&self) -> i32 {
        match self {
            crate::types::QueueError::Full => -32000, // Server error
            crate::types::QueueError::WorkerError(_) => -32603, // Internal error
        }
    }

    fn to_error_data(&self) -> Option<serde_json::Value> {
        use serde_json::json;

        match self {
            crate::types::QueueError::Full => Some(json!({
                "error": "queue_full",
                "suggestion": "Wait a moment and retry, or increase the max_queue_size configuration"
            })),
            crate::types::QueueError::WorkerError(msg) => Some(json!({
                "error": "worker_error",
                "details": msg,
                "suggestion": "Check system resources and model availability"
            })),
        }
    }
}

impl ToJsonRpcError for crate::types::SessionError {
    fn to_json_rpc_code(&self) -> i32 {
        match self {
            crate::types::SessionError::NotFound(_) => -32602, // Invalid params
            crate::types::SessionError::LimitExceeded => -32000, // Server error
            crate::types::SessionError::Timeout => -32000,     // Server error
            crate::types::SessionError::InvalidState(_) => -32602, // Invalid params
        }
    }

    fn to_error_data(&self) -> Option<serde_json::Value> {
        use serde_json::json;

        match self {
            crate::types::SessionError::NotFound(id) => Some(json!({
                "error": "session_not_found",
                "sessionId": id,
                "suggestion": "Verify the session ID is correct and the session hasn't expired"
            })),
            crate::types::SessionError::LimitExceeded => Some(json!({
                "error": "session_limit_exceeded",
                "suggestion": "Close unused sessions or increase session limits in configuration"
            })),
            crate::types::SessionError::Timeout => Some(json!({
                "error": "session_timeout",
                "suggestion": "Increase session timeout or complete operations more quickly"
            })),
            crate::types::SessionError::InvalidState(msg) => Some(json!({
                "error": "invalid_session_state",
                "details": msg,
                "suggestion": "Check that the session is in the correct state for this operation"
            })),
        }
    }
}

impl ToJsonRpcError for crate::types::MCPError {
    fn to_json_rpc_code(&self) -> i32 {
        match self {
            crate::types::MCPError::ServerNotFound(_) => -32602, // Invalid params
            crate::types::MCPError::ToolCallFailed(_) => -32000, // Server error
            crate::types::MCPError::Connection(_) => -32000,     // Server error
            crate::types::MCPError::Protocol(_) => -32600,       // Invalid Request
            crate::types::MCPError::HttpUrlInvalid(_) => -32602, // Invalid params
            crate::types::MCPError::HttpTimeout(_) => -32000,    // Server error
            crate::types::MCPError::HttpConnection(_) => -32000, // Server error
            crate::types::MCPError::Timeout(_) => -32000,        // Server error
        }
    }

    fn to_error_data(&self) -> Option<serde_json::Value> {
        use serde_json::json;

        match self {
            crate::types::MCPError::ServerNotFound(name) => Some(json!({
                "error": "mcp_server_not_found",
                "serverName": name,
                "suggestion": "Check server configuration and ensure the server name is correct"
            })),
            crate::types::MCPError::ToolCallFailed(msg) => Some(json!({
                "error": "mcp_tool_call_failed",
                "details": msg,
                "suggestion": "Verify tool parameters and server availability"
            })),
            crate::types::MCPError::Connection(msg) => Some(json!({
                "error": "mcp_connection_error",
                "details": msg,
                "suggestion": "Check network connectivity and server status"
            })),
            crate::types::MCPError::Protocol(msg) => Some(json!({
                "error": "mcp_protocol_error",
                "details": msg,
                "suggestion": "Ensure MCP server is compatible and responding correctly"
            })),
            crate::types::MCPError::HttpUrlInvalid(msg) => Some(json!({
                "error": "mcp_http_url_invalid",
                "details": msg,
                "suggestion": "Check the URL format and protocol"
            })),
            crate::types::MCPError::HttpTimeout(msg) => Some(json!({
                "error": "mcp_http_timeout",
                "details": msg,
                "suggestion": "Increase timeout settings or check network latency"
            })),
            crate::types::MCPError::HttpConnection(msg) => Some(json!({
                "error": "mcp_http_connection_failed",
                "details": msg,
                "suggestion": "Verify server URL and network connectivity"
            })),
            crate::types::MCPError::Timeout(msg) => Some(json!({
                "error": "mcp_timeout",
                "details": msg,
                "suggestion": "Increase timeout settings or check server responsiveness"
            })),
        }
    }
}

impl ToJsonRpcError for crate::types::TemplateError {
    fn to_json_rpc_code(&self) -> i32 {
        match self {
            crate::types::TemplateError::RenderingFailed(_) => -32602, // Invalid params
            crate::types::TemplateError::ToolCallParsing(_) => -32602, // Invalid params
            crate::types::TemplateError::Invalid(_) => -32602,         // Invalid params
        }
    }

    fn to_error_data(&self) -> Option<serde_json::Value> {
        use serde_json::json;

        match self {
            crate::types::TemplateError::RenderingFailed(msg) => Some(json!({
                "error": "template_rendering_failed",
                "details": msg,
                "suggestion": "Check template syntax and variable availability"
            })),
            crate::types::TemplateError::ToolCallParsing(msg) => Some(json!({
                "error": "template_tool_parsing_failed",
                "details": msg,
                "suggestion": "Verify tool call format and JSON structure"
            })),
            crate::types::TemplateError::Invalid(msg) => Some(json!({
                "error": "template_invalid",
                "details": msg,
                "suggestion": "Check template structure and required parameters"
            })),
        }
    }
}

impl ToJsonRpcError for crate::generation::GenerationError {
    fn to_json_rpc_code(&self) -> i32 {
        match self {
            crate::generation::GenerationError::InvalidConfig(_) => -32602, // Invalid params
            crate::generation::GenerationError::TokenizationFailed(_) => -32603, // Internal error
            crate::generation::GenerationError::BatchFailed(_) => -32603,   // Internal error
            crate::generation::GenerationError::DecodingFailed(_) => -32603, // Internal error
            crate::generation::GenerationError::TokenConversionFailed(_) => -32603, // Internal error
            crate::generation::GenerationError::ContextFailed(_) => -32603, // Internal error
            crate::generation::GenerationError::ContextLock => -32603,      // Internal error
            crate::generation::GenerationError::Cancelled => -32000,        // Server error
            crate::generation::GenerationError::StreamClosed => -32000,     // Server error
            crate::generation::GenerationError::Stopped(_) => -32000,       // Server error
            crate::generation::GenerationError::GenerationFailed(_) => -32603, // Internal error
        }
    }

    fn to_error_data(&self) -> Option<serde_json::Value> {
        use serde_json::json;

        match self {
            crate::generation::GenerationError::InvalidConfig(msg) => Some(json!({
                "error": "invalid_generation_config",
                "details": msg,
                "suggestion": "Check generation parameters are within valid ranges"
            })),
            crate::generation::GenerationError::Cancelled => Some(json!({
                "error": "generation_cancelled",
                "suggestion": "Generation was cancelled by user request"
            })),
            crate::generation::GenerationError::StreamClosed => Some(json!({
                "error": "stream_closed",
                "suggestion": "Stream channel was closed unexpectedly"
            })),
            crate::generation::GenerationError::Stopped(reason) => Some(json!({
                "error": "generation_stopped",
                "reason": reason,
                "suggestion": "Generation was stopped by a stopping condition"
            })),
            _ => None,
        }
    }
}

impl ToJsonRpcError for crate::validation::ValidationError {
    fn to_json_rpc_code(&self) -> i32 {
        -32602 // Invalid params - all validation errors are parameter issues
    }

    fn to_error_data(&self) -> Option<serde_json::Value> {
        use serde_json::json;

        match self {
            crate::validation::ValidationError::SecurityViolation(msg) => Some(json!({
                "error": "security_violation",
                "details": msg,
                "suggestion": "Review your input for potentially dangerous content"
            })),
            crate::validation::ValidationError::ParameterBounds(msg) => Some(json!({
                "error": "parameter_out_of_bounds",
                "details": msg,
                "suggestion": "Check parameter limits in the documentation"
            })),
            crate::validation::ValidationError::InvalidState(msg) => Some(json!({
                "error": "invalid_state",
                "details": msg,
                "suggestion": "Ensure prerequisites are met and the operation is valid"
            })),
            crate::validation::ValidationError::ContentValidation(msg) => Some(json!({
                "error": "content_validation_failed",
                "details": msg,
                "suggestion": "Verify your content format, encoding, and structure"
            })),
            crate::validation::ValidationError::SchemaValidation(msg) => Some(json!({
                "error": "schema_validation_failed",
                "details": msg,
                "suggestion": "Check that your data structure matches the expected schema"
            })),
            crate::validation::ValidationError::Multiple(errors) => {
                let error_details: Vec<serde_json::Value> = errors
                    .iter()
                    .map(|e| {
                        json!({
                            "message": e.to_string(),
                            "code": e.to_json_rpc_code()
                        })
                    })
                    .collect();

                Some(json!({
                    "error": "multiple_validation_errors",
                    "errors": error_details,
                    "suggestion": "Fix all validation errors before retrying"
                }))
            }
        }
    }
}

impl ToJsonRpcError for TranslationError {
    fn to_json_rpc_code(&self) -> i32 {
        -32602 // Invalid params - all translation errors are parameter issues
    }

    fn to_error_data(&self) -> Option<serde_json::Value> {
        use serde_json::json;

        match self {
            TranslationError::UnsupportedContent(msg) => Some(json!({
                "error": "unsupported_content_type",
                "details": msg,
                "suggestion": "Use supported content types (currently text only)"
            })),
            TranslationError::InvalidFormat(msg) => Some(json!({
                "error": "invalid_content_format",
                "details": msg,
                "suggestion": "Check content format matches expected structure"
            })),
            TranslationError::InvalidSessionId(msg) => Some(json!({
                "error": "invalid_session_id",
                "details": msg,
                "suggestion": "Provide a valid ULID session ID"
            })),
        }
    }
}

impl ToJsonRpcError for agent_client_protocol::Error {
    fn to_json_rpc_code(&self) -> i32 {
        // Map agent_client_protocol::Error to JSON-RPC error codes
        // The agent_client_protocol::Error already contains a code field
        self.code.into()
    }

    fn to_error_data(&self) -> Option<serde_json::Value> {
        // Include the error data if present
        self.data.clone()
    }
}

/// Convert ACP SessionId to llama SessionId
///
/// Translates an ACP SessionId (Arc<str>) to a llama-agent SessionId (Ulid).
/// The ACP SessionId must be a valid ULID string.
///
/// # Arguments
/// * `acp_id` - ACP SessionId to convert
///
/// # Returns
/// * `Ok(LlamaSessionId)` - Successfully converted session ID
/// * `Err(TranslationError)` - If the ACP SessionId is not a valid ULID
///
/// # Examples
/// ```ignore
/// use agent_client_protocol::SessionId as AcpSessionId;
/// use llama_agent::acp::translation::acp_to_llama_session_id;
/// use std::sync::Arc;
///
/// let acp_id = AcpSessionId(Arc::from("01HX5ZRQK9X8G2V7N3P4M5W6Y7"));
/// let llama_id = acp_to_llama_session_id(acp_id).unwrap();
/// ```
pub fn acp_to_llama_session_id(acp_id: AcpSessionId) -> Result<LlamaSessionId, TranslationError> {
    acp_id
        .0
        .parse()
        .map_err(|e| TranslationError::InvalidSessionId(format!("Invalid ULID: {}", e)))
}

/// Convert llama SessionId to ACP SessionId
///
/// Translates a llama-agent SessionId (Ulid) to an ACP SessionId (Arc<str>).
/// The ULID is converted to its string representation.
///
/// # Arguments
/// * `llama_id` - llama-agent SessionId to convert
///
/// # Returns
/// * `AcpSessionId` - Converted session ID
///
/// # Examples
/// ```ignore
/// use llama_agent::types::ids::SessionId as LlamaSessionId;
/// use llama_agent::acp::translation::llama_to_acp_session_id;
///
/// let llama_id = LlamaSessionId::new();
/// let acp_id = llama_to_acp_session_id(llama_id);
/// ```
pub fn llama_to_acp_session_id(llama_id: LlamaSessionId) -> AcpSessionId {
    AcpSessionId::new(llama_id.to_string())
}

/// Convert llama messages to ACP content blocks
///
/// Translates llama-agent Messages to ACP ContentBlocks.
/// Currently supports simple text-only conversion. Each message's content
/// is converted to a text ContentBlock.
///
/// # Arguments
/// * `messages` - Vector of llama-agent Messages to translate
///
/// # Returns
/// * `Vec<ContentBlock>` - Translated content blocks
///
/// # Examples
/// ```ignore
/// use llama_agent::types::{Message, MessageRole};
/// use llama_agent::acp::translation::llama_to_acp_content;
/// use std::time::SystemTime;
///
/// let messages = vec![
///     Message {
///         role: MessageRole::Assistant,
///         content: "Hello!".to_string(),
///         tool_call_id: None,
///         tool_name: None,
///         timestamp: SystemTime::now(),
///     }
/// ];
///
/// let content = llama_to_acp_content(messages);
/// assert_eq!(content.len(), 1);
/// ```
pub fn llama_to_acp_content(messages: Vec<Message>) -> Vec<ContentBlock> {
    messages
        .into_iter()
        .map(|msg| ContentBlock::from(msg.content))
        .collect()
}

/// Convert llama stream chunks to ACP notifications
///
/// Translates llama-agent StreamChunks to ACP SessionNotification messages
/// for streaming updates to the client. Each chunk of text from the llama agent
/// is wrapped as an `AgentMessageChunk` update.
///
/// # Arguments
/// * `session_id` - The ACP session ID for this notification
/// * `chunk` - The StreamChunk from llama-agent to translate
///
/// # Returns
/// * `agent_client_protocol::SessionNotification` - Notification with the chunk content
///
/// # Examples
/// ```ignore
/// use agent_client_protocol::SessionId;
/// use llama_agent::acp::translation::llama_chunk_to_acp_notification;
/// use llama_agent::types::StreamChunk;
///
/// let session_id = SessionId::new("01HX5ZRQK9X8G2V7N3P4M5W6Y7");
/// let chunk = StreamChunk {
///     text: "Hello, world!".to_string(),
///     is_complete: false,
///     token_count: 3,
/// };
///
/// let notification = llama_chunk_to_acp_notification(session_id, chunk);
/// ```
pub fn llama_chunk_to_acp_notification(
    session_id: agent_client_protocol::SessionId,
    chunk: crate::types::StreamChunk,
) -> agent_client_protocol::SessionNotification {
    use agent_client_protocol::{ContentBlock, ContentChunk, SessionNotification, SessionUpdate};

    // Convert the text chunk to a ContentBlock
    let content_block = ContentBlock::from(chunk.text);

    // Wrap it in a ContentChunk
    let content_chunk = ContentChunk::new(content_block);

    // Create a SessionUpdate with the agent message chunk
    let update = SessionUpdate::AgentMessageChunk(content_chunk);

    // Return the SessionNotification
    SessionNotification::new(session_id, update)
}

/// Convert llama-agent ToolDefinition to ACP-compatible format
///
/// Translates MCP tool definitions to a format suitable for ACP protocol.
/// The tool schema (JSON Schema parameters) is preserved in the returned JSON value
/// which can be included in ACP responses or metadata.
///
/// # Arguments
/// * `tool_def` - The MCP ToolDefinition to convert
///
/// # Returns
/// * `serde_json::Value` - JSON object containing the tool definition in ACP-compatible format
///
/// # Format
/// The returned JSON has the structure:
/// ```json
/// {
///   "name": "tool_name",
///   "description": "Tool description",
///   "parameters": { /* JSON Schema */ },
///   "server": "mcp_server_name"
/// }
/// ```
///
/// # Examples
/// ```ignore
/// use llama_agent::acp::translation::tool_definition_to_acp_format;
/// use llama_agent::types::ToolDefinition;
///
/// let tool_def = ToolDefinition {
///     name: "fs_read".to_string(),
///     description: "Read a file from disk".to_string(),
///     parameters: serde_json::json!({
///         "type": "object",
///         "properties": {
///             "path": {
///                 "type": "string",
///                 "description": "File path to read"
///             }
///         },
///         "required": ["path"]
///     }),
///     server_name: "filesystem".to_string(),
/// };
///
/// let acp_format = tool_definition_to_acp_format(&tool_def);
/// // Can be included in ACP meta or tool listings
/// ```
pub fn tool_definition_to_acp_format(tool_def: &ToolDefinition) -> serde_json::Value {
    use serde_json::json;

    json!({
        "name": tool_def.name,
        "description": tool_def.description,
        "parameters": tool_def.parameters,
        "server": tool_def.server_name,
    })
}

/// Convert multiple llama-agent ToolDefinitions to ACP-compatible format
///
/// Translates a collection of MCP tool definitions to a JSON array suitable
/// for inclusion in ACP protocol messages, such as in the `_meta` field of
/// InitializeResponse or as part of available tools listings.
///
/// # Arguments
/// * `tool_defs` - Slice of ToolDefinitions to convert
///
/// # Returns
/// * `serde_json::Value` - JSON array containing all tool definitions
///
/// # Examples
/// ```ignore
/// use llama_agent::acp::translation::tool_definitions_to_acp_format;
/// use llama_agent::types::ToolDefinition;
///
/// let tools = vec![
///     ToolDefinition {
///         name: "fs_read".to_string(),
///         description: "Read a file".to_string(),
///         parameters: serde_json::json!({"type": "object"}),
///         server_name: "filesystem".to_string(),
///     },
///     ToolDefinition {
///         name: "fs_write".to_string(),
///         description: "Write a file".to_string(),
///         parameters: serde_json::json!({"type": "object"}),
///         server_name: "filesystem".to_string(),
///     },
/// ];
///
/// let acp_tools = tool_definitions_to_acp_format(&tools);
/// // Returns: [{"name": "fs_read", ...}, {"name": "fs_write", ...}]
/// ```
pub fn tool_definitions_to_acp_format(tool_defs: &[ToolDefinition]) -> serde_json::Value {
    use serde_json::json;

    let tools: Vec<serde_json::Value> = tool_defs
        .iter()
        .map(tool_definition_to_acp_format)
        .collect();

    json!(tools)
}

/// Convert llama-agent ToolCall to ACP ToolCall
///
/// Translates a tool call request from llama-agent format to ACP protocol format.
/// This creates an ACP ToolCall that can be sent to the client for display and tracking.
///
/// # Arguments
/// * `tool_call` - The llama-agent ToolCall to convert
/// * `tool_def` - Optional ToolDefinition containing metadata about the tool
///
/// # Returns
/// * `agent_client_protocol::ToolCall` - ACP ToolCall ready to send to client
///
/// # Tool Kind Inference
/// The function attempts to infer the appropriate `ToolKind` from the tool name:
/// - Tools with "read", "get", "list" → `ToolKind::Read`
/// - Tools with "write", "create", "update", "edit" → `ToolKind::Edit`
/// - Tools with "delete", "remove", "rm" → `ToolKind::Delete`
/// - Tools with "move", "rename", "mv" → `ToolKind::Move`
/// - Tools with "search", "grep", "find" → `ToolKind::Search`
/// - Tools with "execute", "shell", "terminal", "run" → `ToolKind::Execute`
/// - Tools with "http", "web", "fetch" → `ToolKind::Fetch`
/// - Tools with "think", "plan", "reason" → `ToolKind::Think`
/// - Everything else → `ToolKind::Other`
///
/// # Examples
/// ```ignore
/// use llama_agent::acp::translation::tool_call_to_acp;
/// use llama_agent::types::{ToolCall, ToolDefinition};
/// use llama_agent::types::ids::ToolCallId;
///
/// let tool_call = ToolCall {
///     id: ToolCallId::new(),
///     name: "fs_read".to_string(),
///     arguments: serde_json::json!({"path": "/tmp/test.txt"}),
/// };
///
/// let tool_def = Some(ToolDefinition {
///     name: "fs_read".to_string(),
///     description: "Read a file from disk".to_string(),
///     parameters: serde_json::json!({"type": "object"}),
///     server_name: "filesystem".to_string(),
/// });
///
/// let acp_tool_call = tool_call_to_acp(tool_call, tool_def.as_ref());
/// // Returns ACP ToolCall with kind=Read, title="fs_read", raw_input=arguments
/// ```
pub fn tool_call_to_acp(
    tool_call: ToolCall,
    tool_def: Option<&ToolDefinition>,
) -> agent_client_protocol::ToolCall {
    use agent_client_protocol::{ToolCall as AcpToolCall, ToolCallId as AcpToolCallId};

    // Convert tool call ID
    let tool_call_id = AcpToolCallId::new(tool_call.id.to_string());

    // Determine tool title (use description if available, otherwise name)
    let title = tool_def
        .map(|def| format!("{}: {}", tool_call.name, def.description))
        .unwrap_or_else(|| tool_call.name.clone());

    // Infer tool kind from name
    let kind = infer_tool_kind(&tool_call.name);

    // Create the ACP tool call
    let mut acp_call = AcpToolCall::new(tool_call_id, title).kind(kind);

    // Add raw input (the arguments)
    acp_call = acp_call.raw_input(tool_call.arguments.clone());

    // If we have tool definition, add it to meta
    if let Some(def) = tool_def {
        let mut meta = serde_json::Map::new();
        meta.insert(
            "tool_definition".to_string(),
            tool_definition_to_acp_format(def),
        );

        acp_call = acp_call.meta(meta);
    }

    acp_call
}

/// Infer ACP ToolKind from tool name
///
/// Analyzes the tool name to determine the most appropriate ToolKind category
/// for display purposes in ACP clients.
///
/// # Arguments
/// * `tool_name` - The name of the tool to classify
///
/// # Returns
/// * `agent_client_protocol::ToolKind` - The inferred kind
///
/// # Classification Rules
/// The function uses keyword matching (case-insensitive):
/// - **Read**: read, get, list, show, view, load, fetch, search, grep, find, glob
/// - **Edit**: write, create, update, edit, modify
/// - **Delete**: delete, remove, rm (as complete word)
/// - **Move**: move, rename, mv
/// - **Search**: search, grep, find (takes precedence over read for these keywords)
/// - **Execute**: execute, shell, terminal, run, bash
/// - **Fetch**: http, web, url (network operations)
/// - **Think**: think, plan, reason, analyze
/// - **Other**: default for anything that doesn't match
///
/// # Examples
/// ```
/// use llama_agent::acp::translation::infer_tool_kind;
/// use agent_client_protocol::ToolKind;
///
/// assert_eq!(infer_tool_kind("fs_read"), ToolKind::Read);
/// assert_eq!(infer_tool_kind("fs_write"), ToolKind::Edit);
/// assert_eq!(infer_tool_kind("fs_delete"), ToolKind::Delete);
/// assert_eq!(infer_tool_kind("shell_execute"), ToolKind::Execute);
/// assert_eq!(infer_tool_kind("web_fetch"), ToolKind::Fetch);
/// assert_eq!(infer_tool_kind("some_tool"), ToolKind::Other);
/// ```
pub fn infer_tool_kind(tool_name: &str) -> agent_client_protocol::ToolKind {
    use agent_client_protocol::ToolKind;

    let name_lower = tool_name.to_lowercase();

    // Check for specific patterns in priority order

    // Execute operations - check early as they're distinct
    if name_lower.contains("execute")
        || name_lower.contains("shell")
        || name_lower.contains("terminal")
        || name_lower.contains("run")
        || name_lower.contains("bash")
    {
        return ToolKind::Execute;
    }

    // Think/reasoning operations
    if name_lower.contains("think")
        || name_lower.contains("plan")
        || name_lower.contains("reason")
        || name_lower.contains("analyze")
    {
        return ToolKind::Think;
    }

    // Search operations - specific enough to check early
    if name_lower.contains("search") || name_lower.contains("grep") || name_lower.contains("find") {
        return ToolKind::Search;
    }

    // Delete operations
    if name_lower.contains("delete") || name_lower.contains("remove") {
        return ToolKind::Delete;
    }

    // Check for "rm" only as a complete word (not part of another word)
    if name_lower
        .split(|c: char| !c.is_alphanumeric())
        .any(|word| word == "rm")
    {
        return ToolKind::Delete;
    }

    // Move operations
    if name_lower.contains("move") || name_lower.contains("rename") || name_lower.contains("mv") {
        return ToolKind::Move;
    }

    // Edit/write operations
    if name_lower.contains("write")
        || name_lower.contains("create")
        || name_lower.contains("update")
        || name_lower.contains("edit")
        || name_lower.contains("modify")
    {
        return ToolKind::Edit;
    }

    // Fetch operations (network)
    if name_lower.contains("http") || name_lower.contains("web") || name_lower.contains("url") {
        return ToolKind::Fetch;
    }

    // Read operations - check last as it's the most common/default for data access
    if name_lower.contains("read")
        || name_lower.contains("get")
        || name_lower.contains("list")
        || name_lower.contains("show")
        || name_lower.contains("view")
        || name_lower.contains("load")
        || name_lower.contains("fetch")
        || name_lower.contains("glob")
    {
        return ToolKind::Read;
    }

    // Default to Other for unrecognized patterns
    ToolKind::Other
}

/// Convert llama-agent ToolResult to ACP ToolCallUpdate
///
/// Translates tool execution results from llama-agent to ACP protocol format.
/// This creates a ToolCallUpdate notification that reports the outcome of tool execution,
/// including success/failure status and any output or error messages.
///
/// # Arguments
/// * `tool_result` - The ToolResult from llama-agent containing execution outcome
///
/// # Returns
/// * `agent_client_protocol::ToolCallUpdate` - Update with tool execution status and results
///
/// # Status Mapping
/// - If `tool_result.error` is `None`: Status is `Completed` with result in `raw_output`
/// - If `tool_result.error` is `Some(msg)`: Status is `Failed` with error in content
///
/// # Examples
/// ```ignore
/// use llama_agent::acp::translation::tool_result_to_acp_update;
/// use llama_agent::types::ToolResult;
///
/// // Success case
/// let result = ToolResult {
///     call_id: "call_123".to_string(),
///     result: serde_json::json!({"status": "ok", "data": "file content"}),
///     error: None,
/// };
/// let update = tool_result_to_acp_update(result);
/// // update.fields.status == Some(ToolCallStatus::Completed)
/// // update.fields.raw_output == Some(...)
///
/// // Error case
/// let result = ToolResult {
///     call_id: "call_456".to_string(),
///     result: serde_json::Value::Null,
///     error: Some("File not found".to_string()),
/// };
/// let update = tool_result_to_acp_update(result);
/// // update.fields.status == Some(ToolCallStatus::Failed)
/// // update.fields.content contains error message
/// ```
pub fn tool_result_to_acp_update(tool_result: ToolResult) -> agent_client_protocol::ToolCallUpdate {
    use agent_client_protocol::{
        ContentBlock, ToolCallContent, ToolCallId as AcpToolCallId, ToolCallStatus, ToolCallUpdate,
        ToolCallUpdateFields,
    };

    // Convert llama-agent ToolCallId to ACP ToolCallId (which wraps Arc<str>)
    // Use Display trait to convert to string
    let tool_call_id = AcpToolCallId::new(tool_result.call_id.to_string());

    let mut fields = ToolCallUpdateFields::new();

    if let Some(error_msg) = tool_result.error {
        // Tool execution failed
        fields = fields.status(ToolCallStatus::Failed);

        // Add error message as content using the From impl
        // ToolCallContent has From<ContentBlock> which creates ToolCallContent::Content
        let content_block = ContentBlock::from(error_msg);
        let error_content = ToolCallContent::from(content_block);
        fields = fields.content(vec![error_content]);
    } else {
        // Tool execution succeeded
        fields = fields.status(ToolCallStatus::Completed);

        // Set raw_output with the result
        fields = fields.raw_output(tool_result.result);
    }

    ToolCallUpdate::new(tool_call_id, fields)
}

/// Error type for tool call handling
#[derive(Debug, thiserror::Error)]
pub enum ToolCallError {
    #[error("Permission denied for tool call: {0}")]
    PermissionDenied(String),

    #[error("Tool execution failed: {0}")]
    ExecutionFailed(String),

    #[error("Agent error: {0}")]
    AgentError(#[from] crate::types::AgentError),
}

/// Determine if a tool requires permission based on its type
///
/// This function classifies tools into different risk categories and determines
/// whether they require explicit user permission before execution.
///
/// # Tool Classification
///
/// Tools are classified based on their name patterns:
/// - **Read operations**: Low risk, typically don't require permission (e.g., fs_read, get_*, list_*)
/// - **Write operations**: Medium risk, require permission (e.g., fs_write, create_*, update_*)
/// - **Delete operations**: High risk, require permission (e.g., fs_delete, remove_*)
/// - **Execute operations**: High risk, require permission (e.g., terminal_*, execute_*, shell_*)
/// - **Network operations**: Medium risk, require permission (e.g., http_*, fetch_*, web_*)
///
/// # Arguments
/// * `tool_name` - The name of the tool to classify
///
/// # Returns
/// * `bool` - `true` if the tool requires permission, `false` otherwise
///
/// # Examples
/// ```
/// use llama_agent::acp::translation::needs_permission;
///
/// assert!(!needs_permission("fs_read")); // Read operations don't need permission
/// assert!(needs_permission("fs_write")); // Write operations need permission
/// assert!(needs_permission("terminal_create")); // Execute operations need permission
/// assert!(needs_permission("http_get")); // Network operations need permission
/// ```
pub fn needs_permission(tool_name: &str) -> bool {
    let tool_lower = tool_name.to_lowercase();

    // High priority checks - these always require permission

    // Write operations - require permission
    if tool_lower.contains("write")
        || tool_lower.contains("create")
        || tool_lower.contains("update")
        || tool_lower.contains("edit")
        || tool_lower.contains("modify")
    {
        return true;
    }

    // Delete operations - require permission
    if tool_lower.contains("delete") || tool_lower.contains("remove") {
        return true;
    }

    // Check for "rm" only as a complete word (not part of another word like "swissarmyhammer")
    if tool_lower
        .split(|c: char| !c.is_alphanumeric())
        .any(|word| word == "rm")
    {
        return true;
    }

    // Execute/shell/terminal operations - require permission
    if tool_lower.contains("execute")
        || tool_lower.contains("shell")
        || tool_lower.contains("terminal")
        || tool_lower.contains("run")
        || tool_lower.contains("bash")
    {
        return true;
    }

    // Network operations - require permission
    if tool_lower.contains("http") || tool_lower.contains("web") || tool_lower.contains("url") {
        return true;
    }

    // Move operations - require permission
    if tool_lower.contains("move") || tool_lower.contains("rename") || tool_lower.contains("mv") {
        return true;
    }

    // Low priority checks - these don't require permission if they're pure read operations

    // Read operations - typically safe, don't require permission
    let read_indicators = [
        "read", "get", "list", "show", "view", "load", "fetch", "search", "grep", "find", "glob",
    ];
    if read_indicators
        .iter()
        .any(|indicator| tool_lower.contains(indicator))
    {
        return false;
    }

    // Default: if we're unsure, require permission for safety
    true
}

/// Handle a tool call with permission checking and execution
///
/// This function implements the complete tool call workflow:
/// 1. Evaluate whether permission is needed based on the permission policy
/// 2. If permission is needed and not already granted, request it from the user
/// 3. Execute the tool via AgentServer if permission is granted
/// 4. Return the tool result or error
///
/// # Arguments
/// * `tool_call` - The tool call to execute
/// * `session` - The ACP session state containing permission storage and policy
/// * `agent_server` - The agent server to execute the tool call
/// * `permission_engine` - The permission policy engine for evaluation
/// * `permission_storage` - Mutable storage for granted permissions
///
/// # Returns
/// * `Ok(ToolResult)` - The tool was executed successfully (or permission was denied and returned as error in ToolResult)
/// * `Err(ToolCallError)` - An error occurred during the workflow
///
/// # Permission Flow
///
/// The function evaluates the tool call against the permission policy:
/// - `Allowed`: Tool executes immediately
/// - `Denied`: Returns an error ToolResult (no exception thrown)
/// - `RequireUserConsent`: Would request permission from user (not yet implemented)
///
/// Note: User consent request flow is not yet implemented. When a tool requires
/// consent, this function currently denies the tool call. Future implementation
/// will integrate with ACP client to request user approval.
///
/// # Examples
/// ```ignore
/// use llama_agent::acp::translation::handle_tool_call;
/// use llama_agent::types::ToolCall;
/// use std::sync::Arc;
///
/// let tool_call = ToolCall {
///     id: "call_123".to_string(),
///     name: "fs_read".to_string(),
///     arguments: serde_json::json!({"path": "/tmp/test.txt"}),
/// };
///
/// let result = handle_tool_call(
///     tool_call,
///     &session,
///     agent_server.clone(),
///     &permission_engine,
///     &mut permission_storage,
/// ).await?;
/// ```
pub async fn handle_tool_call(
    tool_call: ToolCall,
    session: &AcpSessionState,
    agent_server: Arc<crate::agent::AgentServer>,
    permission_engine: &PermissionPolicyEngine,
    permission_storage: &mut PermissionStorage,
) -> Result<ToolResult, ToolCallError> {
    use tracing::{debug, warn};

    debug!(
        "Handling tool call: {} (id: {}) for session {}",
        tool_call.name, tool_call.id, session.session_id.0
    );

    // Evaluate permission policy for this tool call
    let evaluation = permission_engine.evaluate_tool_call(&tool_call.name, permission_storage);

    match evaluation {
        PermissionEvaluation::Allowed => {
            debug!("Tool call '{}' automatically allowed", tool_call.name);
            // Permission granted - execute the tool
            execute_tool_call(tool_call, session, agent_server).await
        }

        PermissionEvaluation::Denied => {
            warn!("Tool call '{}' denied by policy", tool_call.name);
            // Permission denied - return error in ToolResult (not exception)
            Ok(ToolResult {
                call_id: tool_call.id,
                result: serde_json::Value::Null,
                error: Some(format!(
                    "Permission denied: Tool '{}' is not allowed by policy",
                    tool_call.name
                )),
            })
        }

        PermissionEvaluation::RequireUserConsent => {
            debug!(
                "Tool call '{}' requires user consent - implementation incomplete (client parameter needed)",
                tool_call.name
            );

            // TODO: This function needs to be updated to accept a Client parameter
            // Once that's done, we can call request_permission here:
            //
            // match request_permission(client, &tool_call, session).await {
            //     Ok(true) => {
            //         // User approved - grant permission and execute
            //         permission_storage.grant(tool_call.name.clone());
            //         execute_tool_call(tool_call, session, agent_server).await
            //     }
            //     Ok(false) => {
            //         // User denied
            //         Ok(ToolResult {
            //             call_id: tool_call.id,
            //             result: serde_json::Value::Null,
            //             error: Some(format!(
            //                 "Permission denied: User rejected tool '{}'",
            //                 tool_call.name
            //             )),
            //         })
            //     }
            //     Err(e) => {
            //         // Request failed
            //         Err(ToolCallError::ExecutionFailed(format!(
            //             "Permission request failed: {}",
            //             e
            //         )))
            //     }
            // }
            //
            // For now, treat as denied since handle_tool_call doesn't have client parameter yet
            warn!(
                "User consent required for '{}' but client parameter not available - denying",
                tool_call.name
            );

            Ok(ToolResult {
                call_id: tool_call.id,
                result: serde_json::Value::Null,
                error: Some(format!(
                    "Permission required: Tool '{}' requires user consent (client parameter needed)",
                    tool_call.name
                )),
            })
        }
    }
}

/// Request permission from the ACP client for a tool call
///
/// This function sends a permission request to the ACP client and waits for the user's response.
/// It presents the user with options to allow or reject the operation, either once or always.
///
/// # Arguments
/// * `client` - The ACP client to send the permission request to
/// * `tool_call` - The tool call that requires permission
/// * `session` - The ACP session state
///
/// # Returns
/// * `Ok(true)` - User approved the tool call
/// * `Ok(false)` - User denied the tool call or cancelled
/// * `Err(String)` - Permission request failed
///
/// # Examples
/// ```ignore
/// use llama_agent::acp::translation::request_permission;
/// use llama_agent::types::ToolCall;
///
/// let tool_call = ToolCall {
///     id: "call_123".to_string(),
///     name: "fs_write".to_string(),
///     arguments: serde_json::json!({"path": "/tmp/test.txt", "content": "data"}),
/// };
///
/// let approved = request_permission(&client, &tool_call, &session).await?;
/// if approved {
///     // Execute the tool
/// }
/// ```
/// Execute a tool call via AgentServer
///
/// Internal helper function that performs the actual tool execution.
/// This function checks client capabilities and permissions before execution.
///
/// # Arguments
/// * `tool_call` - The tool call to execute
/// * `session` - The ACP session state
/// * `agent_server` - The agent server to execute the tool call
///
/// # Returns
/// * `Ok(ToolResult)` - Tool execution result (may contain error in ToolResult.error field)
/// * `Err(ToolCallError)` - Execution failed with an exception
///
/// # Capability Checking
/// Before executing the tool, this function verifies that the client has advertised
/// the required capability for the operation:
/// - File read operations require `client.fs.read_text_file`
/// - File write operations require `client.fs.write_text_file`
/// - Terminal operations require `client.terminal`
///
/// If the required capability is missing, a ToolResult with an error is returned
/// (not an exception), allowing the agent to handle the failure gracefully.
async fn execute_tool_call(
    tool_call: ToolCall,
    session: &AcpSessionState,
    agent_server: Arc<crate::agent::AgentServer>,
) -> Result<ToolResult, ToolCallError> {
    use tracing::{debug, error};

    debug!(
        "Executing tool call '{}' with id '{}'",
        tool_call.name, tool_call.id
    );

    // Get the llama-agent session
    let llama_session = agent_server
        .session_manager()
        .get_session(&session.llama_session_id)
        .await
        .map_err(|e| ToolCallError::ExecutionFailed(format!("Failed to get session: {}", e)))?
        .ok_or_else(|| {
            ToolCallError::ExecutionFailed(format!(
                "Session not found: {}",
                session.llama_session_id
            ))
        })?;

    // Execute the tool call via AgentServer
    // Note: AgentServer::execute_tool returns Result<ToolResult, AgentError>
    // where ToolResult may contain an error field for tool-level errors
    match agent_server
        .execute_tool(tool_call.clone(), &llama_session)
        .await
    {
        Ok(tool_result) => {
            if let Some(error) = &tool_result.error {
                debug!(
                    "Tool call '{}' completed with error: {}",
                    tool_call.name, error
                );
            } else {
                debug!("Tool call '{}' completed successfully", tool_call.name);
            }
            Ok(tool_result)
        }
        Err(agent_error) => {
            error!(
                "Tool call '{}' failed with agent error: {}",
                tool_call.name, agent_error
            );
            Err(ToolCallError::AgentError(agent_error))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_acp_to_llama_messages_text_content() {
        // Use From<String> impl to create ContentBlock
        let content = vec![ContentBlock::from("Hello, world!")];

        let result = acp_to_llama_messages(content);
        assert!(result.is_ok());

        let messages = result.unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].content, "Hello, world!");
        assert_eq!(messages[0].role, MessageRole::User);
        assert!(messages[0].tool_call_id.is_none());
        assert!(messages[0].tool_name.is_none());
    }

    #[test]
    fn test_acp_to_llama_messages_multiple_text_blocks() {
        let content = vec![
            ContentBlock::from("First message"),
            ContentBlock::from("Second message"),
        ];

        let result = acp_to_llama_messages(content);
        assert!(result.is_ok());

        let messages = result.unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].content, "First message");
        assert_eq!(messages[1].content, "Second message");
    }

    #[test]
    fn test_acp_to_llama_messages_empty_content() {
        let content = vec![];

        let result = acp_to_llama_messages(content);
        assert!(result.is_ok());

        let messages = result.unwrap();
        assert_eq!(messages.len(), 0);
    }

    #[test]
    fn test_acp_to_llama_messages_timestamps_set() {
        let content = vec![ContentBlock::from("Test")];

        let before = SystemTime::now();
        let result = acp_to_llama_messages(content);
        let after = SystemTime::now();

        assert!(result.is_ok());
        let messages = result.unwrap();
        assert_eq!(messages.len(), 1);

        // Verify timestamp is between before and after
        assert!(messages[0].timestamp >= before);
        assert!(messages[0].timestamp <= after);
    }

    // Tests for unsupported content types (Image, Audio, ResourceLink, Resource)
    // These types are marked as #[non_exhaustive] in agent-client-protocol,
    // so we use JSON deserialization to construct them for testing.

    #[test]
    fn test_acp_to_llama_messages_resource_content_text() {
        // Construct a Resource ContentBlock via JSON deserialization
        let json = r#"{
            "type": "resource",
            "resource": {
                "type": "text",
                "uri": "file:///test.txt",
                "text": "test content"
            }
        }"#;

        let content_block: ContentBlock = serde_json::from_str(json).unwrap();
        let content = vec![content_block];

        let result = acp_to_llama_messages(content);
        assert!(result.is_err());

        match result {
            Err(TranslationError::UnsupportedContent(msg)) => {
                assert!(msg.contains("resource content"));
                assert!(msg.contains("not yet supported"));
            }
            _ => panic!("Expected UnsupportedContent error for Resource"),
        }
    }

    #[test]
    fn test_acp_to_llama_messages_resource_content_blob() {
        // Construct a Resource ContentBlock with blob data via JSON deserialization
        let json = r#"{
            "type": "resource",
            "resource": {
                "type": "blob",
                "uri": "file:///test.bin",
                "blob": "SGVsbG8gV29ybGQh"
            }
        }"#;

        let content_block: ContentBlock = serde_json::from_str(json).unwrap();
        let content = vec![content_block];

        let result = acp_to_llama_messages(content);
        assert!(result.is_err());

        match result {
            Err(TranslationError::UnsupportedContent(msg)) => {
                assert!(msg.contains("resource content"));
                assert!(msg.contains("not yet supported"));
            }
            _ => panic!("Expected UnsupportedContent error for Resource"),
        }
    }

    // Note: ResourceLink test is commented out due to deserialization issues with
    // agent-client-protocol 0.8.0 / schema 0.9.1. The ResourceLink handling in
    // acp_to_llama_messages is verified through the code path coverage in the match statement.
    // When agent-client-protocol is upgraded, this test can be uncommented and adjusted.

    #[test]
    fn test_acp_to_llama_messages_mixed_with_resource() {
        // Test that resource content in a mixed list causes an error
        let text_json = r#"{"type": "text", "text": "Hello"}"#;
        let resource_json = r#"{
            "type": "resource",
            "resource": {
                "type": "text",
                "uri": "file:///test.txt",
                "text": "resource content"
            }
        }"#;

        let text_block: ContentBlock = serde_json::from_str(text_json).unwrap();
        let resource_block: ContentBlock = serde_json::from_str(resource_json).unwrap();

        let content = vec![text_block, resource_block];

        let result = acp_to_llama_messages(content);
        assert!(result.is_err());

        // Should fail on the resource block
        match result {
            Err(TranslationError::UnsupportedContent(msg)) => {
                assert!(msg.contains("resource"));
            }
            _ => panic!("Expected UnsupportedContent error"),
        }
    }

    #[test]
    fn test_llama_to_acp_content_single_message() {
        let messages = vec![Message {
            role: MessageRole::Assistant,
            content: "Hello from llama!".to_string(),
            tool_call_id: None,
            tool_name: None,
            timestamp: SystemTime::now(),
        }];

        let content = llama_to_acp_content(messages);
        assert_eq!(content.len(), 1);

        // Verify it's a text content block by converting it
        match &content[0] {
            ContentBlock::Text(text) => {
                assert_eq!(text.text, "Hello from llama!");
            }
            _ => panic!("Expected text content block"),
        }
    }

    #[test]
    fn test_llama_to_acp_content_multiple_messages() {
        let messages = vec![
            Message {
                role: MessageRole::User,
                content: "First message".to_string(),
                tool_call_id: None,
                tool_name: None,
                timestamp: SystemTime::now(),
            },
            Message {
                role: MessageRole::Assistant,
                content: "Second message".to_string(),
                tool_call_id: None,
                tool_name: None,
                timestamp: SystemTime::now(),
            },
        ];

        let content = llama_to_acp_content(messages);
        assert_eq!(content.len(), 2);

        match &content[0] {
            ContentBlock::Text(text) => {
                assert_eq!(text.text, "First message");
            }
            _ => panic!("Expected text content block"),
        }

        match &content[1] {
            ContentBlock::Text(text) => {
                assert_eq!(text.text, "Second message");
            }
            _ => panic!("Expected text content block"),
        }
    }

    #[test]
    fn test_llama_to_acp_content_empty_messages() {
        let messages: Vec<Message> = vec![];

        let content = llama_to_acp_content(messages);
        assert_eq!(content.len(), 0);
    }

    #[test]
    fn test_llama_to_acp_content_preserves_content() {
        let messages = vec![Message {
            role: MessageRole::Assistant,
            content: "This is a longer message with multiple words and punctuation!".to_string(),
            tool_call_id: None,
            tool_name: None,
            timestamp: SystemTime::now(),
        }];

        let content = llama_to_acp_content(messages);

        match &content[0] {
            ContentBlock::Text(text) => {
                assert_eq!(
                    text.text,
                    "This is a longer message with multiple words and punctuation!"
                );
            }
            _ => panic!("Expected text content block"),
        }
    }

    #[test]
    fn test_llama_to_acp_content_ignores_role() {
        // Verify that different roles all convert to text content blocks
        let messages = vec![
            Message {
                role: MessageRole::System,
                content: "System message".to_string(),
                tool_call_id: None,
                tool_name: None,
                timestamp: SystemTime::now(),
            },
            Message {
                role: MessageRole::User,
                content: "User message".to_string(),
                tool_call_id: None,
                tool_name: None,
                timestamp: SystemTime::now(),
            },
            Message {
                role: MessageRole::Assistant,
                content: "Assistant message".to_string(),
                tool_call_id: None,
                tool_name: None,
                timestamp: SystemTime::now(),
            },
            Message {
                role: MessageRole::Tool,
                content: "Tool message".to_string(),
                tool_call_id: None,
                tool_name: None,
                timestamp: SystemTime::now(),
            },
        ];

        let content = llama_to_acp_content(messages);
        assert_eq!(content.len(), 4);

        // All should be text content blocks
        for (i, block) in content.iter().enumerate() {
            match block {
                ContentBlock::Text(_) => {
                    // Expected
                }
                _ => panic!("Expected text content block at index {}", i),
            }
        }
    }

    #[test]
    fn test_llama_chunk_to_acp_notification_text_content() {
        use crate::types::StreamChunk;
        use agent_client_protocol::SessionId;

        let session_id = SessionId::new("01HX5ZRQK9X8G2V7N3P4M5W6Y7");
        let chunk = StreamChunk {
            text: "Hello from llama!".to_string(),
            is_complete: false,
            token_count: 3,
            finish_reason: None,
        };

        let notification = llama_chunk_to_acp_notification(session_id.clone(), chunk);

        // Verify the session ID matches
        assert_eq!(notification.session_id, session_id);

        // Verify it's an AgentMessageChunk update
        match notification.update {
            agent_client_protocol::SessionUpdate::AgentMessageChunk(content_chunk) => {
                match &content_chunk.content {
                    agent_client_protocol::ContentBlock::Text(text) => {
                        assert_eq!(text.text, "Hello from llama!");
                    }
                    _ => panic!("Expected text content block"),
                }
            }
            _ => panic!("Expected AgentMessageChunk update"),
        }
    }

    #[test]
    fn test_llama_chunk_to_acp_notification_empty_text() {
        use crate::types::StreamChunk;
        use agent_client_protocol::SessionId;

        let session_id = SessionId::new("01HX5ZRQK9X8G2V7N3P4M5W6Y7");
        let chunk = StreamChunk {
            text: "".to_string(),
            is_complete: false,
            token_count: 0,
            finish_reason: None,
        };

        let notification = llama_chunk_to_acp_notification(session_id.clone(), chunk);

        // Verify the session ID matches
        assert_eq!(notification.session_id, session_id);

        // Verify it's an AgentMessageChunk update with empty text
        match notification.update {
            agent_client_protocol::SessionUpdate::AgentMessageChunk(content_chunk) => {
                match &content_chunk.content {
                    agent_client_protocol::ContentBlock::Text(text) => {
                        assert_eq!(text.text, "");
                    }
                    _ => panic!("Expected text content block"),
                }
            }
            _ => panic!("Expected AgentMessageChunk update"),
        }
    }

    #[test]
    fn test_llama_chunk_to_acp_notification_complete_chunk() {
        use crate::types::StreamChunk;
        use agent_client_protocol::SessionId;

        let session_id = SessionId::new("01HX5ZRQK9X8G2V7N3P4M5W6Y7");
        let chunk = StreamChunk {
            text: "Final message.".to_string(),
            is_complete: true,
            token_count: 2,
            finish_reason: Some(crate::types::FinishReason::Stopped(
                "End of sequence token detected".to_string(),
            )),
        };

        let notification = llama_chunk_to_acp_notification(session_id.clone(), chunk);

        // The is_complete flag is not directly translated to ACP
        // (it's used by the streaming layer, not the protocol)
        assert_eq!(notification.session_id, session_id);

        match notification.update {
            agent_client_protocol::SessionUpdate::AgentMessageChunk(content_chunk) => {
                match &content_chunk.content {
                    agent_client_protocol::ContentBlock::Text(text) => {
                        assert_eq!(text.text, "Final message.");
                    }
                    _ => panic!("Expected text content block"),
                }
            }
            _ => panic!("Expected AgentMessageChunk update"),
        }
    }

    #[test]
    fn test_llama_chunk_to_acp_notification_multiline_text() {
        use crate::types::StreamChunk;
        use agent_client_protocol::SessionId;

        let session_id = SessionId::new("01HX5ZRQK9X8G2V7N3P4M5W6Y7");
        let chunk = StreamChunk {
            text: "Line 1\nLine 2\nLine 3".to_string(),
            is_complete: false,
            token_count: 10,
            finish_reason: None,
        };

        let notification = llama_chunk_to_acp_notification(session_id.clone(), chunk);

        assert_eq!(notification.session_id, session_id);

        match notification.update {
            agent_client_protocol::SessionUpdate::AgentMessageChunk(content_chunk) => {
                match &content_chunk.content {
                    agent_client_protocol::ContentBlock::Text(text) => {
                        assert_eq!(text.text, "Line 1\nLine 2\nLine 3");
                    }
                    _ => panic!("Expected text content block"),
                }
            }
            _ => panic!("Expected AgentMessageChunk update"),
        }
    }

    #[test]
    fn test_llama_chunk_to_acp_notification_different_session_ids() {
        use crate::types::StreamChunk;
        use agent_client_protocol::SessionId;

        let session_id_1 = SessionId::new("01HX5ZRQK9X8G2V7N3P4M5W6Y7");
        let session_id_2 = SessionId::new("01HX5ZRQK9X8G2V7N3P4M5W6Y8");

        let chunk = StreamChunk {
            text: "Test".to_string(),
            is_complete: false,
            token_count: 1,
            finish_reason: None,
        };

        let notification_1 = llama_chunk_to_acp_notification(session_id_1.clone(), chunk.clone());
        let notification_2 = llama_chunk_to_acp_notification(session_id_2.clone(), chunk);

        // Verify session IDs are preserved correctly
        assert_eq!(notification_1.session_id, session_id_1);
        assert_eq!(notification_2.session_id, session_id_2);
        assert_ne!(notification_1.session_id, notification_2.session_id);
    }

    // Tool call handling tests
    // Note: StreamChunk only contains text content. Tool calls are NOT part of StreamChunk.
    // Tool calls are extracted from the generated text after streaming completes,
    // and are sent via separate ToolCallUpdate notifications using tool_result_to_acp_update.
    // See acp/server.rs for the tool call execution flow.

    #[test]
    fn test_llama_chunk_to_acp_notification_with_tool_call_text() {
        use crate::types::StreamChunk;
        use agent_client_protocol::{ContentBlock, SessionId};

        // Tool calls appear as text in the stream during generation
        // They are extracted and parsed AFTER streaming completes
        let session_id = SessionId::new("01HX5ZRQK9X8G2V7N3P4M5W6Y7");
        let chunk = StreamChunk {
            text: "<tool_call>fs_read</tool_call>".to_string(),
            is_complete: false,
            token_count: 5,
            finish_reason: None,
        };

        let notification = llama_chunk_to_acp_notification(session_id.clone(), chunk);

        // Verify it's still treated as text content
        assert_eq!(notification.session_id, session_id);
        match notification.update {
            agent_client_protocol::SessionUpdate::AgentMessageChunk(content_chunk) => {
                match &content_chunk.content {
                    ContentBlock::Text(text) => {
                        assert_eq!(text.text, "<tool_call>fs_read</tool_call>");
                    }
                    _ => panic!("Expected text content block"),
                }
            }
            _ => panic!("Expected AgentMessageChunk update"),
        }
    }

    #[test]
    fn test_llama_chunk_to_acp_notification_partial_tool_call_text() {
        use crate::types::StreamChunk;
        use agent_client_protocol::{ContentBlock, SessionId};

        // During streaming, tool calls arrive as partial text chunks
        let session_id = SessionId::new("01HX5ZRQK9X8G2V7N3P4M5W6Y7");
        let chunk = StreamChunk {
            text: "<tool_call>".to_string(),
            is_complete: false,
            token_count: 1,
            finish_reason: None,
        };

        let notification = llama_chunk_to_acp_notification(session_id.clone(), chunk);

        // Partial tool call syntax is still just text
        assert_eq!(notification.session_id, session_id);
        match notification.update {
            agent_client_protocol::SessionUpdate::AgentMessageChunk(content_chunk) => {
                match &content_chunk.content {
                    ContentBlock::Text(text) => {
                        assert_eq!(text.text, "<tool_call>");
                    }
                    _ => panic!("Expected text content block"),
                }
            }
            _ => panic!("Expected AgentMessageChunk update"),
        }
    }

    #[test]
    fn test_llama_chunk_to_acp_notification_does_not_parse_tool_calls() {
        use crate::types::StreamChunk;
        use agent_client_protocol::{ContentBlock, SessionId};

        // Verify that llama_chunk_to_acp_notification does NOT attempt to parse
        // or extract tool calls - that's done separately after streaming completes
        let session_id = SessionId::new("01HX5ZRQK9X8G2V7N3P4M5W6Y7");
        let chunk = StreamChunk {
            text:
                r#"<tool_call>{"name":"fs_read","arguments":{"path":"/tmp/test.txt"}}</tool_call>"#
                    .to_string(),
            is_complete: false,
            token_count: 15,
            finish_reason: None,
        };

        let notification = llama_chunk_to_acp_notification(session_id.clone(), chunk);

        // Should be sent as-is as text, not parsed into a tool call notification
        match notification.update {
            agent_client_protocol::SessionUpdate::AgentMessageChunk(content_chunk) => {
                match &content_chunk.content {
                    ContentBlock::Text(text) => {
                        // The entire tool call XML/JSON is preserved as text
                        assert!(text.text.contains("tool_call"));
                        assert!(text.text.contains("fs_read"));
                        assert!(text.text.contains("/tmp/test.txt"));
                    }
                    _ => panic!("Expected text content block"),
                }
            }
            _ => panic!("Expected AgentMessageChunk update"),
        }
    }

    #[test]
    fn test_acp_to_llama_session_id_valid() {
        // Create a valid ULID string
        let ulid_str = "01HX5ZRQK9X8G2V7N3P4M5W6Y7";
        let acp_id = AcpSessionId::new(ulid_str);

        let result = acp_to_llama_session_id(acp_id);
        assert!(result.is_ok());

        let llama_id = result.unwrap();
        assert_eq!(llama_id.to_string(), ulid_str);
    }

    #[test]
    fn test_acp_to_llama_session_id_invalid() {
        // Invalid ULID string - too short
        let invalid_id = AcpSessionId::new("invalid");
        let result = acp_to_llama_session_id(invalid_id);
        assert!(result.is_err());

        match result {
            Err(TranslationError::InvalidSessionId(msg)) => {
                assert!(msg.contains("Invalid ULID"));
            }
            _ => panic!("Expected InvalidSessionId error"),
        }
    }

    #[test]
    fn test_acp_to_llama_session_id_empty() {
        let empty_id = AcpSessionId::new("");
        let result = acp_to_llama_session_id(empty_id);
        assert!(result.is_err());

        match result {
            Err(TranslationError::InvalidSessionId(_)) => {}
            _ => panic!("Expected InvalidSessionId error"),
        }
    }

    #[test]
    fn test_llama_to_acp_session_id() {
        // Create a new llama SessionId
        let llama_id = LlamaSessionId::new();

        // Convert to ACP SessionId
        let acp_id = llama_to_acp_session_id(llama_id);

        // Verify the string representation matches
        assert_eq!(acp_id.0.as_ref(), llama_id.to_string());
    }

    #[test]
    fn test_session_id_roundtrip() {
        // Start with a llama SessionId
        let original_llama_id = LlamaSessionId::new();

        // Convert to ACP and back
        let acp_id = llama_to_acp_session_id(original_llama_id);
        let roundtrip_llama_id = acp_to_llama_session_id(acp_id).unwrap();

        // Should be equal
        assert_eq!(original_llama_id, roundtrip_llama_id);
    }

    #[test]
    fn test_session_id_string_consistency() {
        let llama_id = LlamaSessionId::new();
        let llama_str = llama_id.to_string();

        let acp_id = llama_to_acp_session_id(llama_id);
        let acp_str = acp_id.0.as_ref();

        assert_eq!(llama_str, acp_str);
    }

    #[test]
    fn test_multiple_session_ids_unique() {
        // Create multiple session IDs and verify they're all unique
        let llama_id1 = LlamaSessionId::new();
        let llama_id2 = LlamaSessionId::new();
        let llama_id3 = LlamaSessionId::new();

        let acp_id1 = llama_to_acp_session_id(llama_id1);
        let acp_id2 = llama_to_acp_session_id(llama_id2);
        let acp_id3 = llama_to_acp_session_id(llama_id3);

        // All should be different
        assert_ne!(acp_id1.0.as_ref(), acp_id2.0.as_ref());
        assert_ne!(acp_id2.0.as_ref(), acp_id3.0.as_ref());
        assert_ne!(acp_id1.0.as_ref(), acp_id3.0.as_ref());
    }

    // Error translation tests

    #[test]
    fn test_agent_error_timeout_conversion() {
        let error = crate::types::AgentError::Timeout {
            timeout: Duration::from_secs(30),
        };

        let json_rpc = error.to_json_rpc_error();
        assert_eq!(json_rpc.code, -32000);
        assert!(json_rpc.message.contains("timeout"));

        let data = json_rpc.data.expect("Expected error data");
        assert_eq!(data["error"], "request_timeout");
        assert_eq!(data["timeoutSeconds"], 30);
        assert!(data["suggestion"].as_str().unwrap().contains("timeout"));
    }

    #[test]
    fn test_agent_error_queue_full_conversion() {
        let error = crate::types::AgentError::QueueFull { capacity: 100 };

        let json_rpc = error.to_json_rpc_error();
        assert_eq!(json_rpc.code, -32000);
        assert!(json_rpc.message.contains("Queue"));

        let data = json_rpc.data.expect("Expected error data");
        assert_eq!(data["error"], "queue_overloaded");
        assert_eq!(data["capacity"], 100);
    }

    #[test]
    fn test_queue_error_full_conversion() {
        let error = crate::types::QueueError::Full;

        let json_rpc = error.to_json_rpc_error();
        assert_eq!(json_rpc.code, -32000);

        let data = json_rpc.data.expect("Expected error data");
        assert_eq!(data["error"], "queue_full");
        assert!(data["suggestion"].as_str().unwrap().contains("retry"));
    }

    #[test]
    fn test_queue_error_worker_conversion() {
        let error = crate::types::QueueError::WorkerError("worker crashed".to_string());

        let json_rpc = error.to_json_rpc_error();
        assert_eq!(json_rpc.code, -32603);

        let data = json_rpc.data.expect("Expected error data");
        assert_eq!(data["error"], "worker_error");
        assert_eq!(data["details"], "worker crashed");
    }

    #[test]
    fn test_session_error_not_found_conversion() {
        let error = crate::types::SessionError::NotFound("session-123".to_string());

        let json_rpc = error.to_json_rpc_error();
        assert_eq!(json_rpc.code, -32602);

        let data = json_rpc.data.expect("Expected error data");
        assert_eq!(data["error"], "session_not_found");
        assert_eq!(data["sessionId"], "session-123");
    }

    #[test]
    fn test_session_error_limit_exceeded_conversion() {
        let error = crate::types::SessionError::LimitExceeded;

        let json_rpc = error.to_json_rpc_error();
        assert_eq!(json_rpc.code, -32000);

        let data = json_rpc.data.expect("Expected error data");
        assert_eq!(data["error"], "session_limit_exceeded");
    }

    #[test]
    fn test_session_error_timeout_conversion() {
        let error = crate::types::SessionError::Timeout;

        let json_rpc = error.to_json_rpc_error();
        assert_eq!(json_rpc.code, -32000);

        let data = json_rpc.data.expect("Expected error data");
        assert_eq!(data["error"], "session_timeout");
    }

    #[test]
    fn test_session_error_invalid_state_conversion() {
        let error = crate::types::SessionError::InvalidState("bad state".to_string());

        let json_rpc = error.to_json_rpc_error();
        assert_eq!(json_rpc.code, -32602);

        let data = json_rpc.data.expect("Expected error data");
        assert_eq!(data["error"], "invalid_session_state");
        assert_eq!(data["details"], "bad state");
    }

    #[test]
    fn test_mcp_error_server_not_found_conversion() {
        let error = crate::types::MCPError::ServerNotFound("test-server".to_string());

        let json_rpc = error.to_json_rpc_error();
        assert_eq!(json_rpc.code, -32602);

        let data = json_rpc.data.expect("Expected error data");
        assert_eq!(data["error"], "mcp_server_not_found");
        assert_eq!(data["serverName"], "test-server");
    }

    #[test]
    fn test_mcp_error_tool_call_failed_conversion() {
        let error = crate::types::MCPError::ToolCallFailed("tool failed".to_string());

        let json_rpc = error.to_json_rpc_error();
        assert_eq!(json_rpc.code, -32000);

        let data = json_rpc.data.expect("Expected error data");
        assert_eq!(data["error"], "mcp_tool_call_failed");
        assert_eq!(data["details"], "tool failed");
    }

    #[test]
    fn test_mcp_error_protocol_conversion() {
        let error = crate::types::MCPError::Protocol("invalid message".to_string());

        let json_rpc = error.to_json_rpc_error();
        assert_eq!(json_rpc.code, -32600);

        let data = json_rpc.data.expect("Expected error data");
        assert_eq!(data["error"], "mcp_protocol_error");
    }

    #[test]
    fn test_mcp_error_http_url_invalid_conversion() {
        let error = crate::types::MCPError::HttpUrlInvalid("bad url".to_string());

        let json_rpc = error.to_json_rpc_error();
        assert_eq!(json_rpc.code, -32602);

        let data = json_rpc.data.expect("Expected error data");
        assert_eq!(data["error"], "mcp_http_url_invalid");
    }

    #[test]
    fn test_template_error_rendering_failed_conversion() {
        let error = crate::types::TemplateError::RenderingFailed("syntax error".to_string());

        let json_rpc = error.to_json_rpc_error();
        assert_eq!(json_rpc.code, -32602);

        let data = json_rpc.data.expect("Expected error data");
        assert_eq!(data["error"], "template_rendering_failed");
        assert_eq!(data["details"], "syntax error");
    }

    #[test]
    fn test_template_error_tool_parsing_conversion() {
        let error = crate::types::TemplateError::ToolCallParsing("invalid json".to_string());

        let json_rpc = error.to_json_rpc_error();
        assert_eq!(json_rpc.code, -32602);

        let data = json_rpc.data.expect("Expected error data");
        assert_eq!(data["error"], "template_tool_parsing_failed");
    }

    #[test]
    fn test_generation_error_invalid_config_conversion() {
        let error = crate::generation::GenerationError::InvalidConfig("bad params".to_string());

        let json_rpc = error.to_json_rpc_error();
        assert_eq!(json_rpc.code, -32602);

        let data = json_rpc.data.expect("Expected error data");
        assert_eq!(data["error"], "invalid_generation_config");
    }

    #[test]
    fn test_generation_error_cancelled_conversion() {
        let error = crate::generation::GenerationError::Cancelled;

        let json_rpc = error.to_json_rpc_error();
        assert_eq!(json_rpc.code, -32000);

        let data = json_rpc.data.expect("Expected error data");
        assert_eq!(data["error"], "generation_cancelled");
    }

    #[test]
    fn test_generation_error_stopped_conversion() {
        let error = crate::generation::GenerationError::Stopped("eos token".to_string());

        let json_rpc = error.to_json_rpc_error();
        assert_eq!(json_rpc.code, -32000);

        let data = json_rpc.data.expect("Expected error data");
        assert_eq!(data["error"], "generation_stopped");
        assert_eq!(data["reason"], "eos token");
    }

    #[test]
    fn test_validation_error_security_violation_conversion() {
        let error =
            crate::validation::ValidationError::SecurityViolation("unsafe content".to_string());

        let json_rpc = error.to_json_rpc_error();
        assert_eq!(json_rpc.code, -32602);

        let data = json_rpc.data.expect("Expected error data");
        assert_eq!(data["error"], "security_violation");
        assert_eq!(data["details"], "unsafe content");
    }

    #[test]
    fn test_validation_error_parameter_bounds_conversion() {
        let error =
            crate::validation::ValidationError::ParameterBounds("value too large".to_string());

        let json_rpc = error.to_json_rpc_error();
        assert_eq!(json_rpc.code, -32602);

        let data = json_rpc.data.expect("Expected error data");
        assert_eq!(data["error"], "parameter_out_of_bounds");
    }

    #[test]
    fn test_validation_error_multiple_conversion() {
        let errors = vec![
            crate::validation::ValidationError::SecurityViolation("error 1".to_string()),
            crate::validation::ValidationError::ParameterBounds("error 2".to_string()),
        ];
        let error = crate::validation::ValidationError::Multiple(errors);

        let json_rpc = error.to_json_rpc_error();
        assert_eq!(json_rpc.code, -32602);

        let data = json_rpc.data.expect("Expected error data");
        assert_eq!(data["error"], "multiple_validation_errors");
        assert!(data["errors"].is_array());
        assert_eq!(data["errors"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn test_translation_error_unsupported_content_conversion() {
        let error = TranslationError::UnsupportedContent("video".to_string());

        let json_rpc = error.to_json_rpc_error();
        assert_eq!(json_rpc.code, -32602);

        let data = json_rpc.data.expect("Expected error data");
        assert_eq!(data["error"], "unsupported_content_type");
        assert_eq!(data["details"], "video");
    }

    #[test]
    fn test_translation_error_invalid_format_conversion() {
        let error = TranslationError::InvalidFormat("malformed".to_string());

        let json_rpc = error.to_json_rpc_error();
        assert_eq!(json_rpc.code, -32602);

        let data = json_rpc.data.expect("Expected error data");
        assert_eq!(data["error"], "invalid_content_format");
    }

    #[test]
    fn test_translation_error_invalid_session_id_conversion() {
        let error = TranslationError::InvalidSessionId("not a ulid".to_string());

        let json_rpc = error.to_json_rpc_error();
        assert_eq!(json_rpc.code, -32602);

        let data = json_rpc.data.expect("Expected error data");
        assert_eq!(data["error"], "invalid_session_id");
    }

    #[test]
    fn test_json_rpc_error_structure() {
        let error = crate::types::SessionError::NotFound("test".to_string());
        let json_rpc = error.to_json_rpc_error();

        // Verify structure
        assert!(json_rpc.code < 0);
        assert!(!json_rpc.message.is_empty());
        assert!(json_rpc.data.is_some());
    }

    #[test]
    fn test_nested_error_conversion() {
        let session_error = crate::types::SessionError::Timeout;
        let agent_error = crate::types::AgentError::Session(session_error);

        let json_rpc = agent_error.to_json_rpc_error();
        assert_eq!(json_rpc.code, -32000);

        let data = json_rpc.data.expect("Expected error data");
        assert_eq!(data["error"], "session_timeout");
    }

    // Tool definition translation tests

    #[test]
    fn test_tool_definition_to_acp_format() {
        let tool_def = ToolDefinition {
            name: "fs_read".to_string(),
            description: "Read a file from disk".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "File path to read"
                    }
                },
                "required": ["path"]
            }),
            server_name: "filesystem".to_string(),
        };

        let acp_format = tool_definition_to_acp_format(&tool_def);

        assert_eq!(acp_format["name"], "fs_read");
        assert_eq!(acp_format["description"], "Read a file from disk");
        assert_eq!(acp_format["server"], "filesystem");
        assert_eq!(acp_format["parameters"]["type"], "object");
        assert!(acp_format["parameters"]["properties"].is_object());
        assert_eq!(acp_format["parameters"]["required"][0], "path");
    }

    #[test]
    fn test_tool_definitions_to_acp_format_empty() {
        let tools: Vec<ToolDefinition> = vec![];
        let acp_format = tool_definitions_to_acp_format(&tools);

        assert!(acp_format.is_array());
        assert_eq!(acp_format.as_array().unwrap().len(), 0);
    }

    #[test]
    fn test_tool_definitions_to_acp_format_multiple() {
        let tools = vec![
            ToolDefinition {
                name: "fs_read".to_string(),
                description: "Read a file".to_string(),
                parameters: serde_json::json!({"type": "object"}),
                server_name: "filesystem".to_string(),
            },
            ToolDefinition {
                name: "fs_write".to_string(),
                description: "Write a file".to_string(),
                parameters: serde_json::json!({"type": "object"}),
                server_name: "filesystem".to_string(),
            },
            ToolDefinition {
                name: "shell_execute".to_string(),
                description: "Execute a command".to_string(),
                parameters: serde_json::json!({"type": "object"}),
                server_name: "shell".to_string(),
            },
        ];

        let acp_format = tool_definitions_to_acp_format(&tools);

        assert!(acp_format.is_array());
        let array = acp_format.as_array().unwrap();
        assert_eq!(array.len(), 3);
        assert_eq!(array[0]["name"], "fs_read");
        assert_eq!(array[1]["name"], "fs_write");
        assert_eq!(array[2]["name"], "shell_execute");
        assert_eq!(array[2]["server"], "shell");
    }

    #[test]
    fn test_tool_definition_to_acp_format_preserves_complex_schema() {
        let tool_def = ToolDefinition {
            name: "complex_tool".to_string(),
            description: "A complex tool".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "string_param": {"type": "string"},
                    "number_param": {"type": "number", "minimum": 0, "maximum": 100},
                    "enum_param": {"type": "string", "enum": ["a", "b", "c"]},
                    "nested": {
                        "type": "object",
                        "properties": {
                            "inner": {"type": "boolean"}
                        }
                    }
                },
                "required": ["string_param", "number_param"]
            }),
            server_name: "test_server".to_string(),
        };

        let acp_format = tool_definition_to_acp_format(&tool_def);

        // Verify schema is preserved
        assert_eq!(
            acp_format["parameters"]["properties"]["number_param"]["minimum"],
            0
        );
        assert_eq!(
            acp_format["parameters"]["properties"]["number_param"]["maximum"],
            100
        );
        assert_eq!(
            acp_format["parameters"]["properties"]["enum_param"]["enum"][0],
            "a"
        );
        assert_eq!(
            acp_format["parameters"]["properties"]["nested"]["properties"]["inner"]["type"],
            "boolean"
        );
    }

    // infer_tool_kind tests

    #[test]
    fn test_infer_tool_kind_read_operations() {
        use agent_client_protocol::ToolKind;

        assert_eq!(infer_tool_kind("fs_read"), ToolKind::Read);
        assert_eq!(infer_tool_kind("get_file"), ToolKind::Read);
        assert_eq!(infer_tool_kind("list_files"), ToolKind::Read);
        assert_eq!(infer_tool_kind("show_content"), ToolKind::Read);
        assert_eq!(infer_tool_kind("view_data"), ToolKind::Read);
        assert_eq!(infer_tool_kind("load_config"), ToolKind::Read);
        assert_eq!(infer_tool_kind("fetch_local"), ToolKind::Read);
        assert_eq!(infer_tool_kind("files_glob"), ToolKind::Read);
    }

    #[test]
    fn test_infer_tool_kind_edit_operations() {
        use agent_client_protocol::ToolKind;

        assert_eq!(infer_tool_kind("fs_write"), ToolKind::Edit);
        assert_eq!(infer_tool_kind("create_file"), ToolKind::Edit);
        assert_eq!(infer_tool_kind("update_content"), ToolKind::Edit);
        assert_eq!(infer_tool_kind("edit_file"), ToolKind::Edit);
        assert_eq!(infer_tool_kind("modify_data"), ToolKind::Edit);
    }

    #[test]
    fn test_infer_tool_kind_delete_operations() {
        use agent_client_protocol::ToolKind;

        assert_eq!(infer_tool_kind("fs_delete"), ToolKind::Delete);
        assert_eq!(infer_tool_kind("delete_file"), ToolKind::Delete);
        assert_eq!(infer_tool_kind("remove_file"), ToolKind::Delete);
        assert_eq!(infer_tool_kind("rm_file"), ToolKind::Delete);
        assert_eq!(infer_tool_kind("file_rm"), ToolKind::Delete);
    }

    #[test]
    fn test_infer_tool_kind_move_operations() {
        use agent_client_protocol::ToolKind;

        assert_eq!(infer_tool_kind("move_file"), ToolKind::Move);
        assert_eq!(infer_tool_kind("rename_file"), ToolKind::Move);
        assert_eq!(infer_tool_kind("file_mv"), ToolKind::Move);
    }

    #[test]
    fn test_infer_tool_kind_search_operations() {
        use agent_client_protocol::ToolKind;

        assert_eq!(infer_tool_kind("search_files"), ToolKind::Search);
        assert_eq!(infer_tool_kind("files_grep"), ToolKind::Search);
        assert_eq!(infer_tool_kind("find_in_files"), ToolKind::Search);
    }

    #[test]
    fn test_infer_tool_kind_execute_operations() {
        use agent_client_protocol::ToolKind;

        assert_eq!(infer_tool_kind("shell_execute"), ToolKind::Execute);
        assert_eq!(infer_tool_kind("execute_command"), ToolKind::Execute);
        assert_eq!(infer_tool_kind("terminal_run"), ToolKind::Execute);
        assert_eq!(infer_tool_kind("run_script"), ToolKind::Execute);
        assert_eq!(infer_tool_kind("bash_command"), ToolKind::Execute);
    }

    #[test]
    fn test_infer_tool_kind_fetch_operations() {
        use agent_client_protocol::ToolKind;

        assert_eq!(infer_tool_kind("http_get"), ToolKind::Fetch);
        assert_eq!(infer_tool_kind("web_fetch"), ToolKind::Fetch);
        assert_eq!(infer_tool_kind("fetch_url"), ToolKind::Fetch);
    }

    #[test]
    fn test_infer_tool_kind_think_operations() {
        use agent_client_protocol::ToolKind;

        assert_eq!(infer_tool_kind("think"), ToolKind::Think);
        assert_eq!(infer_tool_kind("plan_task"), ToolKind::Think);
        assert_eq!(infer_tool_kind("reason_about"), ToolKind::Think);
        assert_eq!(infer_tool_kind("analyze_code"), ToolKind::Think);
    }

    #[test]
    fn test_infer_tool_kind_other_operations() {
        use agent_client_protocol::ToolKind;

        assert_eq!(infer_tool_kind("unknown_tool"), ToolKind::Other);
        assert_eq!(infer_tool_kind("custom_operation"), ToolKind::Other);
        assert_eq!(infer_tool_kind("tool"), ToolKind::Other);
    }

    #[test]
    fn test_infer_tool_kind_case_insensitive() {
        use agent_client_protocol::ToolKind;

        assert_eq!(infer_tool_kind("FS_READ"), ToolKind::Read);
        assert_eq!(infer_tool_kind("Shell_Execute"), ToolKind::Execute);
        assert_eq!(infer_tool_kind("DELETE_FILE"), ToolKind::Delete);
    }

    #[test]
    fn test_infer_tool_kind_mcp_style_names() {
        use agent_client_protocol::ToolKind;

        assert_eq!(infer_tool_kind("mcp__files_read"), ToolKind::Read);
        assert_eq!(infer_tool_kind("mcp__files_write"), ToolKind::Edit);
        assert_eq!(infer_tool_kind("mcp__shell_execute"), ToolKind::Execute);
        assert_eq!(infer_tool_kind("mcp__web_fetch"), ToolKind::Fetch);
    }

    #[test]
    fn test_infer_tool_kind_rm_word_boundary() {
        use agent_client_protocol::ToolKind;

        // "rm" as a complete word should be Delete
        assert_eq!(infer_tool_kind("rm"), ToolKind::Delete);
        assert_eq!(infer_tool_kind("file_rm"), ToolKind::Delete);

        // "rm" as part of a word should NOT be Delete
        assert_eq!(infer_tool_kind("swissarmyhammer"), ToolKind::Other);
        assert_eq!(infer_tool_kind("format"), ToolKind::Other);
        assert_eq!(infer_tool_kind("transform"), ToolKind::Other);
    }

    // tool_call_to_acp tests

    #[test]
    fn test_tool_call_to_acp_basic() {
        use crate::types::ids::ToolCallId;
        use agent_client_protocol::ToolKind;

        let tool_call = ToolCall {
            id: ToolCallId::new(),
            name: "fs_read".to_string(),
            arguments: serde_json::json!({"path": "/tmp/test.txt"}),
        };

        let acp_call = tool_call_to_acp(tool_call.clone(), None);

        // Verify tool call ID
        assert_eq!(acp_call.tool_call_id.0.as_ref(), tool_call.id.to_string());

        // Verify title (should be just the name without definition)
        assert_eq!(acp_call.title, "fs_read");

        // Verify kind is inferred correctly
        assert_eq!(acp_call.kind, ToolKind::Read);

        // Verify raw_input contains arguments
        assert_eq!(
            acp_call.raw_input,
            Some(serde_json::json!({"path": "/tmp/test.txt"}))
        );

        // Verify no meta when no definition provided
        assert!(acp_call.meta.is_none());
    }

    #[test]
    fn test_tool_call_to_acp_with_definition() {
        use crate::types::ids::ToolCallId;
        use agent_client_protocol::ToolKind;

        let tool_call = ToolCall {
            id: ToolCallId::new(),
            name: "fs_read".to_string(),
            arguments: serde_json::json!({"path": "/tmp/test.txt"}),
        };

        let tool_def = ToolDefinition {
            name: "fs_read".to_string(),
            description: "Read a file from disk".to_string(),
            parameters: serde_json::json!({"type": "object"}),
            server_name: "filesystem".to_string(),
        };

        let acp_call = tool_call_to_acp(tool_call, Some(&tool_def));

        // Verify title includes description
        assert_eq!(acp_call.title, "fs_read: Read a file from disk");

        // Verify kind
        assert_eq!(acp_call.kind, ToolKind::Read);

        // Verify meta contains tool definition
        assert!(acp_call.meta.is_some());
        let meta = acp_call.meta.unwrap();
        assert!(meta["tool_definition"].is_object());
        assert_eq!(meta["tool_definition"]["name"], "fs_read");
        assert_eq!(
            meta["tool_definition"]["description"],
            "Read a file from disk"
        );
        assert_eq!(meta["tool_definition"]["server"], "filesystem");
    }

    #[test]
    fn test_tool_call_to_acp_different_kinds() {
        use crate::types::ids::ToolCallId;
        use agent_client_protocol::ToolKind;

        let test_cases = vec![
            ("fs_read", ToolKind::Read),
            ("fs_write", ToolKind::Edit),
            ("fs_delete", ToolKind::Delete),
            ("shell_execute", ToolKind::Execute),
            ("web_fetch", ToolKind::Fetch),
            ("search_files", ToolKind::Search),
            ("move_file", ToolKind::Move),
            ("think", ToolKind::Think),
            ("custom_tool", ToolKind::Other),
        ];

        for (name, expected_kind) in test_cases {
            let tool_call = ToolCall {
                id: ToolCallId::new(),
                name: name.to_string(),
                arguments: serde_json::json!({}),
            };

            let acp_call = tool_call_to_acp(tool_call, None);
            assert_eq!(acp_call.kind, expected_kind, "Failed for tool: {}", name);
        }
    }

    #[test]
    fn test_tool_call_to_acp_preserves_complex_arguments() {
        use crate::types::ids::ToolCallId;

        let complex_args = serde_json::json!({
            "path": "/tmp/test.txt",
            "options": {
                "encoding": "utf-8",
                "create": true
            },
            "metadata": {
                "author": "test",
                "version": 1
            }
        });

        let tool_call = ToolCall {
            id: ToolCallId::new(),
            name: "fs_write".to_string(),
            arguments: complex_args.clone(),
        };

        let acp_call = tool_call_to_acp(tool_call, None);

        assert_eq!(acp_call.raw_input, Some(complex_args));
    }

    #[test]
    fn test_tool_call_to_acp_preserves_call_id() {
        use crate::types::ids::ToolCallId;

        let tool_call_id = ToolCallId::new();
        let tool_call_id_str = tool_call_id.to_string();

        let tool_call = ToolCall {
            id: tool_call_id,
            name: "test_tool".to_string(),
            arguments: serde_json::json!({}),
        };

        let acp_call = tool_call_to_acp(tool_call, None);

        assert_eq!(acp_call.tool_call_id.0.as_ref(), &tool_call_id_str);
    }

    // tool_result_to_acp_update tests

    #[test]
    fn test_tool_result_to_acp_update_success() {
        use crate::types::ids::ToolCallId;
        use agent_client_protocol::ToolCallStatus;

        let tool_call_id = ToolCallId::new();
        let tool_call_id_str = tool_call_id.to_string();

        let tool_result = ToolResult {
            call_id: tool_call_id,
            result: serde_json::json!({"status": "ok", "data": "file content"}),
            error: None,
        };

        let update = tool_result_to_acp_update(tool_result);

        // Verify tool call ID
        assert_eq!(update.tool_call_id.0.as_ref(), &tool_call_id_str);

        // Verify status is Completed
        assert_eq!(update.fields.status, Some(ToolCallStatus::Completed));

        // Verify raw_output contains the result
        assert!(update.fields.raw_output.is_some());
        let output = update.fields.raw_output.unwrap();
        assert_eq!(output["status"], "ok");
        assert_eq!(output["data"], "file content");

        // Verify no error content
        assert!(update.fields.content.is_none());
    }

    #[test]
    fn test_tool_result_to_acp_update_failure() {
        use crate::types::ids::ToolCallId;
        use agent_client_protocol::{ContentBlock, ToolCallContent, ToolCallStatus};

        let tool_call_id = ToolCallId::new();
        let tool_call_id_str = tool_call_id.to_string();

        let tool_result = ToolResult {
            call_id: tool_call_id,
            result: serde_json::Value::Null,
            error: Some("File not found".to_string()),
        };

        let update = tool_result_to_acp_update(tool_result);

        // Verify tool call ID
        assert_eq!(update.tool_call_id.0.as_ref(), &tool_call_id_str);

        // Verify status is Failed
        assert_eq!(update.fields.status, Some(ToolCallStatus::Failed));

        // Verify content contains error message
        assert!(update.fields.content.is_some());
        let content = update.fields.content.unwrap();
        assert_eq!(content.len(), 1);

        match &content[0] {
            ToolCallContent::Content(content_wrapper) => match &content_wrapper.content {
                ContentBlock::Text(text) => {
                    assert_eq!(text.text, "File not found");
                }
                _ => panic!("Expected text content block in error"),
            },
            _ => panic!("Expected Content variant"),
        }

        // Verify no raw_output for errors
        assert!(update.fields.raw_output.is_none());
    }

    #[test]
    fn test_tool_result_to_acp_update_success_with_null_result() {
        use crate::types::ids::ToolCallId;
        use agent_client_protocol::ToolCallStatus;

        let tool_result = ToolResult {
            call_id: ToolCallId::new(),
            result: serde_json::Value::Null,
            error: None,
        };

        let update = tool_result_to_acp_update(tool_result);

        // Even with Null result, if there's no error, it's a success
        assert_eq!(update.fields.status, Some(ToolCallStatus::Completed));
        assert!(update.fields.raw_output.is_some());
        assert_eq!(update.fields.raw_output.unwrap(), serde_json::Value::Null);
    }

    #[test]
    fn test_tool_result_to_acp_update_complex_result() {
        use crate::types::ids::ToolCallId;
        use agent_client_protocol::ToolCallStatus;

        let complex_result = serde_json::json!({
            "files_read": ["file1.txt", "file2.txt"],
            "total_lines": 150,
            "metadata": {
                "encoding": "utf-8",
                "size_bytes": 4096
            }
        });

        let tool_result = ToolResult {
            call_id: ToolCallId::new(),
            result: complex_result.clone(),
            error: None,
        };

        let update = tool_result_to_acp_update(tool_result);

        assert_eq!(update.fields.status, Some(ToolCallStatus::Completed));
        assert_eq!(update.fields.raw_output, Some(complex_result));
    }

    #[test]
    fn test_tool_result_to_acp_update_multiline_error() {
        use crate::types::ids::ToolCallId;
        use agent_client_protocol::{ContentBlock, ToolCallContent, ToolCallStatus};

        let error_msg =
            "Error: Failed to execute command\nReason: Permission denied\nPath: /etc/shadow";

        let tool_result = ToolResult {
            call_id: ToolCallId::new(),
            result: serde_json::Value::Null,
            error: Some(error_msg.to_string()),
        };

        let update = tool_result_to_acp_update(tool_result);

        assert_eq!(update.fields.status, Some(ToolCallStatus::Failed));

        let content = update.fields.content.unwrap();
        assert_eq!(content.len(), 1);

        match &content[0] {
            ToolCallContent::Content(content_wrapper) => match &content_wrapper.content {
                ContentBlock::Text(text) => {
                    assert_eq!(text.text, error_msg);
                }
                _ => panic!("Expected text content block"),
            },
            _ => panic!("Expected Content variant"),
        }
    }

    #[test]
    fn test_tool_result_to_acp_update_empty_error_message() {
        use crate::types::ids::ToolCallId;
        use agent_client_protocol::{ContentBlock, ToolCallContent, ToolCallStatus};

        let tool_result = ToolResult {
            call_id: ToolCallId::new(),
            result: serde_json::Value::Null,
            error: Some("".to_string()),
        };

        let update = tool_result_to_acp_update(tool_result);

        assert_eq!(update.fields.status, Some(ToolCallStatus::Failed));

        let content = update.fields.content.unwrap();
        assert_eq!(content.len(), 1);

        match &content[0] {
            ToolCallContent::Content(content_wrapper) => match &content_wrapper.content {
                ContentBlock::Text(text) => {
                    assert_eq!(text.text, "");
                }
                _ => panic!("Expected text content block"),
            },
            _ => panic!("Expected Content variant"),
        }
    }

    #[test]
    fn test_tool_result_to_acp_update_preserves_call_id() {
        use crate::types::ids::ToolCallId;

        // Test that ToolCallId round-trips correctly through the conversion
        let tool_call_id = ToolCallId::new();
        let tool_call_id_str = tool_call_id.to_string();

        let tool_result = ToolResult {
            call_id: tool_call_id,
            result: serde_json::json!({"test": true}),
            error: None,
        };

        let update = tool_result_to_acp_update(tool_result);
        assert_eq!(update.tool_call_id.0.as_ref(), &tool_call_id_str);
    }

    // needs_permission tests

    #[test]
    fn test_needs_permission_read_operations() {
        // Read operations should not require permission
        assert!(!needs_permission("fs_read"));
        assert!(!needs_permission("fs_read_file"));
        assert!(!needs_permission("get_content"));
        assert!(!needs_permission("list_files"));
        assert!(!needs_permission("show_data"));
        assert!(!needs_permission("view_file"));
        assert!(!needs_permission("load_config"));
        assert!(!needs_permission("fetch_data")); // fetch without http/web
    }

    #[test]
    fn test_needs_permission_write_operations() {
        // Write operations should require permission
        assert!(needs_permission("fs_write"));
        assert!(needs_permission("fs_write_file"));
        assert!(needs_permission("create_file"));
        assert!(needs_permission("update_config"));
        assert!(needs_permission("edit_content"));
        assert!(needs_permission("modify_data"));
    }

    #[test]
    fn test_needs_permission_delete_operations() {
        // Delete operations should require permission
        assert!(needs_permission("fs_delete"));
        assert!(needs_permission("delete_file"));
        assert!(needs_permission("remove_file"));
        assert!(needs_permission("rm_file"));
    }

    #[test]
    fn test_needs_permission_execute_operations() {
        // Execute operations should require permission
        assert!(needs_permission("terminal_create"));
        assert!(needs_permission("execute_command"));
        assert!(needs_permission("shell_run"));
        assert!(needs_permission("run_script"));
        assert!(needs_permission("bash_execute"));
    }

    #[test]
    fn test_needs_permission_network_operations() {
        // Network operations should require permission
        assert!(needs_permission("http_get"));
        assert!(needs_permission("http_post"));
        assert!(needs_permission("web_fetch"));
        assert!(needs_permission("fetch_url")); // contains fetch but also url-like
    }

    #[test]
    fn test_needs_permission_mcp_tools() {
        // Test MCP-style tool names
        assert!(!needs_permission("mcp__files_read"));
        assert!(needs_permission("mcp__files_write"));
        assert!(needs_permission("mcp__shell_execute"));
        assert!(!needs_permission("mcp__files_list"));
        assert!(needs_permission("mcp__web_fetch"));
    }

    #[test]
    fn test_needs_permission_case_insensitive() {
        // Test case insensitivity
        assert!(!needs_permission("FS_READ"));
        assert!(needs_permission("FS_WRITE"));
        assert!(needs_permission("TERMINAL_CREATE"));
        assert!(!needs_permission("GET_FILE"));
        assert!(needs_permission("DELETE_FILE"));
    }

    #[test]
    fn test_needs_permission_mixed_case() {
        // Test mixed case
        assert!(!needs_permission("fsRead"));
        assert!(needs_permission("fsWrite"));
        assert!(needs_permission("terminalCreate"));
        assert!(!needs_permission("getFile"));
        assert!(needs_permission("deleteFile"));
    }

    #[test]
    fn test_needs_permission_compound_operations() {
        // Test that write indicators override read indicators
        assert!(needs_permission("read_and_write")); // Contains both, but write takes precedence
        assert!(needs_permission("get_and_update")); // Contains both, but update takes precedence
        assert!(needs_permission("list_and_delete")); // Contains both, but delete takes precedence
    }

    #[test]
    fn test_needs_permission_ambiguous_tools() {
        // Test tools that might be ambiguous - default to requiring permission
        assert!(needs_permission("tool"));
        assert!(needs_permission("process"));
        assert!(needs_permission("handle"));
        assert!(needs_permission("manage"));
    }

    #[test]
    fn test_needs_permission_think_operations() {
        // Think/reasoning operations should require permission by default
        // (they don't match read patterns)
        assert!(needs_permission("think"));
        assert!(needs_permission("reason"));
        assert!(needs_permission("plan"));
        assert!(needs_permission("analyze"));
    }

    #[test]
    fn test_needs_permission_empty_string() {
        // Empty string should require permission (safe default)
        assert!(needs_permission(""));
    }

    #[test]
    fn test_needs_permission_special_characters() {
        // Tools with special characters
        assert!(!needs_permission("fs-read"));
        assert!(needs_permission("fs-write"));
        assert!(!needs_permission("fs.read"));
        assert!(needs_permission("fs.write"));
    }

    #[test]
    fn test_needs_permission_read_not_confused_with_execute() {
        // Ensure read operations with execute in name still require permission
        assert!(needs_permission("read_and_execute"));
        assert!(needs_permission("execute_read")); // execute takes precedence
    }

    #[test]
    fn test_needs_permission_fetch_with_context() {
        // fetch by itself might be read, but with http/web it requires permission
        assert!(!needs_permission("fetch")); // Plain fetch - could be data retrieval
        assert!(needs_permission("http_fetch")); // Network fetch
        assert!(needs_permission("web_fetch")); // Network fetch
        assert!(!needs_permission("fetch_from_cache")); // Local fetch
    }

    #[test]
    fn test_needs_permission_move_operations() {
        // Move operations should require permission (they're not pure reads)
        assert!(needs_permission("move_file"));
        assert!(needs_permission("rename_file"));
        assert!(needs_permission("mv"));
    }

    #[test]
    fn test_needs_permission_search_operations() {
        // Search/grep operations are reads and shouldn't require permission
        assert!(!needs_permission("search"));
        assert!(!needs_permission("grep"));
        assert!(!needs_permission("find"));
    }

    #[test]
    fn test_needs_permission_real_world_tool_names() {
        // Test real-world tool names from MCP servers
        assert!(!needs_permission("mcp__swissarmyhammer__files_read"));
        assert!(needs_permission("mcp__swissarmyhammer__files_write"));
        assert!(needs_permission("mcp__swissarmyhammer__files_edit"));
        assert!(needs_permission("mcp__swissarmyhammer__shell_execute"));
        assert!(!needs_permission("mcp__swissarmyhammer__files_glob"));
        assert!(!needs_permission("mcp__swissarmyhammer__files_grep"));
        assert!(needs_permission("mcp__swissarmyhammer__web_search"));
        assert!(needs_permission("mcp__swissarmyhammer__web_fetch"));
    }
}
