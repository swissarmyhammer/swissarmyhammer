//! Execution result types for operations

use crate::LogEntry;

/// Result of executing an operation
///
/// Distinguishes between:
/// - Logged: Operations that mutate state and should be audited
/// - Unlogged: Read-only operations with no side effects
/// - Failed: Errors (optionally logged)
pub enum ExecutionResult<T, E> {
    /// Operation succeeded and should be logged
    Logged { value: T, log_entry: LogEntry },
    /// Operation succeeded but no logging needed (read-only)
    Unlogged { value: T },
    /// Operation failed
    Failed {
        error: E,
        log_entry: Option<LogEntry>,
    },
}

impl<T, E> ExecutionResult<T, E> {
    /// Extract the result (Ok or Err)
    pub fn into_result(self) -> Result<T, E> {
        match self {
            Self::Logged { value, .. } => Ok(value),
            Self::Unlogged { value } => Ok(value),
            Self::Failed { error, .. } => Err(error),
        }
    }

    /// Get the value and log entry separately
    pub fn split(self) -> (Result<T, E>, Option<LogEntry>) {
        match self {
            Self::Logged { value, log_entry } => (Ok(value), Some(log_entry)),
            Self::Unlogged { value } => (Ok(value), None),
            Self::Failed { error, log_entry } => (Err(error), log_entry),
        }
    }

    /// Check if this should be logged
    pub fn should_log(&self) -> bool {
        matches!(
            self,
            Self::Logged { .. }
                | Self::Failed {
                    log_entry: Some(_),
                    ..
                }
        )
    }
}
