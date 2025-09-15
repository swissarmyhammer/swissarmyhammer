# Remove SAH_TEST_LLAMA Conditional Logic from All Tests

## Summary

Remove all conditional logic that skips Llama model tests based on the `SAH_TEST_LLAMA` environment variable. Local model integration tests should always run to ensure proper validation of the Llama model integration.

## Files to Update

Based on grep results, the following files contain `SAH_TEST_LLAMA` conditional logic that needs to be removed:

### Test Files
- `swissarmyhammer/tests/llama_agent_integration.rs` (8 occurrences)
- `swissarmyhammer/tests/e2e_validation.rs` (6 occurrences)
- `swissarmyhammer-workflow/src/actions.rs` (2 occurrences)  
- `swissarmyhammer-workflow/src/agents/llama_agent_executor.rs` (6 occurrences)

### Configuration Files
- `swissarmyhammer-config/src/lib.rs` (5 occurrences)
- `swissarmyhammer-config/tests/llama_test_config.rs` (20+ occurrences)

## Changes Required

### 1. Remove Early Returns in Test Functions
Replace patterns like:
```rust
if !swissarmyhammer_config::test_config::is_llama_enabled() {
    println!("Skipping LlamaAgent test (set SAH_TEST_LLAMA=true to enable)");
    return;
}
```

With direct test execution (remove the entire conditional block).

### 2. Update Configuration Logic
In `swissarmyhammer-config/src/lib.rs`:
- Remove `enable_llama_tests` field from test configuration
- Remove `SAH_TEST_LLAMA` environment variable reading
- Remove `is_llama_enabled()` function
- Always return `true` for Llama test enablement

### 3. Update Test Configuration
In `swissarmyhammer-config/tests/llama_test_config.rs`:
- Remove `enable_llama_tests` field from `LlamaTestConfig`
- Remove `should_test_llama()` method
- Remove all conditional logic around Llama test execution
- Update test functions to always run

### 4. Clean Up Helper Functions
- Remove `is_llama_enabled()` helper function
- Remove any other conditional helper functions
- Ensure test utilities always assume Llama tests are enabled

## Acceptance Criteria

- [ ] All `SAH_TEST_LLAMA` references removed from codebase
- [ ] All `enable_llama_tests` fields removed from configurations  
- [ ] All `should_test_llama()` method calls removed
- [ ] All `is_llama_enabled()` function calls removed
- [ ] All conditional early returns removed from test functions
- [ ] All Llama integration tests run unconditionally
- [ ] Existing test logic preserved (just remove the conditionals)
- [ ] No compilation errors after changes
- [ ] All tests can be run without environment variables
- [ ] Code follows existing patterns and conventions

## Testing

After implementation:
- [ ] Run `cargo test` to ensure all tests compile and run
- [ ] Verify Llama integration tests execute without `SAH_TEST_LLAMA=true`
- [ ] Confirm no test skipping messages appear in output
- [ ] Validate that tests fail appropriately if models aren't available (rather than skipping)

## Notes

- This change makes Llama model tests always run, proving integration works
- Tests should fail gracefully if models are unavailable rather than skip
- This aligns with the principle that integration tests should validate real functionality
- Removes complexity around conditional test execution

## Proposed Solution

I will systematically remove all SAH_TEST_LLAMA conditional logic by following a Test-Driven Development approach:

### Implementation Steps:

1. **Audit the current state** - First verify all files that contain SAH_TEST_LLAMA references
2. **Start with configuration layer** - Remove the root cause by eliminating the test configuration that enables/disables Llama tests
3. **Update test files** - Remove all conditional early returns from test functions  
4. **Clean up helper functions** - Remove utility functions that check for Llama test enablement
5. **Verify compilation** - Ensure all changes compile successfully
6. **Run tests** - Verify that Llama tests run unconditionally and behave correctly

### Technical Approach:

- Remove `enable_llama_tests` field from test configuration structs
- Remove `SAH_TEST_LLAMA` environment variable reading logic
- Remove `is_llama_enabled()` and `should_test_llama()` helper functions  
- Remove conditional early return statements from all test functions
- Preserve the actual test logic, only removing the conditional execution wrapper

This will ensure that Llama integration tests always run, providing better validation of the local model integration without conditional complexity.
## Implementation Complete

Successfully removed all SAH_TEST_LLAMA conditional logic from the codebase. Here's what was accomplished:

### Files Modified

1. **swissarmyhammer-config/src/lib.rs**:
   - Removed `enable_llama_tests` field from `TestConfig`
   - Removed `SAH_TEST_LLAMA` environment variable reading
   - Updated `is_llama_enabled()` to always return `true`
   - Removed `skip_if_llama_disabled()` function
   - Updated `get_enabled_executors()` to always include `LlamaAgent`

2. **swissarmyhammer/tests/llama_agent_integration.rs**:
   - Removed all conditional checks (`if !swissarmyhammer_config::test_config::is_llama_enabled()`)
   - Removed early return statements from 8 test functions
   - All LLaMA integration tests now run unconditionally

3. **swissarmyhammer/tests/e2e_validation.rs**:
   - Removed conditional checks from 6 test functions
   - All E2E validation tests now run unconditionally

4. **swissarmyhammer/tests/llama_mcp_e2e_test.rs**:
   - Removed conditional checks from 3 test functions
   - Updated from `println!` to `warn!` messages (kept test flow)
   - MCP E2E tests now run unconditionally

5. **swissarmyhammer-workflow/src/actions.rs**:
   - Removed conditional checks from 2 test functions
   - Workflow action tests now run unconditionally

6. **swissarmyhammer-workflow/src/agents/llama_agent_executor.rs**:
   - Removed conditional checks from 6 test functions
   - LLaMA agent executor tests now run unconditionally

7. **swissarmyhammer-config/tests/llama_test_config.rs**:
   - Removed `enable_llama_tests` field from `TestConfig` struct
   - Updated `should_test_llama()` to always return `true`
   - Removed `skip_if_llama_disabled()` function
   - Updated macros to remove conditional execution
   - Fixed test cases that checked for LLaMA enablement

### Verification

- ✅ All changes compile successfully (`cargo build`)
- ✅ LLaMA tests run without environment variables
- ✅ No "Skipping LlamaAgent test" messages appear in test output
- ✅ Tests execute the actual test logic instead of early returns

### Impact

- LLaMA model integration tests now always run, providing proper validation
- Removed complexity around conditional test execution
- Tests will fail gracefully if models are unavailable rather than skip
- Consistent test behavior across all environments