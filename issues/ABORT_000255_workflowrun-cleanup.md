# ABORT_000255: WorkflowRun Cleanup Integration

Refer to ./specification/abort.md

## Objective

Add abort file cleanup logic to `WorkflowRun::new()` to ensure clean slate for each workflow execution. This prevents stale abort files from previous runs from affecting new workflow executions.

## Context

The specification requires that abort files are cleaned up when starting new workflows to prevent interference from previous aborted workflows. This step modifies the workflow system to remove any existing abort files at initialization.

## Current State Analysis

Location: `swissarmyhammer/src/workflow/run.rs:79-93`

The `WorkflowRun::new()` function currently initializes workflow state but doesn't clean up abort files from previous runs.

## Tasks

### 1. Add Abort File Cleanup to WorkflowRun::new

**Update: `swissarmyhammer/src/workflow/run.rs`**

Add cleanup logic to the `new()` function:

```rust
impl WorkflowRun {
    pub fn new(workflow: Workflow) -> Self {
        // Clean up any existing abort file from previous runs
        let abort_file_path = std::path::Path::new(".swissarmyhammer/.abort");
        if abort_file_path.exists() {
            if let Err(e) = std::fs::remove_file(abort_file_path) {
                tracing::warn!("Failed to clean up abort file: {}", e);
            } else {
                tracing::debug!("Cleaned up existing abort file");
            }
        }
        
        // ... rest of existing implementation
        Self {
            workflow,
            current_state: workflow.initial_state.clone(),
            variables: HashMap::new(),
            execution_history: Vec::new(),
            state_metrics: HashMap::new(),
        }
    }
}
```

### 2. Add Helper Function for Abort File Management

**Create: `swissarmyhammer/src/workflow/abort_utils.rs`**

```rust
//! Utilities for managing abort file state in workflows

use std::path::Path;
use tracing::{debug, warn};

/// Path to the abort file
pub const ABORT_FILE_PATH: &str = ".swissarmyhammer/.abort";

/// Clean up any existing abort file
/// 
/// This should be called at the start of workflow execution to ensure
/// a clean slate. Previous abort states should not affect new workflows.
pub fn cleanup_abort_file() {
    let abort_path = Path::new(ABORT_FILE_PATH);
    if abort_path.exists() {
        match std::fs::remove_file(abort_path) {
            Ok(()) => {
                debug!("Cleaned up existing abort file at {}", ABORT_FILE_PATH);
            }
            Err(e) => {
                warn!("Failed to clean up abort file at {}: {}", ABORT_FILE_PATH, e);
            }
        }
    }
}

/// Check if an abort file exists
pub fn abort_file_exists() -> bool {
    Path::new(ABORT_FILE_PATH).exists()
}

/// Read abort reason from file if it exists
pub fn read_abort_reason() -> Option<String> {
    let abort_path = Path::new(ABORT_FILE_PATH);
    if abort_path.exists() {
        match std::fs::read_to_string(abort_path) {
            Ok(reason) => Some(reason),
            Err(e) => {
                warn!("Failed to read abort file: {}", e);
                Some("Unknown abort reason".to_string())
            }
        }
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_cleanup_abort_file_exists() {
        let temp_dir = TempDir::new().unwrap();
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        // Create abort file
        fs::create_dir_all(".swissarmyhammer").unwrap();
        fs::write(".swissarmyhammer/.abort", "test reason").unwrap();
        
        assert!(abort_file_exists());
        cleanup_abort_file();
        assert!(!abort_file_exists());

        std::env::set_current_dir(original_dir).unwrap();
    }

    #[test]
    fn test_cleanup_abort_file_not_exists() {
        let temp_dir = TempDir::new().unwrap();
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        // Should not panic when file doesn't exist
        cleanup_abort_file();
        assert!(!abort_file_exists());

        std::env::set_current_dir(original_dir).unwrap();
    }

    #[test]
    fn test_read_abort_reason() {
        let temp_dir = TempDir::new().unwrap();
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        assert_eq!(read_abort_reason(), None);

        fs::create_dir_all(".swissarmyhammer").unwrap();
        fs::write(".swissarmyhammer/.abort", "test abort reason").unwrap();
        
        assert_eq!(read_abort_reason(), Some("test abort reason".to_string()));

        std::env::set_current_dir(original_dir).unwrap();
    }
}
```

### 3. Update WorkflowRun to Use Abort Utils

**Update: `swissarmyhammer/src/workflow/run.rs`**

Use the new utility function:

```rust
use crate::workflow::abort_utils::cleanup_abort_file;

impl WorkflowRun {
    pub fn new(workflow: Workflow) -> Self {
        // Clean up any existing abort file from previous runs
        cleanup_abort_file();
        
        // ... rest of existing implementation
        Self {
            workflow,
            current_state: workflow.initial_state.clone(),
            variables: HashMap::new(),
            execution_history: Vec::new(),
            state_metrics: HashMap::new(),
        }
    }
}
```

### 4. Update Module Structure

**Update: `swissarmyhammer/src/workflow/mod.rs`**

Add the new module:

```rust
pub mod abort_utils;
// ... existing modules
```

### 5. Add Integration Test

**Create: `swissarmyhammer/src/workflow/tests/abort_cleanup_tests.rs`**

```rust
#[cfg(test)]
mod tests {
    use crate::workflow::{Workflow, WorkflowRun};
    use crate::workflow::abort_utils::abort_file_exists;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_workflow_run_cleans_up_abort_file() {
        let temp_dir = TempDir::new().unwrap();
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        // Create a stale abort file
        fs::create_dir_all(".swissarmyhammer").unwrap();
        fs::write(".swissarmyhammer/.abort", "stale abort").unwrap();
        assert!(abort_file_exists());

        // Create workflow - should clean up abort file
        let workflow = create_test_workflow();
        let _run = WorkflowRun::new(workflow);
        
        assert!(!abort_file_exists());

        std::env::set_current_dir(original_dir).unwrap();
    }

    #[test] 
    fn test_workflow_run_handles_missing_abort_file() {
        let temp_dir = TempDir::new().unwrap();
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        // No abort file exists - should not panic
        let workflow = create_test_workflow();
        let _run = WorkflowRun::new(workflow);
        
        assert!(!abort_file_exists());

        std::env::set_current_dir(original_dir).unwrap();
    }

    fn create_test_workflow() -> Workflow {
        // Create minimal test workflow
        // Implementation depends on existing test utilities
        use crate::test_utils::create_test_workflow;
        create_test_workflow()
    }
}
```

### 6. Update Existing Tests

Ensure existing workflow tests continue to pass by verifying they don't depend on stale abort files. Review and update any tests that might be affected by the cleanup behavior.

## Success Criteria

- [ ] `WorkflowRun::new()` cleans up existing abort files
- [ ] Cleanup operation is logged appropriately (debug for success, warn for errors)
- [ ] Cleanup handles missing files gracefully (no errors/panics)
- [ ] New abort utility module provides reusable functions
- [ ] Comprehensive unit tests cover cleanup scenarios
- [ ] Integration tests verify cleanup works in workflow context
- [ ] Existing workflow tests continue to pass
- [ ] No performance impact on workflow initialization

## Testing

```bash
# Run workflow tests
cargo test workflow

# Run specific abort cleanup tests
cargo test abort_cleanup

# Run all tests to check for regressions
cargo test

# Verify compilation
cargo check
```

## Notes

- Uses `tracing::warn` for cleanup errors but doesn't fail workflow initialization
- Uses `tracing::debug` for successful cleanup to avoid noise in normal operation
- Cleanup is performed before any other workflow initialization
- Utility functions provide consistent abort file management across the codebase
- Error handling follows the "warn but continue" pattern for cleanup operations

## Next Steps

After completion, proceed to ABORT_000256 for executor integration with abort file detection.