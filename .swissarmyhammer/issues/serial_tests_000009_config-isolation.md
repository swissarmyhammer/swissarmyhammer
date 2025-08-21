# Remove Serial Tests from config.rs

Refer to /Users/wballard/github/swissarmyhammer/ideas/serial_tests.md

## Goal
Remove the `#[serial_test::serial]` attributes from 2 tests in `swissarmyhammer/src/config.rs` and replace them with proper `IsolatedTestEnvironment` usage.

## Current State
- File: `swissarmyhammer/src/config.rs`
- 2 tests with `#[serial_test::serial]` attributes at lines 129 and 164

## Tasks
1. **Remove Serial Attributes**
   - Remove `#[serial_test::serial]` from both test functions
   
2. **Implement Isolation**
   - Add `IsolatedTestEnvironment::new()` guard at start of each test
   - Update any hardcoded paths to use the isolated environment
   - Remove manual temp directory creation if present
   
3. **Fix Configuration Access**
   - Ensure tests use isolated HOME for config file access
   - Update any global configuration caching to work with isolated environments
   - Remove any in-memory caches that prevent parallel execution
   
4. **Verify Test Independence**
   - Ensure each test operates on its own config files
   - Remove any shared state between tests
   - Verify tests can run in parallel with others

5. **Test Validation**
   - Run each test multiple times to ensure consistency
   - Run tests in parallel to verify no race conditions
   - Ensure all assertions pass

## Acceptance Criteria
- [ ] `#[serial_test::serial]` attributes removed from both tests
- [ ] Both tests use `IsolatedTestEnvironment::new()` pattern
- [ ] Tests pass when run individually
- [ ] Tests pass when run in parallel with other tests
- [ ] No manual temp directory creation
- [ ] All existing test logic preserved
- [ ] Configuration files are properly isolated per test

## Implementation Notes
- Config tests often need to read/write configuration files - ensure these use the isolated HOME directory
- Look for any global configuration caching that might need to be disabled or reset per test
- The specification mentions removing caching if it prevents parallel execution