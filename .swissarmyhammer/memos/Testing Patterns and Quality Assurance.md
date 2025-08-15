# SwissArmyHammer Testing Patterns and Quality Assurance

**IMPORTANT** when integration testing the command line, DO NOT use `sah`, instead use `cargo run --`

## Testing Architecture

### Multi-Level Testing Strategy
- **Unit Tests**: Inline `#[cfg(test)]` modules within source files
- **Integration Tests**: External test files in `/tests/` directories  
- **End-to-End Tests**: Complete workflow testing with real processes
- **Property Tests**: Fuzz-like testing with `proptest` crate
- **Performance Tests**: Benchmarking with `criterion` crate

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
- Mock storage backends for testing without filesystem
- Mock MCP servers for protocol testing
- Mock process spawning for CLI testing

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
```rust
#[test]
fn test_prompt_subcommand_list() -> Result<()> {
    let output = Command::cargo_bin("swissarmyhammer")
        .unwrap()
        .args(["prompt", "list"])
        .output()?;
    assert!(output.status.success());
}
```

**MCP Protocol Testing**
- Full protocol handshake simulation
- JSON-RPC message validation
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
- Performance tests: `bench_operation_name()`

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

**Performance Testing**
```rust
// Criterion benchmarking
fn bench_template_rendering(c: &mut Criterion) {
    c.bench_function("template_render", |b| {
        b.iter(|| template.render(black_box(&args)))
    });
}
```

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