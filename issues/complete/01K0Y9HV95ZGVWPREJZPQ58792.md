If claude reponds with:

Failed: Claude command failed: Claude execution failed

turn this into an Abort Error and abort the current run.

## Proposed Solution

Based on my exploration of the codebase, I found that SwissArmyHammer already has a robust ABORT ERROR system in place:

1. **Existing System**: The `crate::common::abort_handler::check_for_abort_error` function checks Claude responses for "ABORT ERROR" patterns and converts them to `ActionError::AbortError`
2. **Current Integration**: This function is already called in the Claude command execution flow in `workflow/actions.rs:622`
3. **Error Propagation**: When an `ActionError::AbortError` is detected, it's properly propagated up through the workflow executor and causes immediate termination

**Implementation Plan**:
1. Enhance the `check_for_abort_error` function in `swissarmyhammer/src/common/abort_handler.rs` to detect the specific Claude failure pattern: "Failed: Claude command failed: Claude execution failed"
2. When this pattern is detected, convert it to an ABORT ERROR with appropriate context
3. Add comprehensive tests to verify the pattern detection works correctly
4. Verify the existing error propagation continues to work as expected

**Benefits**:
- Leverages existing, well-tested ABORT ERROR infrastructure
- Maintains consistency with other abort error handling
- Requires minimal code changes
- Preserves existing error logging and context extraction