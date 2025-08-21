# Remove Serial Test from test_utils.rs

Refer to /Users/wballard/github/swissarmyhammer/ideas/serial_tests.md

## Goal
Remove the `#[serial_test::serial]` attribute from the test in `swissarmyhammer/src/test_utils.rs` and replace it with proper `IsolatedTestEnvironment` usage.

## Current State
- File: `swissarmyhammer/src/test_utils.rs`
- 1 test with `#[serial_test::serial]` attribute at line 636

## Tasks
1. **Remove Serial Attribute**
   - Remove `#[serial_test::serial]` from the test function
   
2. **Implement Isolation**
   - Add `IsolatedTestEnvironment::new()` guard at start of test
   - Update any hardcoded paths to use the isolated environment
   - Remove manual temp directory creation if present
   
3. **Verify Test Independence**
   - Ensure test uses isolated HOME/PWD from the environment
   - Remove any global state modifications
   - Verify test can run in parallel with others

4. **Test Validation**
   - Run the specific test multiple times to ensure consistency
   - Run tests in parallel to verify no race conditions
   - Ensure all assertions pass

## Acceptance Criteria
- [ ] `#[serial_test::serial]` attribute removed
- [ ] Test uses `IsolatedTestEnvironment::new()` pattern
- [ ] Test passes when run individually
- [ ] Test passes when run in parallel with other tests
- [ ] No manual temp directory creation
- [ ] All existing test logic preserved

## Implementation Notes
- Follow the pattern established in other tests that use `IsolatedTestEnvironment`
- The guard should be named `_guard` to indicate it's kept alive for RAII cleanup
- Use `.home_path()` or `.swissarmyhammer_dir()` methods for accessing paths within the isolated environment