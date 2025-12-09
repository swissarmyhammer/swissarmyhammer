//! Error types for the agent framework.
//!
//! This module contains all error types used throughout the llama-agent system,
//! with comprehensive error categorization and user-friendly messages.

use llama_common::error::{ErrorCategory, LlamaError};
use llama_loader::ModelError;
use std::time::Duration;
use thiserror::Error;

/// Top-level agent errors that can occur during agent operations.
#[derive(Debug, Error)]
pub enum AgentError {
    #[error("Model error: {0}\n💡 Check model file exists, is valid GGUF format, and sufficient memory is available")]
    Model(#[from] ModelError),

    #[error("Request processing error: {0}\n💡 Try reducing concurrent requests, increasing queue size, or adding more system resources")]
    Queue(#[from] QueueError),

    #[error(
        "Session error: {0}\n💡 Verify session ID is valid and session limits are not exceeded"
    )]
    Session(#[from] SessionError),

    #[error("MCP server error: {0}\n💡 Ensure MCP server is running, accessible, and check network connectivity")]
    MCP(#[from] MCPError),

    #[error("Template processing error: {0}\n💡 Check message format and tool definitions are properly structured")]
    Template(#[from] TemplateError),

    #[error("Request timeout: processing took longer than {timeout:?}\n💡 Increase timeout settings, reduce max_tokens, or check system performance")]
    Timeout { timeout: Duration },

    #[error("Queue overloaded: {capacity} requests queued (max capacity)\n💡 Wait and retry, or increase max_queue_size configuration")]
    QueueFull { capacity: usize },
}

/// Errors related to request queue operations.
#[derive(Debug, Clone, Error)]
pub enum QueueError {
    #[error("Queue is full")]
    Full,

    #[error("Worker thread error: {0}")]
    WorkerError(String),
}

/// Errors related to session management.
#[derive(Debug, Error)]
pub enum SessionError {
    #[error("Session not found: {0}")]
    NotFound(String),

    #[error("Session limit exceeded")]
    LimitExceeded,

    #[error("Session timeout")]
    Timeout,

    #[error("Invalid session state: {0}")]
    InvalidState(String),
}

/// Errors related to MCP (Model Context Protocol) server operations.
#[derive(Debug, Error)]
pub enum MCPError {
    #[error("MCP server not found: {0}")]
    ServerNotFound(String),

    #[error("Tool call failed: {0}")]
    ToolCallFailed(String),

    #[error("Connection error: {0}")]
    Connection(String),

    #[error("Protocol error: {0}")]
    Protocol(String),

    #[error("HTTP URL validation failed: {0}")]
    HttpUrlInvalid(String),

    #[error("HTTP timeout error: {0}")]
    HttpTimeout(String),

    #[error("HTTP connection failed: {0}")]
    HttpConnection(String),

    #[error("Operation timed out: {0}")]
    Timeout(String),
}

/// Errors related to chat template processing.
#[derive(Debug, Error)]
pub enum TemplateError {
    #[error("Template rendering failed: {0}")]
    RenderingFailed(String),

    #[error("Tool call parsing failed: {0}")]
    ToolCallParsing(String),

    #[error("Invalid template: {0}")]
    Invalid(String),
}

// LlamaError trait implementations for consistent error handling

impl LlamaError for AgentError {
    fn category(&self) -> ErrorCategory {
        match self {
            AgentError::Model(_) => ErrorCategory::System,
            AgentError::Queue(_) => ErrorCategory::System,
            AgentError::Session(session_error) => session_error.category(),
            AgentError::MCP(_) => ErrorCategory::External,
            AgentError::Template(_) => ErrorCategory::User,
            AgentError::Timeout { .. } => ErrorCategory::System,
            AgentError::QueueFull { .. } => ErrorCategory::System,
        }
    }

    fn error_code(&self) -> &'static str {
        match self {
            AgentError::Model(_) => "AGENT_MODEL",
            AgentError::Queue(_) => "AGENT_QUEUE",
            AgentError::Session(_) => "AGENT_SESSION",
            AgentError::MCP(_) => "AGENT_MCP",
            AgentError::Template(_) => "AGENT_TEMPLATE",
            AgentError::Timeout { .. } => "AGENT_TIMEOUT",
            AgentError::QueueFull { .. } => "AGENT_QUEUE_FULL",
        }
    }

    fn user_friendly_message(&self) -> String {
        format!("{}", self)
    }
}

impl LlamaError for QueueError {
    fn category(&self) -> ErrorCategory {
        match self {
            QueueError::Full => ErrorCategory::System,
            QueueError::WorkerError(_) => ErrorCategory::System,
        }
    }

    fn error_code(&self) -> &'static str {
        match self {
            QueueError::Full => "QUEUE_FULL",
            QueueError::WorkerError(_) => "QUEUE_WORKER",
        }
    }

    fn user_friendly_message(&self) -> String {
        match self {
            QueueError::Full => {
                "Queue is full\n💡 Wait a moment and retry, or increase the max_queue_size configuration".to_string()
            }
            QueueError::WorkerError(msg) => {
                format!("Worker thread error: {}\n💡 Check system resources and model availability", msg)
            }
        }
    }
}

impl LlamaError for SessionError {
    fn category(&self) -> ErrorCategory {
        match self {
            SessionError::NotFound(_) => ErrorCategory::User,
            SessionError::LimitExceeded => ErrorCategory::System,
            SessionError::Timeout => ErrorCategory::System,
            SessionError::InvalidState(_) => ErrorCategory::User,
        }
    }

    fn error_code(&self) -> &'static str {
        match self {
            SessionError::NotFound(_) => "SESSION_NOT_FOUND",
            SessionError::LimitExceeded => "SESSION_LIMIT",
            SessionError::Timeout => "SESSION_TIMEOUT",
            SessionError::InvalidState(_) => "SESSION_INVALID_STATE",
        }
    }

    fn is_retriable(&self) -> bool {
        match self {
            SessionError::NotFound(_) => false,
            SessionError::LimitExceeded => false, // Not retriable without configuration change
            SessionError::Timeout => true,
            SessionError::InvalidState(_) => false,
        }
    }

    fn user_friendly_message(&self) -> String {
        match self {
            SessionError::NotFound(id) => {
                format!("Session not found: {}\n💡 Verify the session ID is correct and the session hasn't expired", id)
            }
            SessionError::LimitExceeded => {
                "Session limit exceeded\n💡 Close unused sessions or increase session limits in configuration".to_string()
            }
            SessionError::Timeout => {
                "Session timeout\n💡 Increase session timeout or complete operations more quickly".to_string()
            }
            SessionError::InvalidState(msg) => {
                format!("Invalid session state: {}\n💡 Check that the session is in the correct state for this operation", msg)
            }
        }
    }
}

impl LlamaError for MCPError {
    fn category(&self) -> ErrorCategory {
        match self {
            MCPError::ServerNotFound(_) => ErrorCategory::User,
            MCPError::ToolCallFailed(_) => ErrorCategory::External,
            MCPError::Connection(_) => ErrorCategory::External,
            MCPError::Protocol(_) => ErrorCategory::External,
            MCPError::HttpUrlInvalid(_) => ErrorCategory::User,
            MCPError::HttpTimeout(_) => ErrorCategory::External,
            MCPError::HttpConnection(_) => ErrorCategory::External,
            MCPError::Timeout(_) => ErrorCategory::External,
        }
    }

    fn error_code(&self) -> &'static str {
        match self {
            MCPError::ServerNotFound(_) => "MCP_SERVER_NOT_FOUND",
            MCPError::ToolCallFailed(_) => "MCP_TOOL_CALL_FAILED",
            MCPError::Connection(_) => "MCP_CONNECTION",
            MCPError::Protocol(_) => "MCP_PROTOCOL",
            MCPError::HttpUrlInvalid(_) => "MCP_HTTP_URL_INVALID",
            MCPError::HttpTimeout(_) => "MCP_HTTP_TIMEOUT",
            MCPError::HttpConnection(_) => "MCP_HTTP_CONNECTION",
            MCPError::Timeout(_) => "MCP_TIMEOUT",
        }
    }

    fn user_friendly_message(&self) -> String {
        match self {
            MCPError::ServerNotFound(name) => {
                format!("MCP server not found: {}\n💡 Check server configuration and ensure the server name is correct", name)
            }
            MCPError::ToolCallFailed(msg) => {
                format!(
                    "Tool call failed: {}\n💡 Verify tool parameters and server availability",
                    msg
                )
            }
            MCPError::Connection(msg) => {
                format!(
                    "Connection error: {}\n💡 Check network connectivity and server status",
                    msg
                )
            }
            MCPError::Protocol(msg) => {
                format!("Protocol error: {}\n💡 Ensure MCP server is compatible and responding correctly", msg)
            }
            MCPError::HttpUrlInvalid(msg) => {
                format!(
                    "HTTP URL validation failed: {}\n💡 Check the URL format and protocol",
                    msg
                )
            }
            MCPError::HttpTimeout(msg) => {
                format!(
                    "HTTP timeout error: {}\n💡 Increase timeout settings or check network latency",
                    msg
                )
            }
            MCPError::HttpConnection(msg) => {
                format!(
                    "HTTP connection failed: {}\n💡 Verify server URL and network connectivity",
                    msg
                )
            }
            MCPError::Timeout(msg) => {
                format!("Operation timed out: {}\n💡 Increase timeout settings or check server responsiveness", msg)
            }
        }
    }
}

impl LlamaError for TemplateError {
    fn category(&self) -> ErrorCategory {
        match self {
            TemplateError::RenderingFailed(_) => ErrorCategory::User,
            TemplateError::ToolCallParsing(_) => ErrorCategory::User,
            TemplateError::Invalid(_) => ErrorCategory::User,
        }
    }

    fn error_code(&self) -> &'static str {
        match self {
            TemplateError::RenderingFailed(_) => "TEMPLATE_RENDERING",
            TemplateError::ToolCallParsing(_) => "TEMPLATE_TOOL_PARSING",
            TemplateError::Invalid(_) => "TEMPLATE_INVALID",
        }
    }

    fn user_friendly_message(&self) -> String {
        match self {
            TemplateError::RenderingFailed(msg) => {
                format!("Template rendering failed: {}\n💡 Check template syntax and variable availability", msg)
            }
            TemplateError::ToolCallParsing(msg) => {
                format!(
                    "Tool call parsing failed: {}\n💡 Verify tool call format and JSON structure",
                    msg
                )
            }
            TemplateError::Invalid(msg) => {
                format!(
                    "Invalid template: {}\n💡 Check template structure and required parameters",
                    msg
                )
            }
        }
    }
}
