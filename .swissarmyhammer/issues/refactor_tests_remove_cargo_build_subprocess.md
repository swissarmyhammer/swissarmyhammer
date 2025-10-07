# Refactor Tests: Remove cargo build Subprocess Spawning

## Problem

Five integration tests spawn `cargo build/run` as subprocesses to test MCP server functionality. This causes:

- **Build lock deadlocks** when running tests in parallel
- **20-30+ second** test execution times per test
- Tests are marked `#[ignore]` and can't run in normal CI
- Tests focus on process spawning rather than actual code behavior

## Affected Tests

### swissarmyhammer-tools (2 tests)
1. `tests/rmcp_stdio_working.rs:61` - `test_stdio_rmcp_client_lists_tools_and_prompts_e2e`
2. `tests/rmcp_integration.rs:61` - `test_stdio_server_with_rmcp_client_e2e`

### swissarmyhammer-cli (3 tests)
3. `tests/mcp_integration_test.rs:14` - `test_mcp_server_basic_functionality`
4. `tests/mcp_integration_test.rs:189` - `test_mcp_server_prompt_loading_e2e`
5. `tests/mcp_integration_test.rs:315` - `test_mcp_server_builtin_prompts`

## Root Cause

Tests were written to validate MCP protocol compliance by:
1. Spawning `cargo run --bin sah -- serve` subprocess
2. Using stdin/stdout pipes for JSON-RPC communication
3. Testing through subprocess IPC instead of library APIs

This is **backwards** - we should test the library code directly, not the CLI wrapper.

## Proposed Solutions

### Approach 1: Direct API Testing (Fastest)

Test server methods directly without any IPC:

```rust
#[tokio::test]
async fn test_mcp_tool_listing() {
    use swissarmyhammer_tools::mcp::unified_server::McpServer;
    
    let server = McpServer::new(None).await.unwrap();
    let tools = server.list_tools().await.unwrap();
    
    assert!(!tools.is_empty());
    assert!(tools.iter().any(|t| t.name == "files_read"));
}
```

**Benefits:** 100x faster, no subprocess, no build lock, tests actual code

### Approach 2: HTTP Client Testing (Protocol Validation)

Use in-process HTTP server with reqwest client:

```rust
#[tokio::test]
async fn test_mcp_jsonrpc_protocol() {
    let mut server = start_mcp_server(McpServerMode::Http { port: None }, None)
        .await
        .unwrap();
    
    let client = reqwest::Client::new();
    let response = client
        .post(server.url())
        .json(&json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/list"
        }))
        .send()
        .await
        .unwrap();
    
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["jsonrpc"], "2.0");
}
```

**Benefits:** Tests JSON-RPC protocol, still fast, no subprocess

### Approach 3: Mock Transport for RMCP

Create in-memory transport for RMCP client tests:

```rust
struct InMemoryMcpTransport {
    server: McpServer,
}

impl McpTransport for InMemoryMcpTransport {
    async fn send(&mut self, request: Value) -> Result<Value> {
        self.server.handle_request(request).await
    }
}
```

**Benefits:** Tests RMCP client integration without subprocess

## Implementation Plan

### Step 1: Create Test Utilities

Add `swissarmyhammer-tools/src/mcp/test_utils.rs`:

```rust
pub struct TestMcpClient {
    base_url: String,
    client: reqwest::Client,
}

impl TestMcpClient {
    pub async fn new(server: &McpServerHandle) -> Self { /* ... */ }
    pub async fn list_tools(&self) -> Result<Vec<Tool>> { /* ... */ }
    pub async fn call_tool(&self, name: &str, args: Value) -> Result<Value> { /* ... */ }
    pub async fn list_prompts(&self) -> Result<Vec<Prompt>> { /* ... */ }
}
```

### Step 2: Refactor Each Test File

**For `rmcp_stdio_working.rs` and `rmcp_integration.rs`:**
- Replace subprocess spawning with `start_mcp_server(McpServerMode::Http)`
- Use `TestMcpClient` or `reqwest` to make JSON-RPC calls
- Keep test assertions, replace only the transport mechanism

**For `mcp_integration_test.rs`:**
- Already has `test_mcp_server_prompt_loading()` as fast in-process test (line 128)
- Extend this pattern to cover the three ignored tests
- Replace stdin/stdout pipes with HTTP client

### Step 3: Handle E2E Subprocess Tests

**Option A: Delete them** - They test CLI, not library code
**Option B: Keep but rename** - Rename to `*_e2e_subprocess_manual` and document as manual-only tests

Recommendation: **Delete them**. If we need CLI testing, create separate CLI integration tests that don't block library tests.

### Step 4: Dependencies

May need to add to test dependencies:
```toml
[dev-dependencies]
reqwest = { version = "0.11", features = ["json"] }
```

## Success Criteria

- [ ] All 5 ignored tests replaced with fast in-process versions
- [ ] No tests spawn `cargo build/run`
- [ ] All tests complete in <1 second each (target: <100ms)
- [ ] Tests can run in parallel without deadlocks
- [ ] No `#[ignore]` attributes needed
- [ ] `cargo test` passes on all tests
- [ ] CI can run full test suite

## Implementation Checklist

- [ ] Add `TestMcpClient` utility to `swissarmyhammer-tools`
- [ ] Refactor `rmcp_stdio_working.rs` tests
- [ ] Refactor `rmcp_integration.rs` tests  
- [ ] Refactor `mcp_integration_test.rs` tests
- [ ] Remove or rename E2E subprocess tests
- [ ] Update test documentation
- [ ] Verify all tests pass: `cargo test --workspace`
- [ ] Verify tests run fast: `cargo test --workspace --timings`

## Estimated Effort

- Analysis: Complete ✓
- Implementation: 2-4 hours
- Testing & validation: 1 hour
- **Total: 3-5 hours**

## Related Files

- `swissarmyhammer-tools/tests/rmcp_stdio_working.rs`
- `swissarmyhammer-tools/tests/rmcp_integration.rs`
- `swissarmyhammer-cli/tests/mcp_integration_test.rs`
- `swissarmyhammer-tools/src/mcp/unified_server.rs` (already has `start_mcp_server`)

## Notes

The codebase already has the infrastructure for fast in-process testing:
- `start_mcp_server()` creates HTTP server without subprocess
- Some tests already use this pattern (e.g., line 20-54 in rmcp_stdio_working.rs)
- Just need to extend the pattern consistently across all tests

The key insight: **Test the library, not the CLI binary.**
