use thiserror::Error as ThisError;

/// Workflow-specific errors
#[derive(Debug, ThisError)]
#[non_exhaustive]
pub enum WorkflowError {
    /// Workflow not found
    #[error("Workflow '{name}' not found")]
    NotFound {
        /// The name of the workflow that was not found
        name: String,
    },

    /// Invalid workflow definition
    #[error("Invalid workflow '{name}': {reason}")]
    Invalid {
        /// The name of the invalid workflow
        name: String,
        /// The reason why the workflow is invalid
        reason: String,
    },

    /// Circular dependency detected
    #[error("Circular dependency detected: {cycle}")]
    CircularDependency {
        /// The string representation of the dependency cycle
        cycle: String,
    },

    /// State not found in workflow
    #[error("State '{state}' not found in workflow '{workflow}'")]
    StateNotFound {
        /// The state that was not found
        state: String,
        /// The workflow that should contain the state
        workflow: String,
    },

    /// Invalid transition
    #[error("Invalid transition from '{from}' to '{to}' in workflow '{workflow}'")]
    InvalidTransition {
        /// The source state of the invalid transition
        from: String,
        /// The target state of the invalid transition
        to: String,
        /// The workflow containing the invalid transition
        workflow: String,
    },

    /// Workflow execution error
    #[error("Workflow execution failed: {reason}")]
    ExecutionFailed {
        /// The reason why the workflow execution failed
        reason: String,
    },

    /// Timeout during workflow execution
    #[error("Workflow execution timed out after {duration:?}")]
    Timeout {
        /// The duration after which the workflow timed out
        duration: std::time::Duration,
    },
}
