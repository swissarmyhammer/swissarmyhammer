# Update CLI Error Handling for File-Based Abort Detection

Refer to ./specification/abort.md

## Objective
Update CLI error handling to detect the new `ExecutorError::Abort` variant and remove string-based "ABORT ERROR" detection, maintaining proper exit codes and error messaging.

## Context
The CLI currently uses string-based detection for "ABORT ERROR" patterns in multiple locations. These need to be updated to handle the new `ExecutorError::Abort` variant from the workflow executor while maintaining existing behavior and exit codes.

## Tasks

### 1. Update Main CLI Error Handling
Location: `swissarmyhammer-cli/src/main.rs:279`
- Remove `error_msg.contains("ABORT ERROR")` check
- Add pattern matching for `ExecutorError::Abort`
- Maintain EXIT_ERROR exit code for abort conditions

### 2. Update Prompt Command Error Handling  
Location: `swissarmyhammer-cli/src/prompt.rs:42`
- Remove `error_msg.contains("ABORT ERROR")` check
- Add proper error type matching for abort conditions
- Preserve existing error message formatting

### 3. Update Test Command Error Handling
Location: `swissarmyhammer-cli/src/test.rs:280-284`
- Remove string-based abort error detection
- Update to use proper error type checking
- Maintain test validation behavior

### 4. Remove Abort Error Helper Function
Location: `swissarmyhammer-cli/src/error.rs:32-36`
- Remove `is_abort_error` function that checks for string patterns
- Update any code that uses this function
- Clean up obsolete string-based detection utilities

## Implementation Details

### Main CLI Pattern Matching
```rust
// Replace string-based detection with type-based detection
match workflow_result {
    Err(ref e) => {
        if let Some(executor_error) = e.downcast_ref::<ExecutorError>() {
            match executor_error {
                ExecutorError::Abort(reason) => {
                    tracing::error!("Workflow aborted: {}", reason);
                    std::process::exit(EXIT_ERROR);
                }
                _ => {
                    // Handle other executor errors
                }
            }
        }
        // Handle other error types
    }
    Ok(_) => {
        // Success case
    }
}
```

### Error Propagation Strategy
- Ensure `ExecutorError::Abort` can be downcast from boxed errors
- Maintain error context and reason information
- Preserve logging patterns for abort conditions

### Exit Code Consistency
- Continue using EXIT_ERROR (2) for abort conditions
- Maintain consistency with existing error exit codes
- Ensure proper error message formatting

## Validation Criteria
- [ ] CLI detects `ExecutorError::Abort` correctly
- [ ] String-based "ABORT ERROR" detection is removed
- [ ] Proper exit codes are maintained (EXIT_ERROR for aborts)
- [ ] Error messages include abort reason
- [ ] Existing CLI functionality remains intact
- [ ] Error logging follows established patterns
- [ ] No regression in error handling behavior

## Testing Requirements
- Unit tests for CLI error handling updates
- Integration tests with workflow abort scenarios
- Test proper exit code behavior
- Verify error message formatting
- Regression tests for existing error handling

## Files to Modify
- `swissarmyhammer-cli/src/main.rs` - Main error handling
- `swissarmyhammer-cli/src/prompt.rs` - Prompt command error handling
- `swissarmyhammer-cli/src/test.rs` - Test command error handling
- `swissarmyhammer-cli/src/error.rs` - Remove abort helper function

## Dependencies
- ABORT_000262_executor-integration (ExecutorError::Abort must be available)

## Follow-up Issues
- ABORT_000264_builtin-prompt-updates