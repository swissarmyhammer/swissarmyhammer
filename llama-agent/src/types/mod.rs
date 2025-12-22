//! Type definitions for the llama-agent framework.
//!
//! This module contains all the core types used throughout the agent system,
//! organized into logical submodules for better maintainability.

// Module declarations
pub mod configs;
pub mod errors;
pub mod generation;
pub mod ids;
pub mod mcp;
pub mod messages;
pub mod prompts;
pub mod sessions;
pub mod streaming;
pub mod tools;

// Re-export model types from llama-loader
pub use llama_loader::{ModelConfig, ModelError, ModelSource, RetryConfig};

// Re-export compaction types from session module (will be re-exported via lib.rs)
pub use crate::session::{CompactionResult, CompactionSummary};

// Re-export rmcp transport configuration for direct integration
pub use rmcp::transport::StreamableHttpServerConfig;

// Compaction configuration constants
pub const MIN_COMPRESSION_RATIO: f32 = 0.8; // 20% reduction minimum

// Re-export ID types
pub use ids::{PromptId, SessionId, ToolCallId};

// Re-export error types
pub use errors::{AgentError, MCPError, QueueError, SessionError, TemplateError};

// Re-export message types
pub use messages::{Message, MessageRole, SimpleTokenCounter, TokenCounter, TokenUsage};

// Re-export MCP configuration types
pub use mcp::{HttpServerConfig, MCPServerConfig, ProcessServerConfig};

// Re-export generation types
pub use generation::{FinishReason, GenerationRequest, GenerationResponse, StoppingConfig};

// Re-export tool types
pub use tools::{
    AccessType, ConflictType, ParallelConfig, ParallelExecutionConfig, ParameterReference,
    ReferenceType, ResourceAccess, ResourceType, ToolCall, ToolConflict, ToolDefinition,
    ToolResult,
};

// Re-export configuration types
pub use configs::{AgentConfig, HealthStatus, LlamaAgentMode, QueueConfig, SessionConfig};

// Re-export streaming types
pub use streaming::{AgentAPI, StreamChunk};

// Re-export prompt types
pub use prompts::{
    GetPromptResult, PromptArgument, PromptContent, PromptDefinition, PromptMessage,
    PromptResource, PromptRole,
};

// Re-export session types
pub use sessions::{
    CompactionConfig, CompactionMetadata, CompactionPrompt, ContextState, Session, SessionBackup,
};
