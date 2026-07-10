//! # SwissArmyHammer Operations
//!
//! This crate provides the `Operation` trait for defining tool operations.
//! Operations are structs where the fields ARE the parameters - no duplication.
//!
//! ## Example
//!
//! ```ignore
//! use swissarmyhammer_operations::*;
//!
//! #[operation(verb = "add", noun = "task", description = "Create a new task")]
//! #[derive(Debug, Deserialize)]
//! pub struct AddTask {
//!     /// The task title
//!     pub title: String,
//!     /// Optional description
//!     pub description: Option<String>,
//! }
//!
//! #[async_trait]
//! impl Execute<KanbanContext, KanbanError> for AddTask {
//!     async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
//!         // implementation returns ExecutionResult::Success or Failed
//!     }
//! }
//! ```

pub mod cli_gen;
mod execution_result;
mod notification;
mod operation;
mod parameter;
mod processor;
pub mod schema;

#[cfg(test)]
pub(crate) mod test_support;

pub use execution_result::ExecutionResult;
pub use notification::Notification;
pub use operation::{Execute, Operation};
pub use parameter::{ParamMeta, ParamType};
pub use processor::OperationProcessor;
pub use schema::{
    generate_mcp_schema, generate_mcp_schema_full, generate_mcp_schema_wire,
    generate_notifications_meta, generate_operations_meta, SchemaConfig, WIRE_DROPPED_KEYS,
};

// Re-export proc macros
pub use swissarmyhammer_operations_macros::{notification, operation, operation_tool, param};

/// The `_meta` key under which an operation tool carries its
/// noun → verb → operation discovery tree (see [`generate_operations_meta`]).
///
/// Centralized here so the producer (`operation_tool!`) and Rust consumers
/// reference one constant instead of repeating the bare literal.
pub const OPERATIONS_META_KEY: &str = "io.swissarmyhammer/operations";

/// The `_meta` key under which an operation tool carries its
/// event → notification discovery tree (see [`generate_notifications_meta`]).
pub const NOTIFICATIONS_META_KEY: &str = "io.swissarmyhammer/notifications";

// Re-export for use in implementations
pub use async_trait::async_trait;
pub use serde_json::Value;
