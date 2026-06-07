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
mod operation;
mod parameter;
mod processor;
pub mod schema;

#[cfg(test)]
pub(crate) mod test_support;

pub use execution_result::ExecutionResult;
pub use operation::{Execute, Operation};
pub use parameter::{ParamMeta, ParamType};
pub use processor::OperationProcessor;
pub use schema::{
    generate_mcp_schema, generate_mcp_schema_full, generate_mcp_schema_wire, SchemaConfig,
    WIRE_DROPPED_KEYS,
};

// Re-export proc macros
pub use swissarmyhammer_operations_macros::{operation, param};

// Re-export for use in implementations
pub use async_trait::async_trait;
pub use serde_json::Value;
