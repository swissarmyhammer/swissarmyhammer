# ABORT_000256: Executor Integration with Abort File Detection

Refer to ./specification/abort.md

## Objective

Add file-based abort detection to `execute_state_with_limit` in the workflow executor core. This implements the main abort detection loop that checks for abort files before each workflow iteration.

## Context

The specification requires abort file checking in the main execution loop at `swissarmyhammer/src/workflow/executor/core.rs:215-250`. This step adds the core abort detection logic that will immediately terminate workflow execution when an abort file is detected.

## Current State Analysis

Location: `swissarmyhammer/src/workflow/executor/core.rs:215-250`

The `execute_state_with_limit` function currently contains the main workflow execution loop but doesn't check for abort conditions.

## Tasks

### 1. Add Abort File Check to Execution Loop

**Update: `swissarmyhammer/src/workflow/executor/core.rs`**

Import abort utilities and add abort checking:

```rust
use crate::workflow::abort_utils::{abort_file_exists, read_abort_reason};

impl Executor {
    pub async fn execute_state_with_limit(
        &mut self,
        run: &mut WorkflowRun,
        remaining_transitions: usize,
    ) -> ExecutorResult<()> {
        // ... existing validation logic ...

        loop {
            // Check for abort file before each iteration
            if abort_file_exists() {
                let reason = read_abort_reason()
                    .unwrap_or_else(|| "Unknown abort reason".to_string());
                tracing::info!("Workflow execution aborted: {}", reason);
                return Err(ExecutorError::Abort(reason));
            }

            // ... rest of existing loop logic ...
            
            // Existing transition and state logic continues here
            if let Some(next_state_id) = self.find_transition(run).await? {
                // ... existing transition logic ...
            } else {
                // ... existing completion logic ...
                break;
            }
        }

        Ok(())
    }
}
```

### 2. Add ExecutorError::Abort Variant

**Update: `swissarmyhammer/src/workflow/executor/mod.rs`** or the error module:

Add the new error variant:

```rust
#[derive(Debug, thiserror::Error)]
pub enum ExecutorError {
    // ... existing variants ...
    
    #[error("Workflow aborted: {0}")]
    Abort(String),
}
```

### 3. Add Abort Error Tests

**Create: `swissarmyhammer/src/workflow/executor/tests/abort_tests.rs`**

```rust
#[cfg(test)]
mod abort_tests {
    use super::*;
    use crate::workflow::abort_utils::cleanup_abort_file;
    use std::fs;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_executor_detects_abort_file() {
        let temp_dir = TempDir::new().unwrap();
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        // Create test workflow and executor
        let mut executor = create_test_executor();
        let mut workflow_run = create_test_workflow_run();

        // Create abort file during execution
        fs::create_dir_all(".swissarmyhammer").unwrap();
        fs::write(".swissarmyhammer/.abort", "Test abort reason").unwrap();

        // Execute should detect abort and return error
        let result = executor.execute_state_with_limit(&mut workflow_run, 10).await;
        
        assert!(result.is_err());
        match result.unwrap_err() {
            ExecutorError::Abort(reason) => {
                assert_eq!(reason, "Test abort reason");
            }
            other => panic!("Expected Abort error, got: {:?}", other),
        }

        std::env::set_current_dir(original_dir).unwrap();
    }

    #[tokio::test]
    async fn test_executor_continues_without_abort_file() {
        let temp_dir = TempDir::new().unwrap();
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        let mut executor = create_test_executor();
        let mut workflow_run = create_test_workflow_run();

        // Ensure no abort file exists
        cleanup_abort_file();

        // Execute should proceed normally
        let result = executor.execute_state_with_limit(&mut workflow_run, 1).await;
        
        // Should complete normally (not abort error)
        if let Err(e) = &result {
            if matches!(e, ExecutorError::Abort(_)) {
                panic!("Unexpected abort error when no abort file exists: {:?}", e);
            }
        }

        std::env::set_current_dir(original_dir).unwrap();
    }

    #[tokio::test]
    async fn test_abort_detection_handles_read_error() {
        let temp_dir = TempDir::new().unwrap();
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        let mut executor = create_test_executor();
        let mut workflow_run = create_test_workflow_run();

        // Create abort file that can't be read (empty file)
        fs::create_dir_all(".swissarmyhammer").unwrap();
        fs::write(".swissarmyhammer/.abort", "").unwrap();

        let result = executor.execute_state_with_limit(&mut workflow_run, 10).await;
        
        assert!(result.is_err());
        match result.unwrap_err() {
            ExecutorError::Abort(reason) => {
                // Should use empty string or default message
                assert!(!reason.is_empty());
            }
            other => panic!("Expected Abort error, got: {:?}", other),
        }

        std::env::set_current_dir(original_dir).unwrap();
    }

    // Helper functions for test setup
    fn create_test_executor() -> Executor {
        // Implementation depends on existing test utilities
        // This should create a minimal executor for testing
        Executor::default()
    }

    fn create_test_workflow_run() -> WorkflowRun {
        // Implementation depends on existing test utilities
        // This should create a minimal workflow run for testing
        use crate::test_utils::create_test_workflow;
        WorkflowRun::new(create_test_workflow())
    }
}
```

### 4. Update Module Structure

**Update: `swissarmyhammer/src/workflow/executor/mod.rs`**

Add the test module:

```rust
#[cfg(test)]
mod tests {
    // ... existing test modules ...
    pub mod abort_tests;
}
```

### 5. Add Integration Test for Complete Abort Flow

**Create: `swissarmyhammer/tests/abort_workflow_integration.rs`**

```rust
//! Integration tests for abort workflow functionality
//! Tests the complete flow from MCP tool to workflow termination

use std::fs;
use tempfile::TempDir;
use swissarmyhammer::workflow::{Workflow, WorkflowRun};
use swissarmyhammer::workflow::executor::{Executor, ExecutorError};
use swissarmyhammer_tools::mcp::tools::abort::create::abort_create_tool;
use serde_json::json;
use std::collections::HashMap;

#[tokio::test]
async fn test_complete_abort_flow() {
    let temp_dir = TempDir::new().unwrap();
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp_dir.path()).unwrap();

    // Create workflow and executor
    let workflow = create_long_running_test_workflow();
    let mut workflow_run = WorkflowRun::new(workflow);
    let mut executor = Executor::default();

    // Simulate abort tool being called during execution
    let mut args = HashMap::new();
    args.insert("reason".to_string(), json!("Integration test abort"));
    
    let abort_result = abort_create_tool(args).await;
    assert!(abort_result.is_ok());

    // Execute workflow - should detect abort
    let result = executor.execute_state_with_limit(&mut workflow_run, 100).await;
    
    assert!(result.is_err());
    match result.unwrap_err() {
        ExecutorError::Abort(reason) => {
            assert_eq!(reason, "Integration test abort");
        }
        other => panic!("Expected Abort error, got: {:?}", other),
    }

    std::env::set_current_dir(original_dir).unwrap();
}

fn create_long_running_test_workflow() -> Workflow {
    // Create a workflow that would normally run for multiple iterations
    // This ensures we can test abort detection during execution
    // Implementation depends on existing test utilities
    unimplemented!("Implement based on existing test patterns")
}
```

### 6. Performance Considerations

Add efficient abort checking that minimizes filesystem operations:

```rust
// Optional: Add caching to reduce filesystem calls
// Only check abort file periodically, not every loop iteration
use std::time::{Duration, Instant};

struct ExecutorState {
    last_abort_check: Instant,
    abort_check_interval: Duration,
}

impl Executor {
    fn should_check_abort(&mut self) -> bool {
        let now = Instant::now();
        if now.duration_since(self.state.last_abort_check) > self.state.abort_check_interval {
            self.state.last_abort_check = now;
            true
        } else {
            false
        }
    }
}
```

## Success Criteria

- [ ] `execute_state_with_limit` checks for abort file in main execution loop
- [ ] Abort file detection returns `ExecutorError::Abort` with reason
- [ ] `ExecutorError::Abort` variant is properly defined and integrated
- [ ] Abort detection handles file read errors gracefully
- [ ] Comprehensive unit tests cover abort detection scenarios
- [ ] Integration test verifies complete abort flow from MCP tool to executor
- [ ] Performance impact is minimal (abort check is efficient)
- [ ] Existing executor tests continue to pass

## Testing

```bash
# Run executor tests
cargo test executor

# Run specific abort tests
cargo test abort_tests

# Run integration tests  
cargo test abort_workflow_integration

# Run all tests to check for regressions
cargo test

# Performance test for abort checking overhead
cargo test --release executor_performance
```

## Notes

- Abort checking happens before each workflow iteration to ensure responsive termination
- Uses existing abort utility functions for consistency
- Error handling follows established executor patterns
- Logging provides visibility into abort detection
- Performance considerations may be added if abort checking becomes a bottleneck
- Abort detection is fail-safe: read errors don't prevent abort from triggering

## Next Steps

After completion, proceed to ABORT_000257 for CLI error handling updates to handle `ExecutorError::Abort`.