# ABORT_000257: CLI Error Handling Updates

Refer to ./specification/abort.md

## Objective

Update CLI error handling to detect `ExecutorError::Abort` and remove string-based "ABORT ERROR" detection. This step modernizes the CLI to use the new file-based abort system while maintaining proper exit codes.

## Context

The specification identifies multiple locations in the CLI that currently check for "ABORT ERROR" strings. These need to be updated to handle the new `ExecutorError::Abort` variant while removing brittle string-based detection.

## Current String-Based Detection Locations

From the specification analysis:
- `swissarmyhammer-cli/src/main.rs:279` - Main CLI exit handling
- `swissarmyhammer-cli/src/prompt.rs:42` - Prompt execution error handling  
- `swissarmyhammer-cli/src/test.rs:280-284` - Test abort detection
- `swissarmyhammer-cli/src/error.rs:32-36` - Error helper function

## Tasks

### 1. Update Main CLI Error Handling

**Update: `swissarmyhammer-cli/src/main.rs`**

Replace string-based detection with proper error matching:

```rust
use swissarmyhammer::workflow::executor::ExecutorError;

// Remove old string-based detection around line 279
// Replace with proper error handling
async fn run() -> Result<(), Box<dyn std::error::Error>> {
    // ... existing CLI setup ...

    match result {
        Ok(_) => {
            tracing::info!("Command completed successfully");
            std::process::exit(EXIT_SUCCESS);
        }
        Err(error) => {
            // Check for abort error specifically
            if let Some(executor_error) = error.downcast_ref::<ExecutorError>() {
                if let ExecutorError::Abort(reason) = executor_error {
                    tracing::error!("Workflow aborted: {}", reason);
                    eprintln!("Aborted: {}", reason);
                    std::process::exit(EXIT_ERROR);
                }
            }

            // Handle other errors normally
            tracing::error!("Command failed: {}", error);
            eprintln!("Error: {}", error);
            std::process::exit(EXIT_ERROR);
        }
    }
}
```

### 2. Update Prompt Command Error Handling

**Update: `swissarmyhammer-cli/src/prompt.rs`**

Remove string-based abort detection around line 42:

```rust
use swissarmyhammer::workflow::executor::ExecutorError;

pub async fn run_prompt(command: PromptCommands) -> Result<(), Box<dyn std::error::Error>> {
    match command {
        PromptCommands::Render { /* ... */ } => {
            // ... existing rendering logic ...
            
            match workflow_result {
                Ok(output) => {
                    println!("{}", output);
                    Ok(())
                }
                Err(error) => {
                    // Handle abort error specifically
                    if let Some(ExecutorError::Abort(reason)) = error.downcast_ref::<ExecutorError>() {
                        tracing::error!("Prompt execution aborted: {}", reason);
                        return Err(format!("Aborted: {}", reason).into());
                    }

                    // Remove old string-based check:
                    // if error_msg.contains("ABORT ERROR") { ... }
                    
                    tracing::error!("Prompt execution failed: {}", error);
                    Err(error)
                }
            }
        }
        // ... other commands ...
    }
}
```

### 3. Update Test Command Error Handling

**Update: `swissarmyhammer-cli/src/test.rs`**

Replace string-based detection around lines 280-284:

```rust
use swissarmyhammer::workflow::executor::ExecutorError;

pub async fn run_test(command: TestCommands) -> Result<(), Box<dyn std::error::Error>> {
    match command {
        TestCommands::Validate { /* ... */ } => {
            // ... existing test logic ...
            
            match test_result {
                Ok(_) => {
                    println!("✅ Test passed");
                    Ok(())
                }
                Err(error) => {
                    // Handle abort error specifically
                    if let Some(ExecutorError::Abort(reason)) = error.downcast_ref::<ExecutorError>() {
                        println!("⚠️ Test aborted: {}", reason);
                        return Ok(()); // Tests can be aborted without failing
                    }

                    // Remove old string-based detection:
                    // if error.to_string().contains("ABORT ERROR") {
                    //     println!("⚠️ Test aborted");
                    //     return Ok(());
                    // }
                    
                    println!("❌ Test failed: {}", error);
                    Err(error)
                }
            }
        }
        // ... other test commands ...
    }
}
```

### 4. Remove Error Helper Function

**Update: `swissarmyhammer-cli/src/error.rs`**

Remove the `is_abort_error` function around lines 32-36:

```rust
// Remove this function entirely:
// pub fn is_abort_error(error_msg: &str) -> bool {
//     error_msg.contains("ABORT ERROR")
// }

// If there are other utilities in this file, keep them
// Otherwise, this entire module might be removable
```

### 5. Add Error Handling Utilities

**Create: `swissarmyhammer-cli/src/error_handling.rs`**

Add modern error handling utilities:

```rust
//! Modern error handling utilities for CLI

use swissarmyhammer::workflow::executor::ExecutorError;
use std::error::Error;

/// Check if an error chain contains an abort error
pub fn extract_abort_reason(error: &dyn Error) -> Option<String> {
    // Check if the error itself is an abort error
    if let Some(ExecutorError::Abort(reason)) = error.downcast_ref::<ExecutorError>() {
        return Some(reason.clone());
    }

    // Check error chain for abort errors
    let mut source = error.source();
    while let Some(err) = source {
        if let Some(ExecutorError::Abort(reason)) = err.downcast_ref::<ExecutorError>() {
            return Some(reason.clone());
        }
        source = err.source();
    }

    None
}

/// Handle workflow execution errors with proper abort detection
pub fn handle_workflow_error(error: &dyn Error) -> i32 {
    if let Some(abort_reason) = extract_abort_reason(error) {
        tracing::error!("Workflow aborted: {}", abort_reason);
        eprintln!("Aborted: {}", abort_reason);
        crate::exit_codes::EXIT_ERROR
    } else {
        tracing::error!("Workflow failed: {}", error);
        eprintln!("Error: {}", error);
        crate::exit_codes::EXIT_ERROR
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use swissarmyhammer::workflow::executor::ExecutorError;

    #[test]
    fn test_extract_abort_reason_direct() {
        let abort_error = ExecutorError::Abort("Test abort reason".to_string());
        let reason = extract_abort_reason(&abort_error);
        assert_eq!(reason, Some("Test abort reason".to_string()));
    }

    #[test]
    fn test_extract_abort_reason_none() {
        let other_error = std::io::Error::new(std::io::ErrorKind::Other, "Not an abort");
        let reason = extract_abort_reason(&other_error);
        assert_eq!(reason, None);
    }

    #[test]
    fn test_handle_workflow_error_abort() {
        let abort_error = ExecutorError::Abort("Test abort".to_string());
        let exit_code = handle_workflow_error(&abort_error);
        assert_eq!(exit_code, crate::exit_codes::EXIT_ERROR);
    }
}
```

### 6. Update Module Structure

**Update: `swissarmyhammer-cli/src/lib.rs`** (or main.rs if using binary crate structure):

```rust
pub mod error_handling;
// Remove old error module if it only contained is_abort_error
// pub mod error; // Remove if empty
```

### 7. Add Integration Tests

**Update: `swissarmyhammer-cli/tests/abort_error_cli_test.rs`**

Replace string-based tests with error-based tests:

```rust
use swissarmyhammer_cli::error_handling::extract_abort_reason;
use swissarmyhammer::workflow::executor::ExecutorError;

#[test]
fn test_cli_abort_error_detection() {
    let abort_error = ExecutorError::Abort("CLI test abort".to_string());
    let reason = extract_abort_reason(&abort_error);
    assert_eq!(reason, Some("CLI test abort".to_string()));
}

#[tokio::test]
async fn test_cli_handles_abort_error() {
    // Create a test that triggers an abort through the complete flow
    // This replaces the old string-based integration tests
    
    let temp_dir = tempfile::TempDir::new().unwrap();
    std::env::set_current_dir(temp_dir.path()).unwrap();

    // Create abort file
    std::fs::create_dir_all(".swissarmyhammer").unwrap();
    std::fs::write(".swissarmyhammer/.abort", "Integration abort test").unwrap();

    // Run CLI command that would trigger workflow execution
    // This should detect the abort and handle it properly
    // Implementation depends on existing CLI test patterns
}

// Remove old string-based tests like:
// #[test] 
// fn test_abort_error_string_detection() {
//     assert!(is_abort_error("Some ABORT ERROR message"));
// }
```

## Success Criteria

- [ ] All string-based "ABORT ERROR" detection removed from CLI code
- [ ] CLI properly handles `ExecutorError::Abort` with structured error matching
- [ ] Proper exit codes maintained (EXIT_ERROR for abort conditions)
- [ ] Error messages are clear and user-friendly
- [ ] Error handling utilities provide reusable abort detection
- [ ] Integration tests verify end-to-end abort handling
- [ ] No regression in existing CLI error handling
- [ ] Logging provides appropriate visibility into abort conditions

## Testing

```bash
# Run CLI tests
cargo test --package swissarmyhammer-cli

# Run specific abort error tests
cargo test abort_error

# Run integration tests
cargo test cli_integration

# Test complete CLI with abort scenarios
cargo test e2e_abort

# Verify no regressions
cargo test
```

## Notes

- Uses proper error downcasting instead of string matching for reliability
- Maintains existing exit codes to preserve CLI behavior contracts
- Error handling utilities support error chain traversal for nested errors
- Integration tests replace string-based tests with actual error objects
- Logging maintains visibility while using structured error handling
- Error messages remain user-friendly while being more robust

## Next Steps

After completion, proceed to ABORT_000258 for built-in prompt updates to use the new abort tool.