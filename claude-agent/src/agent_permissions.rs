//! Permission-related types for Agent Client Protocol

use agent_client_protocol::SessionId;

/// ACP tool call information for permission requests
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ToolCallUpdate {
    /// Unique identifier for the tool call
    #[serde(rename = "toolCallId")]
    pub tool_call_id: String,
}

/// ACP-compliant permission request
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PermissionRequest {
    /// Session identifier for the permission request
    #[serde(rename = "sessionId")]
    pub session_id: SessionId,
    /// Tool call information
    #[serde(rename = "toolCall")]
    pub tool_call: ToolCallUpdate,
    /// Available permission options for the user
    pub options: Vec<crate::tools::PermissionOption>,
}

/// ACP-compliant permission response
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PermissionResponse {
    /// The outcome of the permission request
    pub outcome: crate::tools::PermissionOutcome,
}
