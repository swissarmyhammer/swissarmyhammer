//! Execution result types for operations

use crate::LogEntry;

/// Result of executing an operation
///
/// Distinguishes between:
/// - Logged: Operations that mutate state and should be audited
/// - Unlogged: Read-only operations with no side effects
/// - Failed: Errors (optionally logged)
#[derive(Debug)]
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_log_entry() -> LogEntry {
        LogEntry::new("test op", json!({}), json!({}), None, 0)
    }

    #[test]
    fn test_into_result_logged() {
        let entry = make_log_entry();
        let result: ExecutionResult<i32, String> = ExecutionResult::Logged {
            value: 42,
            log_entry: entry,
        };
        assert_eq!(result.into_result(), Ok(42));
    }

    #[test]
    fn test_into_result_unlogged() {
        let result: ExecutionResult<i32, String> = ExecutionResult::Unlogged { value: 99 };
        assert_eq!(result.into_result(), Ok(99));
    }

    #[test]
    fn test_into_result_failed() {
        let result: ExecutionResult<i32, String> = ExecutionResult::Failed {
            error: "oops".to_string(),
            log_entry: None,
        };
        assert_eq!(result.into_result(), Err("oops".to_string()));
    }

    #[test]
    fn test_split_logged() {
        let entry = make_log_entry();
        let result: ExecutionResult<i32, String> = ExecutionResult::Logged {
            value: 1,
            log_entry: entry,
        };
        let (value, log) = result.split();
        assert_eq!(value, Ok(1));
        assert!(log.is_some());
    }

    #[test]
    fn test_split_unlogged() {
        let result: ExecutionResult<i32, String> = ExecutionResult::Unlogged { value: 2 };
        let (value, log) = result.split();
        assert_eq!(value, Ok(2));
        assert!(log.is_none());
    }

    #[test]
    fn test_split_failed_with_log() {
        let entry = make_log_entry();
        let result: ExecutionResult<i32, String> = ExecutionResult::Failed {
            error: "err".to_string(),
            log_entry: Some(entry),
        };
        let (value, log) = result.split();
        assert_eq!(value, Err("err".to_string()));
        assert!(log.is_some());
    }

    #[test]
    fn test_split_failed_without_log() {
        let result: ExecutionResult<i32, String> = ExecutionResult::Failed {
            error: "err".to_string(),
            log_entry: None,
        };
        let (value, log) = result.split();
        assert_eq!(value, Err("err".to_string()));
        assert!(log.is_none());
    }

    #[test]
    fn test_should_log_logged() {
        let result: ExecutionResult<i32, String> = ExecutionResult::Logged {
            value: 1,
            log_entry: make_log_entry(),
        };
        assert!(result.should_log());
    }

    #[test]
    fn test_should_log_unlogged() {
        let result: ExecutionResult<i32, String> = ExecutionResult::Unlogged { value: 1 };
        assert!(!result.should_log());
    }

    #[test]
    fn test_should_log_failed_with_entry() {
        let result: ExecutionResult<i32, String> = ExecutionResult::Failed {
            error: "x".to_string(),
            log_entry: Some(make_log_entry()),
        };
        assert!(result.should_log());
    }

    #[test]
    fn test_should_log_failed_without_entry() {
        let result: ExecutionResult<i32, String> = ExecutionResult::Failed {
            error: "x".to_string(),
            log_entry: None,
        };
        assert!(!result.should_log());
    }

    #[test]
    fn test_debug_logged() {
        let result: ExecutionResult<i32, String> = ExecutionResult::Logged {
            value: 42,
            log_entry: make_log_entry(),
        };
        let debug = format!("{:?}", result);
        assert!(debug.contains("Logged"));
        assert!(debug.contains("42"));
    }

    #[test]
    fn test_debug_unlogged() {
        let result: ExecutionResult<i32, String> = ExecutionResult::Unlogged { value: 99 };
        let debug = format!("{:?}", result);
        assert!(debug.contains("Unlogged"));
        assert!(debug.contains("99"));
    }

    #[test]
    fn test_debug_failed() {
        let result: ExecutionResult<i32, String> = ExecutionResult::Failed {
            error: "boom".to_string(),
            log_entry: None,
        };
        let debug = format!("{:?}", result);
        assert!(debug.contains("Failed"));
        assert!(debug.contains("boom"));
    }
}
