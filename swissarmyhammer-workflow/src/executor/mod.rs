//! Workflow execution engine

pub mod core;
pub mod fork_join;
#[cfg(test)]
mod result_cel_test;
#[cfg(test)]
mod tests;
pub mod validation;

use crate::{ActionError, StateId};
use swissarmyhammer_common::{ErrorSeverity, Severity};
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

/// Convert ExecutorError to SwissArmyHammerError
impl From<ExecutorError> for swissarmyhammer_common::SwissArmyHammerError {
    fn from(err: ExecutorError) -> Self {
        swissarmyhammer_common::SwissArmyHammerError::Other {
            message: format!("Executor error: {}", err),
        }
    }
}

/// Implementation of Severity trait for ExecutorError
impl Severity for ExecutorError {
    fn severity(&self) -> ErrorSeverity {
        match self {
            // Critical: System-level failures that prevent workflow execution
            ExecutorError::ValidationFailed(_) => ErrorSeverity::Critical,
            ExecutorError::TransitionLimitExceeded { .. } => ErrorSeverity::Critical,
            ExecutorError::ExecutionFailed(_) => ErrorSeverity::Critical,
            ExecutorError::Abort(_) => ErrorSeverity::Critical,
            // Error: Recoverable operation failures
            ExecutorError::StateNotFound(_) => ErrorSeverity::Error,
            ExecutorError::InvalidTransition(_) => ErrorSeverity::Error,
            ExecutorError::ExpressionError(_) => ErrorSeverity::Error,
            ExecutorError::ActionError(_) => ErrorSeverity::Error,
            ExecutorError::ManualInterventionRequired(_) => ErrorSeverity::Error,
        }
    }
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

#[cfg(test)]
mod severity_tests {
    use super::*;

    #[test]
    fn test_executor_error_severity() {
        // Critical severity errors
        let validation_failed = ExecutorError::ValidationFailed("invalid workflow".to_string());
        assert_eq!(validation_failed.severity(), ErrorSeverity::Critical);

        let transition_limit = ExecutorError::TransitionLimitExceeded { limit: 1000 };
        assert_eq!(transition_limit.severity(), ErrorSeverity::Critical);

        let exec_failed = ExecutorError::ExecutionFailed("action failed".to_string());
        assert_eq!(exec_failed.severity(), ErrorSeverity::Critical);

        let abort = ExecutorError::Abort("user cancelled".to_string());
        assert_eq!(abort.severity(), ErrorSeverity::Critical);

        // Error severity errors
        let state_not_found = ExecutorError::StateNotFound(StateId::from("missing"));
        assert_eq!(state_not_found.severity(), ErrorSeverity::Error);

        let invalid_transition = ExecutorError::InvalidTransition("invalid".to_string());
        assert_eq!(invalid_transition.severity(), ErrorSeverity::Error);

        let expr_error = ExecutorError::ExpressionError("eval failed".to_string());
        assert_eq!(expr_error.severity(), ErrorSeverity::Error);

        let action_error =
            ExecutorError::ActionError(ActionError::ExecutionError("failed".to_string()));
        assert_eq!(action_error.severity(), ErrorSeverity::Error);

        let manual_intervention =
            ExecutorError::ManualInterventionRequired("approval needed".to_string());
        assert_eq!(manual_intervention.severity(), ErrorSeverity::Error);
    }
}
