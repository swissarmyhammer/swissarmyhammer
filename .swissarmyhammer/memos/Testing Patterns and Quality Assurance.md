# SwissArmyHammer Testing Patterns and Quality Assurance

AVOID AT ALL COSTS #[serial] tests.

## Testing Architecture

### Multi-Level Testing Strategy
- **Unit Tests**: Inline `#[cfg(test)]` modules within source files
- **Integration Tests**: External test files in `/tests/` directories
- **End-to-End Tests**: Complete workflow testing with real processes
- **Property Tests**: Fuzz-like testing with `proptest` crate
- **Performance Tests**: Do no performance testing, ever

### Test Organization Hierarchy
```
workspace/
├── tests/                      # Workspace-level integration tests
├── swissarmyhammer/tests/      # Library integration tests
├── swissarmyhammer-cli/tests/  # CLI integration tests
└── src/**/*.rs                 # Unit tests in #[cfg(test)] modules
```

## Testing Infrastructure

**Core Testing Utilities**
```rust
// Centralized test infrastructure
pub fn create_test_home_guard() -> TestHomeGuard
pub fn create_test_prompt_library() -> PromptLibrary
pub fn create_test_environment() -> Result<(TempDir, PathBuf)>
```

**Resource Management Patterns**
- `TestHomeGuard`: Isolated HOME directory for tests
- `ProcessGuard`: Automatic cleanup of spawned processes
- `TempDir`: Temporary directories with automatic cleanup
- Thread-safe environment variable management

**Mock Implementations**
NEVER Mock, use directory isolation

**IsolatedTestEnvironement**

**ALWAYS** use `IsolatedTestEnvironment::new()` for workflow tests to isolate current working and home
**Per-Test Isolation**: Each test should create its own temporary directory and use it as HOME, rather than modifying the global HOME environment variable.

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



## Property-Based Testing

**PropTest Integration**
```rust
proptest! {
    #[test]
    fn test_template_engine_idempotent(
        s: String,
        args in prop::collection::hash_map(/* ... */)
    ) {
        let result1 = engine.process(&s, &args).unwrap();
        let result2 = engine.process(&s, &args).unwrap();
        assert_eq!(result1, result2);
    }
}
```

**Testing Domains**
- Template engine validation with generated inputs
- Argument validation with random data
- File path validation and security testing
- Serialization round-trip testing

## Integration Testing Patterns

**CLI Integration Testing**
### `run_sah_command_in_process`

**ALWAYS** use `run_sah_command_in_process()` for CLI integration testing.

This avoids the cost of building/spawning the cli while unit testing

Be on the lookout for stray Command::cargo_bin("sah") indicating you have missed a test that should use cargo_bin_sah.

**MCP Protocol Testing**
- Full protocol handshake with an rmcp client and server, never mock or fake our own tools
- Concurrent client simulation
- Server lifecycle testing

**Process Management Testing**
- Automatic process cleanup with `ProcessGuard`
- Signal handling verification
- Resource leak prevention
- Timeout and cancellation testing

## Testing Conventions

**Naming Patterns**
- Unit tests: `test_function_name_scenario()`
- Integration tests: `test_feature_integration()`
- Error cases: `test_error_condition()`

**Test Structure**
```rust
#[test]
fn test_operation_success_case() {
    // Arrange
    let test_data = create_test_data();

    // Act
    let result = operation_under_test(test_data);

    // Assert
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), expected_value);
}
```

## Specialized Testing Features

**Concurrent Testing**
- `serial_test` crate for tests requiring serialization
- Thread safety validation
- Race condition detection
- Deadlock prevention testing

**Environment Isolation**
- Temporary directories for each test
- Environment variable cleanup
- Path validation and security testing
- Cross-platform compatibility testing

## Error Condition Testing

**Comprehensive Error Testing**
- Error propagation validation
- Error message content assertions
- Recovery mechanism testing
- Resource cleanup verification

**Failure Simulation**
- I/O error injection
- Network timeout simulation
- Invalid input boundary testing
- Memory pressure testing

This testing strategy ensures high code quality through comprehensive coverage, realistic failure simulation, and robust resource management while maintaining fast feedback loops for development.
