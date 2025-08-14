# Testing Approach: Parallel Test Isolation

## Problem
The current test setup uses a shared HOME environment variable modification through a global mutex. This causes tests to serialize when run in parallel, defeating the purpose of parallel testing and causing hangs/deadlocks.

## Solution
**Per-Test Isolation**: Each test should create its own temporary directory and use it as HOME, rather than modifying the global HOME environment variable.


## **RECOMMENDED PATTERN: Use IsolatedTestHome RAII Guard**

**ALWAYS use `IsolatedTestEnvironment::new()` for workflow tests to isolate current working and home**

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
