//! Queue-specific validation logic for operation limits and capacity

use super::{ValidationError, ValidationResult, Validator};
use crate::types::{QueueError, Session};

/// Configuration for queue validation limits and capacity constraints
///
/// Defines the operational limits for various queue operations to prevent
/// resource exhaustion and maintain system stability.
#[derive(Debug, Clone)]
pub struct QueueLimits {
    pub max_queue_size: usize,
    pub max_batch_size: usize,
    pub max_concurrent_workers: usize,
}

impl Default for QueueLimits {
    fn default() -> Self {
        Self {
            max_queue_size: 1000,
            max_batch_size: 100,
            max_concurrent_workers: 10,
        }
    }
}

/// Queue operation types for validation
///
/// Represents different types of queue operations that require validation
/// against configured limits and capacity constraints.
#[derive(Debug, Clone)]
pub enum QueueOperation {
    /// Enqueue operation with current queue size
    Enqueue { current_size: usize },
    /// Batch processing operation with proposed batch size
    BatchProcess { batch_size: usize },
    /// Worker spawn operation with current worker count
    WorkerSpawn { current_workers: usize },
}

/// Validates queue operations against configured limits and constraints
///
/// The `QueueValidator` ensures that queue operations stay within safe
/// operational limits to prevent resource exhaustion, memory issues, and
/// system instability. It validates enqueue operations, batch processing
/// sizes, and concurrent worker limits.
///
/// # Validation Categories
///
/// - **Capacity Limits**: Prevents queue overflow beyond maximum size
/// - **Batch Size Limits**: Ensures batch operations don't exceed safe sizes
/// - **Concurrency Limits**: Controls maximum concurrent workers
///
/// # Usage
///
/// ```rust
/// use crate::validation::{QueueValidator, QueueLimits, QueueOperation};
///
/// let limits = QueueLimits {
///     max_queue_size: 1000,
///     max_batch_size: 50,
///     max_concurrent_workers: 8,
/// };
/// let validator = QueueValidator::new(limits);
///
/// let operation = QueueOperation::Enqueue { current_size: 950 };
/// match validator.validate(&session, &operation) {
///     Ok(()) => println!("Queue operation is within limits"),
///     Err(e) => println!("Operation rejected: {}", e),
/// }
/// ```
///
/// # Thread Safety
///
/// This validator is thread-safe and can be shared across multiple threads.
pub struct QueueValidator {
    limits: QueueLimits,
}

impl QueueValidator {
    /// Creates a new queue validator with specified limits
    ///
    /// # Arguments
    ///
    /// * `limits` - The queue limits and constraints to enforce
    ///
    /// # Returns
    ///
    /// A new `QueueValidator` instance configured with the provided limits
    pub fn new(limits: QueueLimits) -> Self {
        Self { limits }
    }

    /// Creates a new queue validator with default limits
    ///
    /// Uses the default limits:
    /// - Max queue size: 1000
    /// - Max batch size: 100  
    /// - Max concurrent workers: 10
    ///
    /// # Returns
    ///
    /// A new `QueueValidator` instance with default configuration
    pub fn with_defaults() -> Self {
        Self::new(QueueLimits::default())
    }
}

impl Default for QueueValidator {
    fn default() -> Self {
        Self::with_defaults()
    }
}

impl Validator<QueueOperation> for QueueValidator {
    type Error = ValidationError;

    fn validate(&self, _context: &Session, target: &QueueOperation) -> ValidationResult {
        match target {
            QueueOperation::Enqueue { current_size } => {
                if *current_size >= self.limits.max_queue_size {
                    return Err(ValidationError::parameter_bounds(format!(
                        "Queue is at capacity ({}/{}). Cannot enqueue more items",
                        current_size, self.limits.max_queue_size
                    )));
                }
            }
            QueueOperation::BatchProcess { batch_size } => {
                if *batch_size == 0 {
                    return Err(ValidationError::invalid_state(
                        "Batch size must be greater than zero",
                    ));
                }

                if *batch_size > self.limits.max_batch_size {
                    return Err(ValidationError::parameter_bounds(format!(
                        "Batch size ({}) exceeds maximum allowed ({})",
                        batch_size, self.limits.max_batch_size
                    )));
                }
            }
            QueueOperation::WorkerSpawn { current_workers } => {
                if *current_workers >= self.limits.max_concurrent_workers {
                    return Err(ValidationError::parameter_bounds(format!(
                        "Maximum concurrent workers reached ({}/{})",
                        current_workers, self.limits.max_concurrent_workers
                    )));
                }
            }
        }

        Ok(())
    }
}

/// Convert QueueError to ValidationError for compatibility
impl From<QueueError> for ValidationError {
    fn from(err: QueueError) -> Self {
        match err {
            QueueError::Full => ValidationError::parameter_bounds("Queue is at capacity"),
            QueueError::WorkerError(msg) => {
                ValidationError::invalid_state(format!("Queue worker error: {}", msg))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Message, MessageRole, SessionId};
    use std::time::SystemTime;

    fn create_test_session() -> Session {
        Session {
            id: SessionId::new(),
            messages: vec![Message {
                role: MessageRole::User,
                content: "Test message".to_string(),
                tool_call_id: None,
                tool_name: None,
                timestamp: SystemTime::now(),
            }],
            mcp_servers: vec![],
            available_tools: vec![],
            available_prompts: vec![],
            created_at: SystemTime::now(),
            updated_at: SystemTime::now(),
            compaction_history: Vec::new(),
            transcript_path: None,
            context_state: None,
            template_token_count: None,
            #[cfg(feature = "acp")]
            todos: Vec::new(),
            #[cfg(feature = "acp")]
            available_commands: Vec::new(),
            current_mode: None,
            #[cfg(feature = "acp")]
            client_capabilities: None,
        }
    }

    #[test]
    fn test_queue_validator_valid_enqueue() {
        let validator = QueueValidator::with_defaults();
        let session = create_test_session();
        let operation = QueueOperation::Enqueue { current_size: 500 };

        let result = validator.validate(&session, &operation);
        assert!(result.is_ok());
    }

    #[test]
    fn test_queue_validator_full_queue() {
        let limits = QueueLimits {
            max_queue_size: 100,
            max_batch_size: 50,
            max_concurrent_workers: 5,
        };
        let validator = QueueValidator::new(limits);
        let session = create_test_session();
        let operation = QueueOperation::Enqueue { current_size: 100 };

        let result = validator.validate(&session, &operation);
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert!(matches!(err, ValidationError::ParameterBounds(_)));
        assert!(err.to_string().contains("at capacity"));
    }

    #[test]
    fn test_queue_validator_valid_batch() {
        let validator = QueueValidator::with_defaults();
        let session = create_test_session();
        let operation = QueueOperation::BatchProcess { batch_size: 50 };

        let result = validator.validate(&session, &operation);
        assert!(result.is_ok());
    }

    #[test]
    fn test_queue_validator_zero_batch_size() {
        let validator = QueueValidator::with_defaults();
        let session = create_test_session();
        let operation = QueueOperation::BatchProcess { batch_size: 0 };

        let result = validator.validate(&session, &operation);
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert!(matches!(err, ValidationError::InvalidState(_)));
        assert!(err.to_string().contains("greater than zero"));
    }

    #[test]
    fn test_queue_validator_batch_too_large() {
        let limits = QueueLimits {
            max_queue_size: 1000,
            max_batch_size: 10,
            max_concurrent_workers: 5,
        };
        let validator = QueueValidator::new(limits);
        let session = create_test_session();
        let operation = QueueOperation::BatchProcess { batch_size: 20 };

        let result = validator.validate(&session, &operation);
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert!(matches!(err, ValidationError::ParameterBounds(_)));
        assert!(err.to_string().contains("exceeds maximum"));
    }

    #[test]
    fn test_queue_validator_valid_worker_spawn() {
        let validator = QueueValidator::with_defaults();
        let session = create_test_session();
        let operation = QueueOperation::WorkerSpawn { current_workers: 5 };

        let result = validator.validate(&session, &operation);
        assert!(result.is_ok());
    }

    #[test]
    fn test_queue_validator_max_workers_reached() {
        let limits = QueueLimits {
            max_queue_size: 1000,
            max_batch_size: 100,
            max_concurrent_workers: 3,
        };
        let validator = QueueValidator::new(limits);
        let session = create_test_session();
        let operation = QueueOperation::WorkerSpawn { current_workers: 3 };

        let result = validator.validate(&session, &operation);
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert!(matches!(err, ValidationError::ParameterBounds(_)));
        assert!(err
            .to_string()
            .contains("Maximum concurrent workers reached"));
    }

    #[test]
    fn test_queue_error_conversion() {
        let queue_error = QueueError::Full;
        let validation_error: ValidationError = queue_error.into();

        assert!(matches!(
            validation_error,
            ValidationError::ParameterBounds(_)
        ));
        assert!(validation_error.to_string().contains("at capacity"));
    }

    #[test]
    fn test_queue_worker_error_conversion() {
        let queue_error = QueueError::WorkerError("processing failed".to_string());
        let validation_error: ValidationError = queue_error.into();

        assert!(matches!(validation_error, ValidationError::InvalidState(_)));
        assert!(validation_error.to_string().contains("worker error"));
        assert!(validation_error.to_string().contains("processing failed"));
    }
}
