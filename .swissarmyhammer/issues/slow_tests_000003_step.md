# Step 3: Optimize MCP Integration Tests

Refer to /Users/wballard/sah-slow_tests/ideas/slow_tests.md

## Objective
Optimize tests that involve MCP (Message Control Protocol) server startup, communication, and shutdown to reduce execution time through better test structure and resource management.

## Background
MCP integration tests are inherently slow because they:
- Start MCP servers as separate processes
- Establish IO-based communication channels  
- Wait for server initialization and responses
- Have complex setup/teardown cycles

Based on the codebase analysis, there are extensive MCP tests in `swissarmyhammer-cli/tests/` and `swissarmyhammer-tools/`.

## Tasks

### 1. Audit MCP Test Structure
- Identify all tests involving MCP server communication
- Document current test patterns and bottlenecks
- Map test dependencies and shared functionality

### 2. Optimize Server Lifecycle Management
- **Reduce Server Restarts**: Group related tests to reuse server instances where safe
- **Faster Initialization**: Optimize server startup time with minimal configuration
- **Parallel Server Tests**: Ensure MCP servers can run on different ports/channels concurrently
- **Cleanup Efficiency**: Implement faster server shutdown and cleanup

### 3. Break Down Large Integration Tests
Split complex MCP integration tests into:
- **Unit Tests**: Test MCP message handling logic without server overhead
- **Component Tests**: Test MCP tool interactions with mock servers
- **Focused Integration Tests**: Smaller tests validating specific MCP workflows
- **Contract Tests**: Validate MCP protocol compliance without full integration

### 4. Implement Test Optimization Patterns

#### Server Instance Reuse Pattern
```rust
// Instead of starting server per test
#[tokio::test]
async fn test_mcp_workflow() {
    let server = start_mcp_server().await; // Expensive
    // Single test operation
    server.shutdown().await;
}

// Use shared server for related test group
static ONCE: std::sync::Once = std::sync::Once::new();
static mut MCP_SERVER: Option<McpServer> = None;

fn get_or_create_server() -> &'static McpServer {
    // Shared server instance for test group
}
```

#### Mock-Heavy Testing
```rust
// Replace real MCP communication with mocks for unit tests
#[test] 
fn test_mcp_tool_logic() {
    let mock_client = MockMcpClient::new();
    mock_client.expect_call().returning(|_| Ok(response));
    // Test logic without server overhead
}
```

## Acceptance Criteria
- [ ] All MCP integration tests identified and categorized
- [ ] Large integration tests split into focused, smaller tests  
- [ ] Server lifecycle optimized to reduce startup/shutdown overhead
- [ ] Tests can run in parallel with isolated MCP server instances
- [ ] Unit tests created for MCP tool logic using mocks
- [ ] Overall MCP test execution time reduced by >50%
- [ ] All existing test coverage maintained

## Implementation Strategy

### Test Categories to Optimize
1. **CLI MCP Integration Tests** (`swissarmyhammer-cli/tests/`)
2. **MCP Tool Tests** (`swissarmyhammer-tools/src/mcp/`)  
3. **E2E MCP Workflow Tests**
4. **MCP Server Management Tests**

### Optimization Techniques
- Use `tokio-test` for async test utilities
- Implement test-specific MCP server configurations
- Create reusable MCP test fixtures
- Add timeout controls for MCP operations
- Use in-memory communication where possible

## Estimated Effort
Large (5-6 focused work sessions)

## Dependencies
- Step 2 (serial test fixes to enable parallel MCP server testing)

## Follow-up Steps
- Step 4: Optimize File System Heavy Tests
- Improved MCP test performance will significantly impact overall test suite speed