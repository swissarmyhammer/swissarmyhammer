use swissarmyhammer_common::{ErrorSeverity, Severity};
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

/// Implementation of Severity trait for WorkflowError
impl Severity for WorkflowError {
    fn severity(&self) -> ErrorSeverity {
        match self {
            // Critical: Structural problems that prevent workflow execution
            WorkflowError::CircularDependency { .. } => ErrorSeverity::Critical,
            WorkflowError::ExecutionFailed { .. } => ErrorSeverity::Critical,
            // Error: Configuration issues that prevent specific operations
            WorkflowError::NotFound { .. } => ErrorSeverity::Error,
            WorkflowError::Invalid { .. } => ErrorSeverity::Error,
            WorkflowError::StateNotFound { .. } => ErrorSeverity::Error,
            WorkflowError::InvalidTransition { .. } => ErrorSeverity::Error,
            WorkflowError::Timeout { .. } => ErrorSeverity::Error,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workflow_error_severity() {
        // Critical severity errors
        let circular_dep = WorkflowError::CircularDependency {
            cycle: "A -> B -> A".to_string(),
        };
        assert_eq!(circular_dep.severity(), ErrorSeverity::Critical);

        let exec_failed = WorkflowError::ExecutionFailed {
            reason: "action failed".to_string(),
        };
        assert_eq!(exec_failed.severity(), ErrorSeverity::Critical);

        // Error severity errors
        let not_found = WorkflowError::NotFound {
            name: "test_workflow".to_string(),
        };
        assert_eq!(not_found.severity(), ErrorSeverity::Error);

        let invalid = WorkflowError::Invalid {
            name: "bad_workflow".to_string(),
            reason: "missing states".to_string(),
        };
        assert_eq!(invalid.severity(), ErrorSeverity::Error);

        let state_not_found = WorkflowError::StateNotFound {
            state: "missing_state".to_string(),
            workflow: "test_workflow".to_string(),
        };
        assert_eq!(state_not_found.severity(), ErrorSeverity::Error);

        let invalid_transition = WorkflowError::InvalidTransition {
            from: "state_a".to_string(),
            to: "state_b".to_string(),
            workflow: "test_workflow".to_string(),
        };
        assert_eq!(invalid_transition.severity(), ErrorSeverity::Error);

        let timeout = WorkflowError::Timeout {
            duration: std::time::Duration::from_secs(30),
        };
        assert_eq!(timeout.severity(), ErrorSeverity::Error);
    }
}
