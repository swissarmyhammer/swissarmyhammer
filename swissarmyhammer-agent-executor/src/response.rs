//! Agent response types for executor results
//!
//! This module defines the response structures returned by agent executors
//! after processing prompts and generating AI responses.
//!
//! # Core Types
//!
//! - [`AgentResponse`]: The main response structure containing content, metadata, and status
//! - [`AgentResponseType`]: Enum indicating whether the response is successful, partial, or an error
//!
//! # Response Types
//!
//! - **Success**: Complete successful response from the agent
//! - **Partial**: Incomplete response due to streaming, timeout, or early termination
//! - **Error**: Failed execution with error details in the content
//!
//! # Usage
//!
//! ```rust
//! use swissarmyhammer_agent_executor::AgentResponse;
//!
//! // Create a successful response
//! let response = AgentResponse::success("Hello, world!".to_string());
//! assert!(response.is_success());
//!
//! // Create an error response
//! let error = AgentResponse::error("Model unavailable".to_string());
//! assert!(error.is_error());
//! ```

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
