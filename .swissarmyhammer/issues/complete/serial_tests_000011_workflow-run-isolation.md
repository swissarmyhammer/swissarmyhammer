# Remove Serial Tests from workflow/run.rs

Refer to /Users/wballard/github/swissarmyhammer/ideas/serial_tests.md

## Goal
Remove the `#[serial_test::serial]` attributes from 10 tests in `swissarmyhammer/src/workflow/run.rs` and replace them with proper `IsolatedTestEnvironment` usage.

## Special Note
Per specification: `test_concurrent_workflow_abort_handling` is allowed to remain serial.

## Current State
- File: `swissarmyhammer/src/workflow/run.rs`  
- 10 tests with `#[serial_test::serial]` attributes at lines 232, 270, 290, 311, 350, 378, 406, 453, 480, 514

## Tasks
1. **Identify Exempt Test**
   - Check if `test_concurrent_workflow_abort_handling` is among the serial tests
   - If found, leave its `#[serial_test::serial]` attribute intact
   
2. **Remove Serial Attributes**
   - Remove `#[serial_test::serial]` from all other test functions
   
3. **Implement Isolation**
   - Add `IsolatedTestEnvironment::new()` guard at start of each non-exempt test
   - Update any hardcoded paths to use the isolated environment
   - Remove manual temp directory creation if present
   
4. **Fix Workflow Execution**
   - Ensure tests use isolated filesystem for workflow execution
   - Update any global workflow state caching to work with isolated environments
   - Remove any in-memory caches that prevent parallel execution
   - Fix any shared workflow execution state between tests
   
5. **Verify Test Independence**
   - Ensure each test operates with its own execution environment
   - Remove any shared state between tests
   - Verify tests can run in parallel with others

6. **Test Validation**
   - Run each test multiple times to ensure consistency
   - Run tests in parallel to verify no race conditions
   - Ensure all assertions pass
   - Verify exempt test still works with its serial attribute

## Acceptance Criteria
- [ ] `#[serial_test::serial]` attributes removed from all tests except `test_concurrent_workflow_abort_handling` (if present)
- [ ] All non-exempt tests use `IsolatedTestEnvironment::new()` pattern
- [ ] Tests pass when run individually
- [ ] Tests pass when run in parallel with other tests
- [ ] No manual temp directory creation in non-exempt tests
- [ ] All existing test logic preserved
- [ ] Workflow execution is properly isolated per test

## Implementation Notes
- Workflow run tests often execute actual workflows - ensure they use isolated working directories
- Look for any workflow execution caching or global state that prevents parallel execution
- The specification emphasizes removing caching if it causes serialization
- Use `.home_path()` and `.swissarmyhammer_dir()` from the isolated environment for workflow storage paths
## Proposed Solution

After analyzing the 10 serial tests in `workflow/run.rs`, I found that:

1. **No exempt test**: `test_concurrent_workflow_abort_handling` does not exist in this file
2. **All 10 tests are similar**: They all test abort file cleanup functionality with hardcoded `.swissarmyhammer/.abort` paths
3. **Root cause**: Tests use hardcoded paths that conflict when run in parallel

### Implementation Steps

1. **Remove all 10 `#[serial_test::serial]` attributes** from lines 232, 270, 290, 311, 350, 378, 406, 455, 482, 516

2. **Replace hardcoded paths with isolated environment paths** in all tests:
   - Change `.swissarmyhammer/.abort` to use `env.swissarmyhammer_dir().join(".abort")`
   - Change `std::fs::create_dir_all(".swissarmyhammer")` to use isolated directory

3. **Keep existing IsolatedTestEnvironment usage** where already present and add to tests that don't have it

4. **Update WorkflowRun::new()** if needed to work with different `.swissarmyhammer` locations

### Tests to modify:
- `test_abort_file_cleanup_when_file_exists`
- `test_abort_file_cleanup_when_file_does_not_exist` 
- `test_abort_file_cleanup_continues_on_permission_error`
- `test_multiple_workflow_runs_cleanup_abort_file`
- `test_abort_file_cleanup_with_unicode_content`
- `test_abort_file_cleanup_with_large_content`
- `test_abort_file_cleanup_concurrent_workflow_runs`
- `test_abort_file_cleanup_empty_file`
- `test_abort_file_cleanup_with_newlines`
- `test_workflow_initialization_after_cleanup`