//! Coverage tests for claude-agent crate, split by area into one module each.
//!
//! Tests targeting previously uncovered code in:
//! - mcp.rs (MCP integration)
//! - terminal_manager.rs (terminal management)
//! - tools.rs (tool execution)
//! - agent.rs (core agent loop)
//! - mcp_error_handling.rs (MCP error handling)
//! - config.rs (configuration types)
//! - session.rs (session management)
//! - plan.rs (plan types)
//! - conversation_manager.rs (conversation types)
//! - editor_state.rs (editor state management)
//! - error.rs (error types)
//! - tool_types.rs (tool type conversions)
//! - tool_classification.rs (tool classification)

mod agent_misc;
mod config;
mod errors;
mod mcp;
mod plan;
mod session;
mod terminal;
mod tool_classification;
mod tool_types;

/// JSON-RPC code for a request that is not valid.
const JSON_RPC_INVALID_REQUEST: i32 = -32600;

/// JSON-RPC code for an internal fault.
const JSON_RPC_INTERNAL_ERROR: i32 = -32603;

/// JSON-RPC code for a message that is not valid JSON.
const JSON_RPC_PARSE_ERROR: i32 = -32700;

/// JSON-RPC code in the implementation-defined server error range.
const JSON_RPC_SERVER_ERROR: i32 = -32000;
