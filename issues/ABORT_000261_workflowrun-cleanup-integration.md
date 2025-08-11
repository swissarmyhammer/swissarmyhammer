# Integrate Abort File Cleanup into WorkflowRun

Refer to ./specification/abort.md

## Objective
Add abort file cleanup logic to `WorkflowRun::new()` to ensure clean slate for each workflow execution and prevent stale abort states from affecting new runs.

## Context
The workflow system must clean up any existing abort files when starting a new workflow run to prevent old abort states from incorrectly terminating new workflows. This cleanup happens at the entry point of workflow execution.

## Tasks

### 1. Locate WorkflowRun::new()
- Find `WorkflowRun::new()` in `swissarmyhammer/src/workflow/run.rs:79-93`
- Review existing initialization logic
- Identify proper location for cleanup code

### 2. Implement Abort File Cleanup
Add cleanup logic to remove existing abort files:
```rust
pub fn new(workflow: Workflow) -> Self {
    // Clean up any existing abort file
    if let Err(e) = std::fs::remove_file(".swissarmyhammer/.abort") {
        if e.kind() != std::io::ErrorKind::NotFound {
            tracing::warn!("Failed to clean up abort file: {}", e);
        }
    }
    
    // ... rest of existing implementation
}
```

### 3. Error Handling Strategy
- Handle `NotFound` errors silently (file doesn't exist is OK)
- Log warnings for other file system errors
- Don't fail workflow initialization if cleanup fails
- Use appropriate log levels for different scenarios

### 4. Logging Integration
- Add appropriate logging statements using `tracing` crate
- Use `warn!` level for cleanup failures
- Optionally add `debug!` level for successful cleanup
- Follow existing logging patterns in the codebase

## Implementation Details

### Error Handling Pattern
```rust
match std::fs::remove_file(".swissarmyhammer/.abort") {
    Ok(()) => {
        tracing::debug!("Cleaned up existing abort file");
    }
    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
        // File doesn't exist, no cleanup needed
    }
    Err(e) => {
        tracing::warn!("Failed to clean up abort file: {}", e);
        // Continue with workflow initialization
    }
}
```

### Location Strategy
- Add cleanup at the very beginning of `WorkflowRun::new()`
- Before any other initialization logic
- Ensure cleanup happens for every workflow run

## Validation Criteria
- [ ] Abort file is removed when starting new workflow run
- [ ] WorkflowRun initialization still succeeds if cleanup fails
- [ ] Appropriate logging for cleanup operations
- [ ] No regression in existing workflow functionality
- [ ] Cleanup handles missing files gracefully
- [ ] Cleanup handles file system permission errors

## Testing Requirements
- Unit tests for cleanup logic
- Test cleanup with existing abort file
- Test cleanup when no abort file exists
- Test cleanup when file system errors occur
- Integration tests with workflow execution

## Dependencies
- ABORT_000260_core-abort-tool-implementation (abort file format must be established)

## Follow-up Issues
- ABORT_000262_executor-integration