# Test Refactoring Plan: Remove cargo build from tests

## Objective
Eliminate all `cargo run` subprocess spawning from tests that cause build lock deadlocks.

## Tests to Refactor

### 1. swissarmyhammer-tools/tests/rmcp_stdio_working.rs
**Current:** Lines 61-134 spawn cargo run
**Refactor:** 
- Extract MCP server creation into testable module
- Use HTTP mode with reqwest client
- OR create InMemoryTransport for RMCP

### 2. swissarmyhammer-tools/tests/rmcp_integration.rs  
**Current:** Lines 61-134 spawn cargo run
**Refactor:** Same as #1

### 3. swissarmyhammer-cli/tests/mcp_integration_test.rs
**Three ignored tests:**
- test_mcp_server_basic_functionality (line 14)
- test_mcp_server_prompt_loading_e2e (line 189)
- test_mcp_server_builtin_prompts (line 315)

**Refactor:**
- Replace stdin/stdout pipes with HTTP client
- Already have `start_mcp_server()` - just need to call it properly
- Use reqwest for JSON-RPC calls

## Implementation Steps

### Step 1: Add test utilities module
```rust
// swissarmyhammer-tools/src/mcp/test_utils.rs
pub struct TestMcpClient {
    base_url: String,
    client: reqwest::Client,
}

impl TestMcpClient {
    pub async fn new(server: &McpServerHandle) -> Self {
        Self {
            base_url: server.url(),
            client: reqwest::Client::new(),
        }
    }
    
    pub async fn list_tools(&self) -> Result<Vec<Tool>> {
        let response = self.client
            .post(&self.base_url)
            .json(&json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "tools/list"
            }))
            .send()
            .await?;
        // ... parse response
    }
    
    pub async fn call_tool(&self, name: &str, args: Value) -> Result<Value> {
        // ... implement
    }
}
```

### Step 2: Refactor each test
Replace subprocess spawning with direct server instantiation + HTTP client.

### Step 3: Keep E2E tests but make them optional
- Rename ignored tests to `*_e2e_subprocess` to be explicit
- Document when to run them (manual testing, not CI)
- Keep them for integration validation only

## Success Criteria
- All tests pass without `#[ignore]`
- Tests run in <1s each instead of 20-30s
- No build lock deadlocks
- Can run `cargo test` in parallel successfully

## Timeline
- Analysis: Complete
- Implementation: 2-4 hours
- Testing: 1 hour
