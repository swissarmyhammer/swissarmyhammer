//! CEL-based state management tools for MCP
//!
//! This module provides MCP tool interfaces to the process-global CEL
//! (Common Expression Language) state management from swissarmyhammer-cel.
//!
//! # Tools
//!
//! - `cel_set`: Evaluate CEL expression and store result as named variable
//! - `cel_get`: Evaluate CEL expression in current context and return result

pub mod cel_get;
pub mod cel_set;

// Re-export the CEL state and utilities from the swissarmyhammer-cel crate
pub use swissarmyhammer_cel::{cel_value_to_json, CelState};

/// Register CEL tools with the tool registry
pub fn register_cel_tools(registry: &mut crate::mcp::tool_registry::ToolRegistry) {
    registry.register(cel_set::CelSetTool::new());
    registry.register(cel_get::CelGetTool::new());
    tracing::debug!("Registered CEL tools");
}
