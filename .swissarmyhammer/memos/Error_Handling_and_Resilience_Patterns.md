# Error Handling and Resilience Patterns

## Error Type Hierarchy

### Application Errors
- Use `anyhow::Result<T>` for CLI applications and top-level error handling
- Use `anyhow::Context` to add contextual information to errors
- Chain errors with `.with_context(|| "contextual info")`

### Library Errors  
- Define specific error types using `thiserror`
- Implement `std::error::Error` trait
- Use `#[error("descriptive message")]` for error display
- Group related errors in enums

### Error Propagation
- Always use `?` operator for error propagation
- Never silently ignore errors with `let _ = ...`
- Log errors at the boundary where they're handled, not where they occur
- Use `tracing::error!` for logging errors

## Resilience Patterns

### Retry Logic
- Implement exponential backoff for transient failures
- Set maximum retry attempts (typically 3-5)
- Use jitter to prevent thundering herd
- Only retry on specific, recoverable error types

### Timeouts
- Set reasonable timeouts for all I/O operations
- Use `tokio::time::timeout` for async operations
- Distinguish between connection timeout and read timeout
- Make timeouts configurable

### Circuit Breaker
- Implement circuit breaker for external service calls
- Track failure rates and response times
- Fail fast when circuit is open
- Implement half-open state for recovery attempts

### Graceful Degradation
- Provide fallback behavior when possible
- Cache results for offline scenarios
- Use default values for non-critical failures
- Inform users about degraded functionality

## Validation Patterns

### Input Validation
- Validate at system boundaries (API endpoints, file parsing)
- Use newtype patterns for validated data
- Implement `TryFrom` for conversion with validation
- Return descriptive validation errors

### Business Rule Validation
- Encode business rules in the type system when possible
- Use builder patterns with validation steps
- Validate complete objects, not individual fields
- Separate syntax validation from semantic validation

## Recovery Patterns

### Transaction Safety
- Use database transactions for multi-step operations
- Implement compensation actions for distributed transactions
- Store operation state for crash recovery
- Use write-ahead logs for durability

### Resource Cleanup
- Always use RAII patterns with `Drop` trait
- Implement cleanup in `Drop` even if explicit cleanup exists
- Use `scopeguard` crate for complex cleanup scenarios
- Never assume destructors will run (they might not in panics)

### State Recovery
- Persist critical state to durable storage
- Implement state machine patterns for complex recovery
- Use checksums to verify state integrity
- Provide manual recovery tools for edge cases