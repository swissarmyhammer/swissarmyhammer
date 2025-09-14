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