# MCP Abort Tool Specification

## Overview

Replace the brittle string-based `ABORT ERROR` detection system with a robust MCP tool that allows controlled termination of prompts, workflows, and actions.

## Current State Analysis

### String-Based `ABORT ERROR` Detection Locations

The following locations currently check for "ABORT ERROR" strings and must be updated:

**CLI Error Detection:**
- `swissarmyhammer-cli/src/main.rs:279` - Main CLI exit handling
- `swissarmyhammer-cli/src/prompt.rs:42` - Prompt execution error handling  
- `swissarmyhammer-cli/src/test.rs:280-284` - Test abort detection
- `swissarmyhammer-cli/src/error.rs:32-36` - Error helper function

**Workflow System:**
- `swissarmyhammer/src/workflow/executor/core.rs:527-538` - ActionError::AbortError handling
- Documentation references in `doc/src/workflows.md` and other files

**Test Files:**
- `swissarmyhammer-cli/tests/abort_error_cli_test.rs` - Comprehensive abort testing
- `swissarmyhammer-cli/tests/cli_mcp_integration_test.rs:278-282` - MCP integration tests

**Built-in Prompts:**
- `builtin/prompts/abort.md:9` - Contains `"Respond only with ABORT ERROR"`

## Proposed Solution

### 1. New MCP Tool: `abort`

**Parameters:**
- `reason` (required): String containing the abort reason/message

**Behavior:**
- Creates `.swissarmyhammer/.abort` file with the reason text
- Tool returns success (MCP tools should not fail to allow proper error propagation)
- File-based approach ensures abort state persists across process boundaries

**Example Usage:**
```json
{
  "tool": "abort",
  "parameters": {
    "reason": "User cancelled the destructive operation"
  }
}
```

### 2. File-Based Abort Detection

**Abort File Location:** `.swissarmyhammer/.abort`

**File Contents:** Plain text containing the abort reason

**Advantages:**
- Robust across process boundaries
- Language/framework agnostic
- Simple to implement and test
- Atomic operation (file creation is atomic)

### 3. Integration Points

#### 3.1 WorkflowRun::new Cleanup

Location: `swissarmyhammer/src/workflow/run.rs:79-93`

Add cleanup logic to `WorkflowRun::new()`:
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

#### 3.2 execute_state_with_limit Integration

Location: `swissarmyhammer/src/workflow/executor/core.rs:215-250`

Add abort file check in the main execution loop:
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

#### 3.3 New ExecutorError::Abort Variant

Add new error type to handle abort conditions:
```rust
#[derive(Debug, thiserror::Error)]
pub enum ExecutorError {
    // ... existing variants ...
    
    #[error("Workflow aborted: {0}")]
    Abort(String),
}
```

### 4. Remove String-Based Detection

#### 4.1 Files to Modify

**Remove or Update String Checks:**
- `swissarmyhammer-cli/src/main.rs:279` - Remove `error_msg.contains("ABORT ERROR")`
- `swissarmyhammer-cli/src/prompt.rs:42` - Remove `error_msg.contains("ABORT ERROR")`
- `swissarmyhammer-cli/src/test.rs:280-284` - Remove abort error string check
- `swissarmyhammer-cli/src/error.rs:32-36` - Remove `is_abort_error` function
- `swissarmyhammer/src/common/abort_handler.rs` - Remove entire module if exists

**Update Built-in Prompts:**
- `builtin/prompts/abort.md` - Replace with instructions to use `abort` tool

#### 4.2 Test Updates Required

**Test Files to Update:**
- `swissarmyhammer-cli/tests/abort_error_cli_test.rs` - Update to test file-based abort
- `swissarmyhammer-cli/tests/cli_mcp_integration_test.rs` - Remove string-based tests

### 5. Error Propagation Strategy

#### 5.1 CLI Level
Replace string checking with `ExecutorError::Abort` handling:
```rust
match workflow_result {
    Err(ExecutorError::Abort(reason)) => {
        tracing::error!("Workflow aborted: {}", reason);
        std::process::exit(EXIT_ERROR);
    }
    // ... handle other errors
}
```

#### 5.2 Workflow Level  
The abort file check in `execute_state_with_limit` ensures immediate termination when abort is requested.

### 6. Documentation Updates

**Files to Update:**
- `doc/src/workflows.md` - Update abort error handling section
- `doc/book/print.html` - Regenerate with updated content
- `.swissarmyhammer/memos/Error Handling and Resilience Patterns.md` - Update pattern documentation

**New Documentation:**
- Add MCP tool documentation for `abort`
- Update workflow error handling examples

### 7. Migration Strategy

#### Phase 1: Add New System
1. Implement `abort` MCP tool
2. Add file-based detection in `execute_state_with_limit`
3. Add `ExecutorError::Abort` variant
4. Update `WorkflowRun::new` cleanup

#### Phase 2: Update Usage
1. Update built-in prompts to use `abort` tool
2. Update documentation and examples

#### Phase 3: Remove Old System  
1. Remove all string-based detection code
2. Update tests to use new system
3. Clean up obsolete modules

### 8. Testing Requirements

#### 8.1 Unit Tests
- Test `abort` tool file creation
- Test abort file detection in workflow executor
- Test cleanup in `WorkflowRun::new`

#### 8.2 Integration Tests
- Test end-to-end abort flow from prompt to CLI exit
- Test abort in nested workflows
- Test abort file cleanup between runs

#### 8.3 Regression Tests
- Ensure existing abort behavior is preserved
- Verify proper exit codes are maintained

### 9. Benefits of New Approach

1. **Reliability:** File-based detection is more robust than string parsing
2. **Testability:** Easy to test by creating/checking files
3. **Maintainability:** Single source of truth for abort state
4. **Extensibility:** Can easily add abort metadata in the future
5. **Cross-Process:** Works across different processes and languages
6. **Atomic:** File operations provide natural atomicity

### 10. Implementation Notes

- Use `.swissarmyhammer/` directory for consistency with existing patterns
- Ensure abort file is cleaned up on successful workflow completion
- Add appropriate logging for abort operations
- Consider adding abort timestamp to file contents for debugging
- Ensure proper error handling if abort file cannot be created/read

### 11. Backward Compatibility

During transition period:
- Keep existing string-based detection as fallback
- Add deprecation warnings for old approach
- Provide migration guide for custom prompts/workflows

This specification provides a complete roadmap for replacing the brittle string-based abort detection with a robust file-based MCP tool approach.