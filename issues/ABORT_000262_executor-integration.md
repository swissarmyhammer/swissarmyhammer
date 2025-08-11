# Add Abort File Detection to Workflow Executor

Refer to ./specification/abort.md

## Objective
Integrate file-based abort detection into the workflow executor's main loop to enable immediate termination when abort is requested via the MCP tool.

## Context
The workflow executor's `execute_state_with_limit` function needs to check for abort files before each iteration to enable responsive abort handling. This replaces the current string-based "ABORT ERROR" detection with robust file-based detection.

## Tasks

### 1. Locate execute_state_with_limit Function
- Find `execute_state_with_limit` in `swissarmyhammer/src/workflow/executor/core.rs:215-250`
- Review existing loop structure and validation logic
- Identify optimal location for abort check

### 2. Implement Abort File Detection
Add abort file check in main execution loop:
```rust
pub async fn execute_state_with_limit(
    &mut self,
    run: &mut WorkflowRun,
    remaining_transitions: usize,
) -> ExecutorResult<()> {
    // ... existing validation ...

    loop {
        // Check for abort file before each iteration
        if std::path::Path::new(".swissarmyhammer/.abort").exists() {
            let reason = std::fs::read_to_string(".swissarmyhammer/.abort")
                .unwrap_or_else(|_| "Unknown abort reason".to_string());
            return Err(ExecutorError::Abort(reason));
        }

        // ... rest of existing loop logic ...
    }
}
```

### 3. Add ExecutorError::Abort Variant
Add new error type to handle abort conditions in executor error enum:
```rust
#[derive(Debug, thiserror::Error)]
pub enum ExecutorError {
    // ... existing variants ...
    
    #[error("Workflow aborted: {0}")]
    Abort(String),
}
```

### 4. Error Propagation
- Ensure `ExecutorError::Abort` propagates correctly through the call chain
- Maintain error context and abort reason information
- Follow existing error handling patterns

### 5. Performance Considerations
- File existence check should be fast (no full file read unless exists)
- Place check at optimal location in loop to minimize overhead
- Consider caching strategies if needed

## Implementation Details

### File Check Logic
```rust
// Check for abort before each workflow iteration
if std::path::Path::new(".swissarmyhammer/.abort").exists() {
    match std::fs::read_to_string(".swissarmyhammer/.abort") {
        Ok(reason) => return Err(ExecutorError::Abort(reason)),
        Err(e) => {
            tracing::warn!("Found abort file but couldn't read reason: {}", e);
            return Err(ExecutorError::Abort("Abort requested (reason unavailable)".to_string()));
        }
    }
}
```

### Loop Integration Strategy
- Add check at the beginning of each loop iteration
- Place after existing validation but before state execution
- Ensure abort is checked frequently for responsiveness

### Error Type Addition
```rust
#[derive(Debug, thiserror::Error)]
pub enum ExecutorError {
    #[error("Action execution failed: {0}")]
    ActionExecution(#[from] ActionError),
    #[error("State '{state}' not found in workflow")]
    StateNotFound { state: String },
    #[error("Maximum transitions ({0}) exceeded")]
    MaxTransitions(usize),
    #[error("Workflow aborted: {0}")]
    Abort(String),
    // ... other variants
}
```

## Validation Criteria
- [ ] Abort file detection works in workflow execution loop
- [ ] ExecutorError::Abort is created with correct reason text
- [ ] Error propagates correctly through executor system
- [ ] Workflow execution terminates immediately on abort
- [ ] Performance impact is minimal
- [ ] Existing workflow functionality remains intact
- [ ] Abort reason is preserved in error message

## Testing Requirements
- Unit tests for abort detection logic
- Tests for abort file reading and error handling
- Integration tests with workflow execution
- Performance tests for loop overhead
- Error propagation tests

## Dependencies
- ABORT_000261_workflowrun-cleanup-integration (cleanup must be in place)
- ABORT_000260_core-abort-tool-implementation (abort file format established)

## Follow-up Issues
- ABORT_000263_cli-error-handling-updates