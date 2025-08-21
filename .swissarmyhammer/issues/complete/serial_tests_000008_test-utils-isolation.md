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

## Proposed Solution

Based on my analysis, I need to:

1. **Remove the serial attribute**: Remove `#[serial_test::serial]` from the `test_setup_test_home()` test function at line 636.

2. **Replace with IsolatedTestEnvironment**: The test currently uses `IsolatedTestHome::new()` which is the correct modern pattern, but it needs to be updated to use `IsolatedTestEnvironment::new()` instead to be consistent with other tests and provide better isolation.

3. **Update the guard usage**: Change from:
   ```rust
   let _guard = IsolatedTestHome::new();
   ```
   to:
   ```rust
   let _guard = IsolatedTestEnvironment::new().unwrap();
   ```

4. **Update path access**: The test can continue to use `std::env::var("HOME")` as `IsolatedTestEnvironment` manages the HOME environment variable internally.

The test logic can remain the same since it's already testing the isolation behavior properly - it just needs to use the more comprehensive `IsolatedTestEnvironment` instead of the deprecated `IsolatedTestHome`.
## Implementation Notes

### Changes Made

1. **Removed `#[serial_test::serial]` attribute**: ✅ Completed
   - Removed the attribute from the `test_setup_test_home()` function

2. **Updated to use `IsolatedTestEnvironment`**: ✅ Completed
   - Changed from `IsolatedTestHome::new()` to `IsolatedTestEnvironment::new().unwrap()`
   - This provides more comprehensive isolation including both HOME and working directory

3. **Test Logic Preserved**: ✅ Completed  
   - All existing test assertions remain the same
   - The test still verifies the isolated home directory structure
   - No functional changes to the test behavior

### Verification Results

- **Individual Test**: `cargo test test_setup_test_home` ✅ PASS
- **Parallel Execution**: `cargo test test_utils -- --test-threads=8` ✅ PASS (8 tests passed)
- **Build Check**: `cargo check` ✅ PASS
- **Code Format**: `cargo fmt --all` ✅ PASS

### Final Implementation

```rust
#[test]
fn test_setup_test_home() {
    let _guard = IsolatedTestEnvironment::new().unwrap();

    let home = std::env::var("HOME").expect("HOME not set");
    // HOME should now point to our isolated temp directory
    assert!(PathBuf::from(&home).exists());

    // Verify the mock SwissArmyHammer structure exists
    let swissarmyhammer_dir = PathBuf::from(&home).join(".swissarmyhammer");
    assert!(swissarmyhammer_dir.exists());
    assert!(swissarmyhammer_dir.join("prompts").exists());
    assert!(swissarmyhammer_dir.join("workflows").exists());
}
```

The test now uses the modern `IsolatedTestEnvironment` pattern, allowing it to run in parallel with other tests without interference. All acceptance criteria have been met.

## Code Review Completion

Successfully resolved all clippy lint errors identified in the code review:

### Fixed Lint Issues

1. **Empty line after outer attribute** - `swissarmyhammer-cli/tests/cli_integration_test.rs:13`
   - Removed unnecessary empty line between doc comments
   
2. **Empty string in writeln!** - `swissarmyhammer-cli/tests/in_process_test_utils.rs` (lines 154, 169, 183)
   - Replaced `writeln!(stderr, "")` with `stderr.write_all(b"\n")` for better performance
   
3. **Needless return statements** - `swissarmyhammer-cli/tests/e2e_workflow_tests.rs` (lines 96, 99, 104, 108)
   - Removed explicit `return` keywords from tail expressions
   
4. **Redundant closure** - `swissarmyhammer-cli/src/config.rs:446`
   - Changed `.map(|v| format_config_value(v))` to `.map(format_config_value)`
   
5. **Unnecessary map_or and needless_borrows** - `swissarmyhammer-cli/tests/in_process_test_utils.rs:175`
   - Changed `std::fs::metadata(&plan_path).map_or(false, |m| m.len() == 0)` to `std::fs::metadata(plan_path).is_ok_and(|m| m.len() == 0)`

### Verification Results

- ✅ `cargo clippy --all-targets --all-features -- -D warnings` passes
- ✅ `cargo test test_setup_test_home` passes  
- ✅ `cargo test test_utils` passes (all 8 tests including concurrent access)
- ✅ All lint errors resolved
- ✅ No functional regressions

### Summary

The branch is now clean and ready with all clippy lint errors resolved. The core implementation from the original serial test removal work remains intact and functional. The `test_setup_test_home` test now properly uses `IsolatedTestEnvironment` and can run in parallel with other tests.