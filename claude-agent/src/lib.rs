//! Claude Agent Library
//!
//! A Rust library that implements an Agent Client Protocol (ACP) server,
//! wrapping Claude Code functionality to enable any ACP-compatible client
//! to interact with Claude Code.

pub mod acp_error_conversion;
pub mod agent;
pub mod base64_processor;
pub mod base64_validation;
pub mod capability_validation;
pub mod claude;
pub mod claude_backend;
pub mod claude_process;
pub mod config;
pub mod constants;
pub mod content_block_processor;
pub mod content_capability_validator;
pub mod content_security_validator;
pub mod conversation_manager;
pub mod editor_state;
pub mod json_rpc_codes;
pub mod mime_type_validator;

#[cfg(test)]
mod content_security_integration_tests;
pub mod error;
pub mod mcp;
pub mod mcp_error_handling;
pub mod path_validator;
pub mod permission_storage;
pub mod permissions;
pub mod plan;
pub mod protocol_translator;
#[cfg(test)]
// mod permission_interaction_tests; // Disabled: tests MockPromptHandler which was deleted
pub mod request_validation;
pub mod server;
pub mod session;
pub mod session_errors;
pub mod session_loading;
pub mod session_validation;
pub mod size_validator;
pub mod terminal_manager;
#[cfg(test)]
mod tool_call_lifecycle_tests;
pub mod tool_classification;
pub mod tool_types;
pub mod tools;
pub mod url_validation;

pub use agent::{ClaudeAgent, RawMessageManager};
pub use config::AgentConfig;
pub use error::{AgentError, Result};
pub use plan::{
    todowrite_to_acp_plan, todowrite_to_agent_plan, AgentPlan, PlanEntry, PlanEntryStatus, Priority,
};
pub use server::ClaudeAgentServer;
pub use tools::{ToolCallHandler, ToolCallResult, ToolPermissions};
