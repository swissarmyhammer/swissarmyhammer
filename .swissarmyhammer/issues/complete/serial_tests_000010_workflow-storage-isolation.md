# Remove Serial Tests from workflow/storage.rs

Refer to /Users/wballard/github/swissarmyhammer/ideas/serial_tests.md

## Goal
Remove the `#[serial_test::serial]` attributes from 3 tests in `swissarmyhammer/src/workflow/storage.rs` and replace them with proper `IsolatedTestEnvironment` usage.

## Current State
- File: `swissarmyhammer/src/workflow/storage.rs`
- 3 tests with `#[serial_test::serial]` attributes at lines 1106, 1154, and 1225

## Tasks
1. **Remove Serial Attributes**
   - Remove `#[serial_test::serial]` from all 3 test functions
   
2. **Implement Isolation**
   - Add `IsolatedTestEnvironment::new()` guard at start of each test
   - Update any hardcoded paths to use the isolated environment
   - Remove manual temp directory creation if present
   
3. **Fix Storage Access**
   - Ensure tests use isolated filesystem for workflow storage
   - Update any global storage caching to work with isolated environments  
   - Remove any in-memory caches that prevent parallel execution
   - Fix any shared workflow storage state between tests
   
4. **Verify Test Independence**
   - Ensure each test operates on its own storage files/directories
   - Remove any shared state between tests
   - Verify tests can run in parallel with others

5. **Test Validation**
   - Run each test multiple times to ensure consistency
   - Run tests in parallel to verify no race conditions
   - Ensure all assertions pass

## Acceptance Criteria
- [ ] `#[serial_test::serial]` attributes removed from all 3 tests
- [ ] All tests use `IsolatedTestEnvironment::new()` pattern
- [ ] Tests pass when run individually
- [ ] Tests pass when run in parallel with other tests
- [ ] No manual temp directory creation
- [ ] All existing test logic preserved
- [ ] Workflow storage is properly isolated per test

## Implementation Notes
- Storage tests often access `.swissarmyhammer` directories - use the isolated environment's `.swissarmyhammer_dir()` method
- Look for any workflow caching or storage backends that maintain global state
- Ensure each test gets its own isolated storage backend instance
- The specification emphasizes removing caching that prevents parallel execution
## Implementation Completed

✅ **All Tasks Completed Successfully**

### Changes Made:
1. **Converted `test_workflow_resolver_local_workflows`** to use `IsolatedTestEnvironment`:
   - Replaced `tempfile::TempDir` with `IsolatedTestEnvironment::new()`
   - Removed manual directory changing logic (`set_current_dir`, `DirGuard`)
   - Simplified the test by leveraging the isolated environment's automatic directory management

### Verification Results:
- ✅ All tests pass when run individually
- ✅ All tests pass when run in parallel (`--jobs 4`)  
- ✅ Tests are stable across multiple runs (`--retries 5`)
- ✅ Code builds without warnings
- ✅ Code passes `cargo fmt --check`
- ✅ Code passes `cargo clippy` with no warnings

### Current State:
- **Serial attributes**: Already removed from all tests ✅
- **Isolation pattern**: All 3 mentioned tests now use `IsolatedTestEnvironment` ✅
  - `test_workflow_resolver_user_workflows` (line ~1106) 
  - `test_workflow_resolver_local_workflows` (line ~1154) - **FIXED**
  - `test_workflow_resolver_precedence` (line ~1225)

### Technical Details:
The key change was replacing the manual temp directory and working directory management with the `IsolatedTestEnvironment` pattern, which provides:
- Isolated filesystem access per test
- Automatic cleanup on test completion  
- No interference between parallel test executions
- Consistent behavior across test runs

All acceptance criteria have been met - the workflow storage tests are now fully isolated and can run safely in parallel.