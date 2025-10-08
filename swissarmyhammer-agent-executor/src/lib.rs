//! Agent executor implementations for SwissArmyHammer
//!
//! This crate provides agent executor implementations that can be used by
//! both the workflow and rules crates without creating circular dependencies.

pub mod claude;
pub mod context;
pub mod error;
pub mod executor;
pub mod llama;
pub mod response;

// Re-export commonly used types
pub use claude::ClaudeCodeExecutor;
pub use context::AgentExecutionContext;
pub use error::{ActionError, ActionResult};
pub use executor::{AgentExecutor, AgentExecutorFactory};
pub use response::{AgentResponse, AgentResponseType};

// Re-export llama types for convenience
pub use llama::{LlamaAgentExecutor, LlamaAgentExecutorWrapper, McpServerHandle};
