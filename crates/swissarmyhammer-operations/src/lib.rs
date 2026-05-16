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

mod execution_result;
mod operation;
mod parameter;
mod processor;
pub mod schema;

pub use execution_result::ExecutionResult;
pub use operation::{Execute, Operation};
pub use parameter::{ParamMeta, ParamType};
pub use processor::OperationProcessor;
pub use schema::{generate_mcp_schema, generate_operations_meta, SchemaConfig};

// Re-export proc macros
pub use swissarmyhammer_operations_macros::{operation, operation_tool, param};

// Re-export for use in implementations
pub use async_trait::async_trait;
pub use serde_json::Value;
