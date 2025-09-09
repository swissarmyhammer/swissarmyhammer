//! Workflow execution engine

pub mod core;
pub mod fork_join;
#[cfg(test)]
mod tests;
pub mod validation;

use crate::{ActionError, StateId};
use thiserror::Error;

/// Errors that can occur during workflow execution
#[derive(Debug, Error)]
pub enum ExecutorError {
    /// State referenced in workflow does not exist
    #[error("State not found: {0}")]
    StateNotFound(StateId),
    /// Transition is invalid or not allowed
    #[error("Invalid transition: {0}")]
    InvalidTransition(String),
    /// Workflow validation failed before execution
    #[error("Workflow validation failed: {0}")]
    ValidationFailed(String),
    /// Maximum transition limit exceeded to prevent infinite loops
    #[error("Maximum transition limit of {limit} exceeded")]
    TransitionLimitExceeded {
        /// The maximum number of transitions that was exceeded
        limit: usize,
    },
    /// Generic workflow execution failure
    #[error("Execution failed: {0}")]
    ExecutionFailed(String),
    /// Attempted to resume a completed workflow
    #[error("Workflow already completed")]
    WorkflowCompleted,
    /// Expression evaluation failed
    #[error("Expression evaluation failed: {0}")]
    ExpressionError(String),
    /// Action execution failed
    #[error("Action execution failed: {0}")]
    ActionError(#[from] ActionError),
    /// Manual intervention required to continue workflow
    #[error("Manual intervention required: {0}")]
    ManualInterventionRequired(String),
    /// Workflow aborted via abort file
    #[error("Workflow aborted: {0}")]
    Abort(String),
}

/// Result type for executor operations
pub type ExecutorResult<T> = Result<T, ExecutorError>;

/// Maximum number of state transitions allowed in a single execution
pub const MAX_TRANSITIONS: usize = 1000;

/// Default maximum execution history size to prevent unbounded growth
pub const DEFAULT_MAX_HISTORY_SIZE: usize = 10000;

/// Context key for last action result
pub const LAST_ACTION_RESULT_KEY: &str = "last_action_result";

/// Event recorded during workflow execution
#[derive(Debug, Clone)]
pub struct ExecutionEvent {
    /// When the event occurred
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Type of execution event
    pub event_type: ExecutionEventType,
    /// Human-readable details about the event
    pub details: String,
}

/// Types of events that can occur during workflow execution
#[derive(Debug, Clone, Copy)]
pub enum ExecutionEventType {
    /// Workflow execution started
    Started,
    /// Transitioned to a new state
    StateTransition,
    /// Executed a state's action
    StateExecution,
    /// Evaluated a transition condition
    ConditionEvaluated,
    /// Workflow completed successfully
    Completed,
    /// Workflow execution failed
    Failed,
}

// Implement Display for ExecutionEventType
impl std::fmt::Display for ExecutionEventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            ExecutionEventType::Started => "Started",
            ExecutionEventType::StateTransition => "StateTransition",
            ExecutionEventType::StateExecution => "StateExecution",
            ExecutionEventType::ConditionEvaluated => "ConditionEvaluated",
            ExecutionEventType::Completed => "Completed",
            ExecutionEventType::Failed => "Failed",
        };
        write!(f, "{s}")
    }
}

// Re-export main types
pub use core::WorkflowExecutor;
