# Testing Approach: Parallel Test Isolation

AVOID AT ALL COSTS #[serial] tests.

## Patterns

### `IsolatedTestEnvironement`

**ALWAYS** use `IsolatedTestEnvironment::new()` for workflow tests to isolate current working and home
    If you think you need a TempDir in a test, you actually need an IsolatedTestEnvironment instead
    If you think you need to modify the HOME environment variable in a test, you actually need an IsolatedTestEnvironment instead

```rust
use swissarmyhammer::test_utils::IsolatedTestEnvironment;

#[test]
fn test_something() {
    let _guard = IsolatedTestEnvironment::new();
    // HOME/PWD now points to an isolated temporary directory
    // with mock .swissarmyhammer structure
    // Original HOME/PWD is restored when _guard is dropped
    // Tests can run in parallel safely
}
```

The `IsolatedTestEnvironment` RAII guard pattern:
- Creates a temporary directory with mock `.swissarmyhammer` structure
- Sets HOME to point to it  
- Restores original HOME on drop
- Repeats this for a temporary PWD
- Allows parallel test execution
- Provides complete test isolation
- Has methods like `.home_path()` and `.swissarmyhammer_dir()` for accessing paths

## Benefits
- True parallel test execution
- No race conditions or deadlocks
- Each test is completely isolated
- Faster test execution
- More reliable CI/CD
- Clean RAII pattern with automatic cleanup

### `run_sah_command_in_process`

**ALWAYS** use `run_sah_command_in_process()` for CLI integration testing.

This avoids the cost of building/spawning the cli while unit testing

Be on the lookout for stray Command::cargo_bin("sah") indicating you have missed a test that should use cargo_bin_sah.