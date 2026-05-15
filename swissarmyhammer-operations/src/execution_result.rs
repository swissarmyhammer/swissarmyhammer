//! Execution result types for operations

/// Result of executing an operation
///
/// A successful operation returns its value; a failed operation returns an error.
/// The processor lifts this into the domain `Result<T, E>` for the caller.
#[derive(Debug)]
pub enum ExecutionResult<T, E> {
    /// Operation succeeded
    Success { value: T },
    /// Operation failed
    Failed { error: E },
}

impl<T, E> ExecutionResult<T, E> {
    /// Extract the result (Ok or Err)
    pub fn into_result(self) -> Result<T, E> {
        match self {
            Self::Success { value } => Ok(value),
            Self::Failed { error } => Err(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_into_result_success() {
        let result: ExecutionResult<i32, String> = ExecutionResult::Success { value: 42 };
        assert_eq!(result.into_result(), Ok(42));
    }

    #[test]
    fn test_into_result_failed() {
        let result: ExecutionResult<i32, String> = ExecutionResult::Failed {
            error: "oops".to_string(),
        };
        assert_eq!(result.into_result(), Err("oops".to_string()));
    }

    #[test]
    fn test_debug_success() {
        let result: ExecutionResult<i32, String> = ExecutionResult::Success { value: 42 };
        let debug = format!("{:?}", result);
        assert!(debug.contains("Success"));
        assert!(debug.contains("42"));
    }

    #[test]
    fn test_debug_failed() {
        let result: ExecutionResult<i32, String> = ExecutionResult::Failed {
            error: "boom".to_string(),
        };
        let debug = format!("{:?}", result);
        assert!(debug.contains("Failed"));
        assert!(debug.contains("boom"));
    }
}
