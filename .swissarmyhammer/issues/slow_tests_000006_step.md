# Step 6: Optimize Integration and E2E Tests

Refer to /Users/wballard/sah-slow_tests/ideas/slow_tests.md

## Objective
Optimize end-to-end workflow tests and large integration tests by breaking them into smaller, focused tests and reducing the scope of expensive operations while maintaining comprehensive system validation.

## Background
The SwissArmyHammer codebase contains extensive E2E and integration tests:
- Workflow execution tests (`swissarmyhammer/src/workflow/`)
- CLI integration tests (`swissarmyhammer-cli/tests/`)  
- Complete system workflow tests (`tests/e2e_workflow_tests.rs`)
- Multi-component integration tests combining MCP, files, search, and database operations
- Git repository integration tests

## Tasks

### 1. Identify Large Integration Tests
- Audit tests executing complete workflows end-to-end
- Document tests combining multiple system components
- Map tests with extensive setup/teardown cycles
- Identify tests with complex inter-component dependencies

### 2. Break Down Large Integration Tests
Transform monolithic integration tests into:
- **Focused Integration Tests**: Test specific component interactions
- **Contract Tests**: Validate interfaces between components
- **Component Tests**: Test individual components with mocked dependencies
- **System Tests**: Minimal E2E tests validating critical user journeys
- **Unit Tests**: Test business logic without infrastructure overhead

### 3. Optimize Test Data and Fixtures  
- **Minimal Workflows**: Create simple workflows that validate functionality
- **Reusable Fixtures**: Share test setup across related tests
- **Lazy Initialization**: Initialize expensive resources only when needed
- **Test Data Factories**: Generate minimal test data programmatically

### 4. Optimize Integration Test Patterns

#### Workflow Test Decomposition
```rust
// Instead of one large E2E test
#[test]
fn test_complete_system_workflow() {
    // 1. Setup complex environment (slow)
    // 2. Execute full workflow (slow)  
    // 3. Validate all outputs (slow)
    // 4. Cleanup everything (slow)
}

// Break into focused tests
#[test] 
fn test_workflow_parsing() {
    // Test workflow definition parsing only
}

#[test]
fn test_workflow_execution_engine() {
    // Test execution with mock actions
}

#[test]
fn test_workflow_state_management() {
    // Test state transitions with minimal workflow
}

#[test]
fn test_critical_user_journey() {
    // Minimal E2E test for most important workflow
}
```

#### Mock-Heavy Integration Testing
```rust
// Replace expensive real components with mocks
#[test]
fn test_cli_mcp_integration() {
    let mock_mcp_server = MockMcpServer::new();
    let mock_file_system = MockFileSystem::new();
    
    // Test CLI integration with mocked dependencies
    let result = execute_cli_command_with_mocks(cmd, mock_mcp_server, mock_file_system);
    assert_eq!(result.status, 0);
}
```

### 5. Implement Parallel Integration Testing
- **Test Isolation**: Ensure integration tests can run concurrently
- **Resource Isolation**: Use unique ports, directories, and identifiers
- **State Management**: Avoid shared state between integration tests
- **Cleanup Automation**: Implement reliable test cleanup mechanisms

## Acceptance Criteria
- [ ] All large integration and E2E tests identified and analyzed
- [ ] Monolithic tests broken into focused, smaller tests  
- [ ] Critical user journey E2E tests preserved with minimal scope
- [ ] Integration tests can run in parallel without conflicts
- [ ] Test fixtures and data optimized for speed and reusability
- [ ] Integration test execution time reduced by >50%
- [ ] All system integration test coverage maintained  
- [ ] Clear separation between unit, component, and integration tests

## Implementation Strategy

### Test Categories to Optimize
1. **Workflow Integration Tests** - Complete workflow execution validation
2. **CLI Integration Tests** - End-to-end CLI command testing
3. **MCP E2E Tests** - Full MCP server/client interaction testing
4. **Multi-Component Tests** - Tests spanning file system, database, search, and MCP
5. **System Validation Tests** - Critical user journey validation

### Integration Test Optimization Patterns
- **Test Pyramid Principle**: More unit tests, fewer integration tests, minimal E2E tests
- **Mock Boundaries**: Mock at component boundaries rather than internal dependencies
- **Test Data Management**: Use factories and builders for consistent test data
- **Resource Pooling**: Share expensive resources across related test suites
- **Fail-Fast Testing**: Early termination of integration tests on critical failures

### Test Decomposition Strategy
1. **Identify Core Functionality**: What is the essential behavior being tested?
2. **Separate Concerns**: Split tests by component responsibility
3. **Mock Dependencies**: Replace expensive operations with fast mocks
4. **Preserve Coverage**: Ensure decomposed tests cover same scenarios
5. **Maintain Contracts**: Keep interface validation between components

## Estimated Effort
Extra Large (7-8 focused work sessions)

## Dependencies
- Step 2 (serial test fixes for parallel integration testing)
- Step 3 (MCP optimizations for integration tests using MCP)
- Step 4 (file system optimizations for integration tests using files)  
- Step 5 (database optimizations for integration tests using search/storage)

## Follow-up Steps
- Step 7: Performance Monitoring and Regression Prevention
- Integration test optimizations will provide the largest performance gains