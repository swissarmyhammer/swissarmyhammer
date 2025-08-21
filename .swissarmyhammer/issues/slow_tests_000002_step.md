# Step 2: Fix Serial Test Dependencies

Refer to /Users/wballard/sah-slow_tests/ideas/slow_tests.md

## Objective
Remove unnecessary `#[serial]` annotations and refactor tests to enable parallel execution where possible, as tests need to run in isolated environments per the specification.

## Background
Tests marked with `#[serial]` prevent parallel execution and significantly slow down the test suite. The specification explicitly states tests should not use `#[serial]` and must run in isolated environments.

## Tasks

### 1. Identify Serial Tests
- Search codebase for `#[serial]` annotations using grep
- Document each serial test and the reason for serialization
- Categorize by type of resource conflict (files, ports, environment, etc.)

### 2. Analyze Serial Dependencies
For each serial test, identify:
- **Shared Resources**: Files, directories, ports, environment variables
- **State Dependencies**: Tests that depend on state from other tests
- **External Dependencies**: Services, databases, or external processes
- **Configuration Conflicts**: Tests that modify global configuration

### 3. Refactor for Isolation
Apply appropriate isolation strategies:
- **Unique Test Data**: Use unique file paths, ports, or identifiers per test
- **Temporary Directories**: Use `tempfile` crate for isolated file operations
- **Process Isolation**: Ensure each test runs independent processes
- **Resource Cleanup**: Proper setup/teardown to avoid state leaks
- **Environment Isolation**: Use scoped environment variable changes

### 4. Remove Serial Annotations
- Remove `#[serial]` annotations from refactored tests
- Ensure tests can run concurrently without conflicts
- Verify test reliability with parallel execution

## Acceptance Criteria
- [ ] All `#[serial]` annotations identified and documented
- [ ] Tests refactored to use isolated resources (temp directories, unique ports, etc.)
- [ ] Serial annotations removed where possible
- [ ] Tests pass reliably when run in parallel
- [ ] No shared state dependencies between tests
- [ ] All tests maintain same functional coverage

## Implementation Strategy

### Resource Isolation Patterns
```rust
// Instead of shared directories
#[serial] // REMOVE THIS
#[test]
fn test_file_operations() {
    let temp_dir = tempfile::tempdir().unwrap();
    let test_file = temp_dir.path().join("test.txt");
    // Test operations on unique file path
}

// Instead of shared ports  
#[test] 
fn test_server() {
    let port = get_free_port(); // Dynamic port allocation
    // Test with unique port
}
```

### Common Serial Test Types to Address
- File system tests using shared directories
- Network tests using fixed ports  
- MCP server tests with shared communication channels
- Configuration tests modifying global state
- Database tests sharing connections or schemas

## Estimated Effort
Medium (3-4 focused work sessions)

## Dependencies
- Step 1 (analysis of current slow tests)

## Follow-up Steps
- Step 3: Optimize MCP Integration Tests
- Parallel execution will enable faster overall test suite performance