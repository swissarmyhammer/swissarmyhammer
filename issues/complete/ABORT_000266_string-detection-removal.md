# Remove All String-Based "ABORT ERROR" Detection Code

Refer to ./specification/abort.md

## Objective
Remove all remaining string-based "ABORT ERROR" detection code from the codebase, clean up obsolete modules and functions, and ensure the system fully relies on the new file-based abort mechanism.

## Context
With the new file-based abort system fully implemented and tested, the old string-based "ABORT ERROR" detection code can be safely removed. This cleanup ensures there's no confusion between old and new systems and removes technical debt.

## Tasks

### 1. Remove String-Based Detection in Workflow System
Location: `swissarmyhammer/src/workflow/executor/core.rs:527-538`
- Remove ActionError::AbortError handling that checks for strings
- Clean up any remaining string-based abort logic
- Ensure workflow system only uses file-based detection

### 2. Remove Common Abort Handler Module
Location: `swissarmyhammer/src/common/abort_handler.rs`
- Remove entire abort handler module if it exists
- Update module imports that reference this module
- Clean up any exports or re-exports

### 3. Remove Action-Level String Detection
Location: `swissarmyhammer/src/workflow/actions.rs`
- Remove any string-based "ABORT ERROR" detection in actions
- Clean up action error handling that looks for abort strings
- Ensure actions work with new abort system

### 4. Remove Git Module String Detection  
Location: `swissarmyhammer/src/git.rs`
- Remove any abort string detection in git operations
- Clean up git error handling related to abort patterns
- Ensure git operations work with file-based abort

### 5. Update Error Module
Location: `swissarmyhammer/src/error.rs`
- Remove any string-based abort detection constants or functions
- Clean up error types that are no longer needed
- Update error context handling

### 6. Search and Remove All Remaining References
Use comprehensive search to find and remove:
- All remaining "ABORT ERROR" string constants
- Functions that check for abort error strings
- Comments referring to old string-based system
- Dead code related to string-based detection

## Implementation Details

### Search Strategy
```bash
# Find all remaining string-based references
grep -r "ABORT ERROR" --include="*.rs" .
grep -r "is_abort_error" --include="*.rs" .
grep -r "abort_error" --include="*.rs" .
```

### Code Removal Pattern
- Remove string constant definitions
- Remove string matching logic in error handlers
- Remove helper functions for string-based detection
- Update error handling to use only ExecutorError::Abort

### Module Cleanup
```rust
// Remove patterns like:
const ABORT_ERROR_PREFIX: &str = "ABORT ERROR";

fn is_abort_error(error_message: &str) -> bool {
    error_message.contains("ABORT ERROR")
}

// Replace with file-based detection already implemented
```

## Validation Criteria
- [ ] No "ABORT ERROR" string constants remain in codebase
- [ ] No functions checking for abort error strings exist
- [ ] All string-based detection logic is removed
- [ ] Obsolete modules are completely removed
- [ ] No dead code related to old abort system remains
- [ ] Code compiles without errors after removal
- [ ] All tests still pass with removals

## Testing Requirements
- Run full test suite to ensure no regressions
- Verify that abort functionality still works end-to-end
- Check that no code depends on removed functionality
- Validate that error handling still works correctly

## Files to Modify
Based on specification analysis:
- `swissarmyhammer/src/workflow/executor/core.rs`
- `swissarmyhammer/src/workflow/actions.rs`
- `swissarmyhammer/src/common/abort_handler.rs` (remove entirely)
- `swissarmyhammer/src/git.rs`
- `swissarmyhammer/src/error.rs`
- Any other files found through comprehensive search

## Dependencies
- ABORT_000265_comprehensive-testing (testing must be complete to ensure safe removal)
- All previous abort implementation issues must be complete

## Follow-up Issues
- ABORT_000267_test-suite-updates

## Proposed Solution

Based on my analysis of the codebase, I'll systematically remove all string-based "ABORT ERROR" detection code in the following order:

### 1. **Remove Common Abort Handler Module**
- **File**: `swissarmyhammer/src/common/abort_handler.rs`
- **Action**: Remove entire module since it's dedicated to string-based detection
- **Impact**: Update module imports that reference this module

### 2. **Remove Error Module String Constants and Functions**
- **File**: `swissarmyhammer/src/error.rs`
- **Action**: Remove:
  - `ABORT_ERROR_PREFIX` constant
  - `CANNOT_SWITCH_ISSUE_TO_ISSUE` constant  
  - `CANNOT_CREATE_ISSUE_FROM_ISSUE` constant
  - `is_abort_error()` method
  - `extract_abort_message()` method
  - `cannot_switch_issue_to_issue()` method
  - `cannot_create_issue_from_issue()` method
- **Keep**: Other error handling functionality not related to string detection

### 3. **Remove Git Module String Detection**
- **File**: `swissarmyhammer/src/git.rs`
- **Action**: Replace hardcoded "ABORT ERROR:" strings with regular error handling
- **Impact**: Use standard error propagation instead of string-based detection

### 4. **Remove Workflow Actions String Detection** 
- **File**: `swissarmyhammer/src/workflow/actions.rs`
- **Action**: Remove:
  - `ActionError::AbortError` variant with ABORT ERROR string formatting
  - String checking logic for "ABORT ERROR:" prefixes
  - Replace with file-based detection already implemented

### 5. **Remove Workflow Executor String Detection**
- **File**: `swissarmyhammer/src/workflow/executor/core.rs`
- **Action**: Remove ActionError::AbortError formatting that adds "ABORT ERROR:" prefix
- **Impact**: The file-based detection is already implemented, just need to remove string formatting

### 6. **Update All Test Files**
- **Files**: Remove or update tests that expect string-based detection:
  - `swissarmyhammer/tests/abort_error_integration_tests.rs`
  - `swissarmyhammer/tests/abort_error_pattern_tests.rs` 
  - `swissarmyhammer-cli/tests/abort_error_cli_test.rs`
  - `swissarmyhammer-cli/tests/cli_mcp_integration_test.rs`
  - `swissarmyhammer/src/workflow/actions_tests/prompt_action_tests.rs`
- **Action**: Either remove obsolete tests or update to test file-based abort functionality

### 7. **Clean Up CLI Test Reference**
- **File**: `swissarmyhammer-cli/src/test.rs`
- **Action**: Remove comment referencing old ABORT ERROR detection

### Implementation Strategy
1. Remove modules and constants first (clean compile errors)
2. Update actual detection logic to use file-based system
3. Update tests to validate new system
4. Run comprehensive test suite to ensure no regressions

This approach ensures we completely remove the old string-based system while preserving all abort functionality through the new file-based mechanism that's already implemented and tested.