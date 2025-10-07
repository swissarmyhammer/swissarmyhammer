//! Agent response types

/// Response type from agent execution
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AgentResponse {
    /// The primary response content from the agent
    pub content: String,
    /// Optional metadata about the response
    pub metadata: Option<serde_json::Value>,
    /// Response status/type for different kinds of responses
    pub response_type: AgentResponseType,
}

/// Type of agent response
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum AgentResponseType {
    /// Standard successful text response
    Success,
    /// Partial response (streaming, timeout, etc.)
    Partial,
    /// Error response with error details
    Error,
}

impl AgentResponse {
    /// Create a successful response
    pub fn success(content: String) -> Self {
        Self {
            content,
            metadata: None,
            response_type: AgentResponseType::Success,
        }
    }

    /// Create a successful response with metadata
    pub fn success_with_metadata(content: String, metadata: serde_json::Value) -> Self {
        Self {
            content,
            metadata: Some(metadata),
            response_type: AgentResponseType::Success,
        }
    }

    /// Create an error response
    pub fn error(content: String) -> Self {
        Self {
            content,
            metadata: None,
            response_type: AgentResponseType::Error,
        }
    }

    /// Create a partial response
    pub fn partial(content: String) -> Self {
        Self {
            content,
            metadata: None,
            response_type: AgentResponseType::Partial,
        }
    }

    /// Check if this is a successful response
    pub fn is_success(&self) -> bool {
        matches!(self.response_type, AgentResponseType::Success)
    }

    /// Check if this is an error response
    pub fn is_error(&self) -> bool {
        matches!(self.response_type, AgentResponseType::Error)
    }
}
