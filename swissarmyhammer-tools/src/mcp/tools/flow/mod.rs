//! Flow tools for MCP operations
//!
//! This module provides tools for dynamic workflow execution via MCP. The flow tool
//! enables both workflow execution and discovery through a unified interface.
//!
//! ## Architecture Overview
//!
//! The flow MCP tool implements a unified interface for workflow operations:
//!
//! 1. **Single Tool**: One `flow` tool handles both execution and discovery
//! 2. **Special Case**: When `flow_name="list"`, returns workflow metadata
//! 3. **Dynamic Schema**: Workflow names are included in the tool schema enum
//! 4. **Parameters**: Workflow-specific parameters passed as key-value pairs
//!
//! ## Workflow Discovery
//!
//! The flow tool supports workflow discovery through the special "list" flow name:
//!
//! ```ignore
//! {
//!   "flow_name": "list",
//!   "format": "json",
//!   "verbose": true
//! }
//! ```
//!
//! Returns:
//! ```ignore
//! {
//!   "workflows": [
//!     {
//!       "name": "implement",
//!       "description": "Execute the implement workflow",
//!       "source": "builtin",
//!       "parameters": []
//!     }
//!   ]
//! }
//! ```
//!
//! ## Workflow Execution
//!
//! Execute workflows by specifying the workflow name and parameters:
//!
//! ```ignore
//! {
//!   "flow_name": "plan",
//!   "parameters": {
//!     "plan_filename": "spec.md"
//!   },
//!   "interactive": false,
//!   "dry_run": false,
//!   "quiet": false
//! }
//! ```
//!
//! ## Implementation Status
//!
//! This module currently provides:
//! - Type definitions for requests and responses
//! - Schema generation utilities
//! - Comprehensive test coverage
//!
//! Future additions will include:
//! - Flow tool implementation with McpTool trait
//! - Integration with workflow storage and execution
//! - MCP notification support for long-running workflows
//! - CLI command generation

pub mod types;

// Re-export commonly used types
pub use types::{
    generate_flow_tool_schema, FlowToolRequest, WorkflowListResponse, WorkflowMetadata,
    WorkflowParameter,
};
