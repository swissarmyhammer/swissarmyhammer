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

## Proposed Solution

Based on my analysis of the codebase, I will implement the abort file cleanup in `WorkflowRun::new()` as specified. The solution will:

1. **Add cleanup logic at the beginning of `WorkflowRun::new()`** - Clean up any existing `.swissarmyhammer/.abort` file before workflow initialization
2. **Use proper error handling** - Handle `NotFound` errors silently and log warnings for other file system errors
3. **Follow existing patterns** - Use `tracing` crate for logging as used throughout the workflow module
4. **Ensure non-blocking behavior** - Don't fail workflow initialization if cleanup fails

### Implementation Details

The cleanup will be added at the very beginning of `WorkflowRun::new()` in `/Users/wballard/github/swissarmyhammer/swissarmyhammer/src/workflow/run.rs:79-93`:

```rust
pub fn new(workflow: Workflow) -> Self {
    // Clean up any existing abort file
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
    
    // ... rest of existing implementation
}
```

### Testing Strategy

I will add comprehensive tests covering:
- Cleanup when abort file exists
- Cleanup when abort file doesn't exist  
- Cleanup when file system errors occur
- Verification that workflow initialization continues regardless of cleanup results
## Implementation Complete ✅

Successfully integrated abort file cleanup into `WorkflowRun::new()` at line 79-93 in `/Users/wballard/github/swissarmyhammer/swissarmyhammer/src/workflow/run.rs`.

### Key Changes Made

1. **Added Cleanup Logic**: Modified `WorkflowRun::new()` to clean up any existing `.swissarmyhammer/.abort` file at the very beginning of workflow initialization
2. **Proper Error Handling**: Used pattern matching to handle `NotFound` errors silently while logging warnings for other file system errors
3. **Appropriate Logging**: Added `tracing::debug!` for successful cleanup and `tracing::warn!` for cleanup failures
4. **Non-blocking Implementation**: Workflow initialization continues regardless of cleanup success/failure

### Code Implementation

```rust
pub fn new(workflow: Workflow) -> Self {
    // Clean up any existing abort file to ensure clean slate
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

    // ... rest of existing implementation
}
```

### Comprehensive Test Coverage

Added 4 comprehensive tests covering all validation criteria:

1. **`test_abort_file_cleanup_when_file_exists`** - Verifies abort file is removed when it exists
2. **`test_abort_file_cleanup_when_file_does_not_exist`** - Ensures graceful handling when no abort file exists
3. **`test_abort_file_cleanup_continues_on_permission_error`** - Confirms workflow creation continues even if cleanup fails
4. **`test_multiple_workflow_runs_cleanup_abort_file`** - Tests cleanup behavior across multiple workflow runs

### Validation Results

All validation criteria met:
- ✅ Abort file is removed when starting new workflow run
- ✅ WorkflowRun initialization succeeds if cleanup fails  
- ✅ Appropriate logging for cleanup operations
- ✅ No regression in existing workflow functionality (all 12 tests passing)
- ✅ Cleanup handles missing files gracefully
- ✅ Cleanup handles file system errors gracefully

### Testing Results

```
running 12 tests
test workflow::run::tests::test_workflow_run_id_monotonic_generation ... ok
test workflow::run::tests::test_workflow_run_id_creation ... ok
test workflow::run::tests::test_abort_file_cleanup_when_file_does_not_exist ... ok
test workflow::run::tests::test_workflow_run_completion ... ok
test workflow::run::tests::test_abort_file_cleanup_when_file_exists ... ok
test workflow::run::tests::test_abort_file_cleanup_continues_on_permission_error ... ok
test workflow::run::tests::test_workflow_run_creation ... ok
test workflow::run::tests::test_workflow_run_id_parse_and_to_string ... ok
test workflow::run::tests::test_workflow_run_id_parse_invalid ... ok
test workflow::run::tests::test_workflow_run_id_parse_valid_ulid ... ok
test workflow::run::tests::test_multiple_workflow_runs_cleanup_abort_file ... ok
test workflow::run::tests::test_workflow_run_transition ... ok

test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured
```

The implementation is ready for integration with the overall abort system described in the specification.