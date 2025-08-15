The place where you error log.

Detected ABORT ERROR in prompt output, triggering immediate shutdown

REALLY -- I keep saying this -- hard exit with a non zero exit code. There is no reason to continue afterward.

REALLY immediate shutdown, DO NOT KEEP GOING when running.

I know this makes the tests tricky, so have conditional behavior when running in test.

## Proposed Solution

The issue is that when "ABORT ERROR" is detected in prompt output, the application should immediately terminate the process with a non-zero exit code instead of returning an error and continuing through the normal error handling flow.

**Current Problem:**
- ABORT ERROR detection exists but only returns error codes through normal flow
- Process continues execution until normal termination  
- User wants immediate shutdown with `std::process::exit()` call

**Implementation Steps:**

1. **Modify `abort_handler.rs`** to add immediate exit functionality:
   - Add a new function `immediate_exit_on_abort_error()` that calls `std::process::exit(2)`
   - Add conditional behavior using `#[cfg(not(test))]` for production vs test mode
   - In test mode, return error normally; in production, call `std::process::exit()`

2. **Update prompt processing in `test.rs`**:
   - Replace current ABORT ERROR check with call to immediate exit function
   - Ensure the exit happens before any further processing

3. **Update CLI error handlers**:
   - Replace ABORT ERROR checks in `main.rs` and `prompt.rs` with immediate exit calls
   - Ensure consistent behavior across all CLI entry points

4. **Test Adaptations**:
   - Use `#[cfg(test)]` to enable different behavior in test mode
   - Tests can continue to use normal error returns for assertions
   - Production code will use `std::process::exit(2)` for immediate shutdown

This approach ensures immediate termination in production while keeping tests functional.