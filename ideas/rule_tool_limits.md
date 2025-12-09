# Per-Rule Tool Limits

## Goal

Allow rules to specify which MCP tools are available during rule checking via regex patterns in YAML frontmatter.

## Use Cases

1. **Security**: Restrict rules to read-only tools (prevent shell execution, file writes)
2. **Reliability**: Limit rules to deterministic tools (exclude web_search which can vary)
3. **Performance**: Reduce tool options to speed up LLM inference
4. **Debugging**: Test rules with specific tool subsets

## Proposed YAML Frontmatter

```yaml
---
name: no-unwrap
severity: error
allowed_tools:
  - "files_read"
  - "files_grep"
  - "files_glob"
  - "web_fetch"
# OR use regex patterns
allowed_tools_regex:
  - "^files_.*"
  - "^web_fetch$"
# OR deny-list approach
denied_tools_regex:
  - "^shell_.*"
  - "^files_write$"
  - "^files_edit$"
---
```

## Architectural Challenge

**Single MCP Server Instance**: We have one running MCP server with all tools registered. How do we filter tools per-rule when each rule check uses the same server?

## Design Options

### Option 1: Filter at ToolRegistry Level (Pre-Execution)

**Approach**: Create a filtered `ToolRegistry` per rule check before starting the agent.

**Implementation**:
```rust
// In RuleChecker::check_file()
let tool_filter = rule.frontmatter.get_tool_filter();
let filtered_registry = self.tool_registry.clone_with_filter(tool_filter);
let temp_mcp_server = McpServer::new_with_registry(filtered_registry);
let agent = create_agent_with_server(temp_mcp_server);
```

**Pros**:
- Clean separation per rule
- True tool isolation
- No shared state issues

**Cons**:
- Need to spin up/down MCP server per rule check (expensive)
- Complex lifecycle management
- Doesn't work well with long-running server model

### Option 2: Context-Aware MCP Server (Dynamic Filtering)

**Approach**: Pass rule context in MCP requests, server filters tools dynamically.

**Implementation**:
```rust
// Add context to MCP requests
impl McpServer {
    pub async fn list_tools(&self, context: Option<ToolFilterContext>) -> Vec<Tool> {
        let all_tools = self.registry.all_tools();
        
        if let Some(ctx) = context {
            all_tools.into_iter()
                .filter(|tool| ctx.is_allowed(tool.name()))
                .collect()
        } else {
            all_tools
        }
    }
}

// Pass context when creating agent for rule
let agent_context = AgentExecutionContext::new()
    .with_tool_filter(rule.get_tool_filter());
```

**Pros**:
- Single MCP server instance (efficient)
- Dynamic per-request filtering
- Works with long-running server

**Cons**:
- Need to modify MCP protocol to pass context
- More complex server-side logic
- MCP standard may not support this

### Option 3: System Prompt Filtering (LLM-Level)

**Approach**: Tell the LLM in system prompt which tools it's allowed to use.

**Implementation**:
```rust
let system_prompt = format!(
    "You may only use these tools: {}. Do not attempt to use any other tools.",
    rule.allowed_tools.join(", ")
);
```

**Pros**:
- Simple to implement
- No architecture changes
- Works with any MCP setup

**Cons**:
- Not enforceable (LLM might ignore instructions)
- No hard security boundary
- Unreliable

### Option 4: Filtered MCP Config (Config-Level)

**Approach**: Generate different MCP config per rule, pass to Claude CLI.

**Implementation**:
```rust
// In ClaudeCodeExecutor::execute_prompt()
let filtered_tools = filter_tools_for_rule(&all_tools, &rule.allowed_tools_regex);

let mcp_config = json!({
    "mcpServers": {
        "sah": {
            "type": "http",
            "url": server_url,
            "enabled_tools": filtered_tools  // Non-standard extension
        }
    }
});
```

**Pros**:
- Per-rule configuration
- Could work with Claude CLI if it supports filtering

**Cons**:
- Claude CLI may not support tool filtering in config
- Non-standard MCP extension
- Dependent on client support

### Option 5: Tool Wrapping/Proxying (SELECTED)

**Approach**: Wrap main MCP server with a filtering proxy that intercepts and validates all tool calls before forwarding.

**Architecture**:
```
Main MCP Server (all tools registered, always running)
    ↑
    | forwards approved calls
    |
FilteringMcpProxy (per-rule instance)
    - allowed_tools_regex: Vec<Regex>
    - denied_tools_regex: Vec<Regex>
    - wrapped_server: Arc<McpServer>
    ↑
    | MCP protocol (HTTP)
    |
Claude CLI Agent (connects to proxy endpoint)
```

**Implementation**:
```rust
pub struct FilteringMcpProxy {
    wrapped_server: Arc<McpServer>,
    allowed_patterns: Vec<Regex>,
    denied_patterns: Vec<Regex>,
}

impl FilteringMcpProxy {
    pub fn new(
        wrapped_server: Arc<McpServer>,
        allowed_patterns: Vec<Regex>,
        denied_patterns: Vec<Regex>,
    ) -> Self {
        Self {
            wrapped_server,
            allowed_patterns,
            denied_patterns,
        }
    }
    
    fn is_tool_allowed(&self, tool_name: &str) -> bool {
        // Deny patterns take precedence (security-first)
        if self.denied_patterns.iter().any(|p| p.is_match(tool_name)) {
            return false;
        }
        
        // If no allow patterns specified, allow all (except denied)
        if self.allowed_patterns.is_empty() {
            return true;
        }
        
        // Check allow patterns
        self.allowed_patterns.iter().any(|p| p.is_match(tool_name))
    }
    
    // Implement MCP protocol handlers
    async fn handle_list_tools(&self) -> Result<Vec<Tool>> {
        let all_tools = self.wrapped_server.list_tools().await?;
        Ok(all_tools
            .into_iter()
            .filter(|t| self.is_tool_allowed(t.name()))
            .collect())
    }
    
    async fn handle_call_tool(&self, name: &str, args: Value) -> Result<Value> {
        if !self.is_tool_allowed(name) {
            return Err(McpError::ToolNotAllowed {
                tool: name.to_string(),
                reason: "Tool filtered by rule configuration".to_string(),
            });
        }
        
        // Forward to wrapped server
        self.wrapped_server.call_tool(name, args).await
    }
}
```

**Lifecycle**:
```rust
// For each rule check:
1. Create FilteringMcpProxy wrapping main server
2. Start HTTP server for proxy on unique port
3. Generate MCP config pointing to proxy endpoint
4. Pass config to Claude CLI via --mcp-config
5. Claude CLI makes tool requests to proxy
6. Proxy validates and forwards to main server
7. After rule check, shutdown proxy HTTP server
```

**Pros**:
- **Shared state and file locks**: All tools access files through single server instance - no lock contention or race conditions
- **Consistent caching**: Rule check cache, git state, file handles all managed in one place
- **Enforceable filtering**: Protocol-level security boundary (not just LLM instructions)
- **Efficient**: Main server stays running, no repeated initialization
- **Clean separation**: Proxies are thin filtering layers, main server handles complexity
- **Batchable**: Rules with identical filters can share same proxy instance

**Cons**:
- Need to manage proxy HTTP server lifecycle
- Extra network hop (proxy → main server) - negligible on localhost
- Slight complexity in forwarding all MCP protocol methods

## Recommended Approach: **Option 5 (Tool Wrapping) + Option 3 (System Prompt)**

Combine two layers:
1. **Hard boundary**: FilteringMcpProxy prevents unauthorized tool access
2. **Soft guidance**: System prompt tells LLM which tools to use (improves behavior)

## Complete Proxy Implementation

### MCP Protocol Methods to Proxy

Based on rmcp 0.8.4 and the MCP 2024-11-05 specification, the proxy must implement all `ServerHandler` trait methods:

| Method | Filtering Required | Behavior |
|--------|-------------------|----------|
| `initialize()` | No | Forward as-is to wrapped server |
| `list_prompts()` | No | Forward as-is to wrapped server |
| `get_prompt()` | No | Forward as-is to wrapped server |
| `list_tools()` | **YES** | Filter tools by allowed/denied regex |
| `call_tool()` | **YES** | Validate tool name, reject if not allowed |

### New Crate: `swissarmyhammer-mcp-proxy`

**Rationale**:
- Reusable for any MCP server filtering needs
- Testable against external HTTP MCP servers
- Clean separation from main tool implementation
- Can be used by other projects

**Dependencies**:
```toml
[dependencies]
rmcp = "0.8.4"
regex = "1.10"
tokio = { version = "1.0", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
axum = "0.7"
tracing = "0.1"
```

### Proxy Architecture

```rust
// swissarmyhammer-mcp-proxy/src/lib.rs

use rmcp::{
    InitializeRequestParam, InitializeResult,
    PaginatedRequestParam, 
    ListPromptsResult, GetPromptRequestParam, GetPromptResult,
    ListToolsResult, CallToolRequestParam, CallToolResult,
    RequestContext, RoleServer, ErrorData as McpError,
    ServerHandler,
};
use regex::Regex;
use std::sync::Arc;

pub struct ToolFilter {
    allowed_patterns: Vec<Regex>,
    denied_patterns: Vec<Regex>,
}

impl ToolFilter {
    pub fn new(
        allowed: Vec<String>, 
        denied: Vec<String>
    ) -> Result<Self, regex::Error> {
        let allowed_patterns = allowed.iter()
            .map(|s| Regex::new(s))
            .collect::<Result<Vec<_>, _>>()?;
            
        let denied_patterns = denied.iter()
            .map(|s| Regex::new(s))
            .collect::<Result<Vec<_>, _>>()?;
            
        Ok(Self { allowed_patterns, denied_patterns })
    }
    
    pub fn is_allowed(&self, tool_name: &str) -> bool {
        // Deny takes precedence
        if self.denied_patterns.iter().any(|p| p.is_match(tool_name)) {
            return false;
        }
        
        // Empty allow list means allow all (except denied)
        if self.allowed_patterns.is_empty() {
            return true;
        }
        
        // Check allow list
        self.allowed_patterns.iter().any(|p| p.is_match(tool_name))
    }
}

pub struct FilteringMcpProxy<H: ServerHandler> {
    wrapped_handler: Arc<H>,
    tool_filter: ToolFilter,
}

impl<H: ServerHandler> FilteringMcpProxy<H> {
    pub fn new(wrapped_handler: Arc<H>, tool_filter: ToolFilter) -> Self {
        Self {
            wrapped_handler,
            tool_filter,
        }
    }
}

#[async_trait::async_trait]
impl<H: ServerHandler + Send + Sync> ServerHandler for FilteringMcpProxy<H> {
    // Forward without modification
    async fn initialize(
        &self,
        request: InitializeRequestParam,
        context: RequestContext<RoleServer>,
    ) -> Result<InitializeResult, McpError> {
        self.wrapped_handler.initialize(request, context).await
    }
    
    // Forward without modification
    async fn list_prompts(
        &self,
        request: Option<PaginatedRequestParam>,
        context: RequestContext<RoleServer>,
    ) -> Result<ListPromptsResult, McpError> {
        self.wrapped_handler.list_prompts(request, context).await
    }
    
    // Forward without modification
    async fn get_prompt(
        &self,
        request: GetPromptRequestParam,
        context: RequestContext<RoleServer>,
    ) -> Result<GetPromptResult, McpError> {
        self.wrapped_handler.get_prompt(request, context).await
    }
    
    // FILTER: Only return allowed tools
    async fn list_tools(
        &self,
        request: Option<PaginatedRequestParam>,
        context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        let result = self.wrapped_handler.list_tools(request, context).await?;
        
        let filtered_tools = result.tools
            .into_iter()
            .filter(|tool| self.tool_filter.is_allowed(&tool.name))
            .collect();
            
        Ok(ListToolsResult {
            tools: filtered_tools,
            next_cursor: result.next_cursor,
        })
    }
    
    // VALIDATE: Block disallowed tool calls
    async fn call_tool(
        &self,
        request: CallToolRequestParam,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        if !self.tool_filter.is_allowed(&request.name) {
            return Err(McpError::invalid_request(
                format!("Tool '{}' is not allowed by filter", request.name),
                None,
            ));
        }
        
        self.wrapped_handler.call_tool(request, context).await
    }
}
```

### HTTP Server for Proxy

```rust
// swissarmyhammer-mcp-proxy/src/server.rs

use axum::Router;
use rmcp::transport::{StreamableHttpService, LocalSessionManager};
use std::net::SocketAddr;

pub async fn start_proxy_server<H: ServerHandler + Send + Sync + 'static>(
    handler: Arc<FilteringMcpProxy<H>>,
    port: Option<u16>,
) -> Result<(u16, tokio::task::JoinHandle<()>), Box<dyn std::error::Error>> {
    let session_manager = Arc::new(LocalSessionManager::default());
    let service = StreamableHttpService::new(handler, session_manager);
    
    let app = Router::new()
        .route("/mcp", axum::routing::post(service.handle_request))
        .route("/health", axum::routing::get(|| async { "OK" }));
    
    let port = port.unwrap_or(0); // 0 = auto-assign
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    let actual_port = listener.local_addr()?.port();
    
    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    
    Ok((actual_port, handle))
}
```

### Testing Strategy

#### Phase 1: Unit Tests (swissarmyhammer-mcp-proxy/src/filter.rs)

**ToolFilter Logic Tests**:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_filter_deny_precedence() {
        let filter = ToolFilter::new(
            vec![".*".to_string()],  // Allow all
            vec!["^shell_.*".to_string()],  // Deny shell tools
        ).unwrap();
        
        assert!(filter.is_allowed("files_read"));
        assert!(!filter.is_allowed("shell_execute"));
        assert!(!filter.is_allowed("shell_kill"));
    }
    
    #[test]
    fn test_filter_empty_allow_list() {
        let filter = ToolFilter::new(
            vec![],  // Empty = allow all
            vec!["^dangerous_.*".to_string()],
        ).unwrap();
        
        assert!(filter.is_allowed("safe_tool"));
        assert!(!filter.is_allowed("dangerous_tool"));
    }
    
    #[test]
    fn test_filter_specific_patterns() {
        let filter = ToolFilter::new(
            vec!["^files_read$".to_string(), "^files_grep$".to_string()],
            vec![],
        ).unwrap();
        
        assert!(filter.is_allowed("files_read"));
        assert!(filter.is_allowed("files_grep"));
        assert!(!filter.is_allowed("files_write"));
        assert!(!filter.is_allowed("files_edit"));
    }
    
    #[test]
    fn test_filter_regex_errors() {
        let result = ToolFilter::new(
            vec!["[invalid".to_string()],  // Invalid regex
            vec![],
        );
        
        assert!(result.is_err());
    }
    
    #[test]
    fn test_filter_complex_patterns() {
        let filter = ToolFilter::new(
            vec!["^(files|web)_.*".to_string()],  // Allow files_* and web_*
            vec![".*_(write|edit|delete)$".to_string()],  // Deny mutations
        ).unwrap();
        
        assert!(filter.is_allowed("files_read"));
        assert!(filter.is_allowed("web_fetch"));
        assert!(!filter.is_allowed("files_write"));  // Denied
        assert!(!filter.is_allowed("web_edit"));  // Denied
        assert!(!filter.is_allowed("shell_execute"));  // Not allowed
    }
}
```

#### Phase 2: Mock Server Tests (swissarmyhammer-mcp-proxy/tests/mock_server_test.rs)

**Create Mock ServerHandler**:
```rust
use rmcp::*;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Default)]
struct MockMcpServer {
    list_tools_calls: Arc<Mutex<Vec<()>>>,
    call_tool_calls: Arc<Mutex<Vec<String>>>,
}

#[async_trait::async_trait]
impl ServerHandler for MockMcpServer {
    async fn initialize(
        &self,
        _request: InitializeRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> Result<InitializeResult, McpError> {
        Ok(InitializeResult {
            protocol_version: "2024-11-05".to_string(),
            capabilities: ServerCapabilities::default(),
            server_info: Implementation {
                name: "mock".to_string(),
                version: "1.0.0".to_string(),
            },
        })
    }
    
    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParam>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        self.list_tools_calls.lock().await.push(());
        
        Ok(ListToolsResult {
            tools: vec![
                Tool {
                    name: "allowed_read".to_string(),
                    description: Some("Allowed tool".to_string()),
                    input_schema: serde_json::json!({}),
                },
                Tool {
                    name: "denied_write".to_string(),
                    description: Some("Denied tool".to_string()),
                    input_schema: serde_json::json!({}),
                },
            ],
            next_cursor: None,
        })
    }
    
    async fn call_tool(
        &self,
        request: CallToolRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        self.call_tool_calls.lock().await.push(request.name.clone());
        
        Ok(CallToolResult {
            content: vec![TextContent {
                type_: "text".to_string(),
                text: format!("Executed {}", request.name),
            }],
            is_error: None,
        })
    }
    
    // Implement other methods as no-ops...
}

#[tokio::test]
async fn test_proxy_filters_list_tools() {
    let mock = Arc::new(MockMcpServer::default());
    let filter = ToolFilter::new(
        vec!["^allowed_.*".to_string()],
        vec![],
    ).unwrap();
    
    let proxy = FilteringMcpProxy::new(mock.clone(), filter);
    
    let result = proxy.list_tools(None, RequestContext::default()).await.unwrap();
    
    assert_eq!(result.tools.len(), 1);
    assert_eq!(result.tools[0].name, "allowed_read");
}

#[tokio::test]
async fn test_proxy_blocks_disallowed_tool_calls() {
    let mock = Arc::new(MockMcpServer::default());
    let filter = ToolFilter::new(
        vec!["^allowed_.*".to_string()],
        vec![],
    ).unwrap();
    
    let proxy = FilteringMcpProxy::new(mock.clone(), filter);
    
    let request = CallToolRequestParam {
        name: "denied_write".to_string(),
        arguments: None,
    };
    
    let result = proxy.call_tool(request, RequestContext::default()).await;
    
    assert!(result.is_err());
    assert_eq!(mock.call_tool_calls.lock().await.len(), 0);  // Never forwarded
}

#[tokio::test]
async fn test_proxy_forwards_allowed_tool_calls() {
    let mock = Arc::new(MockMcpServer::default());
    let filter = ToolFilter::new(
        vec!["^allowed_.*".to_string()],
        vec![],
    ).unwrap();
    
    let proxy = FilteringMcpProxy::new(mock.clone(), filter);
    
    let request = CallToolRequestParam {
        name: "allowed_read".to_string(),
        arguments: None,
    };
    
    let result = proxy.call_tool(request, RequestContext::default()).await;
    
    assert!(result.is_ok());
    assert_eq!(mock.call_tool_calls.lock().await.len(), 1);
}
```

#### Phase 3: Integration Tests with Real SwissArmyHammer MCP Server

**Test Against Actual Server** (swissarmyhammer-mcp-proxy/tests/integration_test.rs):
```rust
use swissarmyhammer_tools::mcp::{McpServer, ToolRegistry};
use swissarmyhammer_cli::mcp_integration::CliToolContext;

#[tokio::test]
async fn test_proxy_with_swissarmyhammer_server() {
    // Create real tool context and registry
    let temp_dir = tempfile::tempdir().unwrap();
    let context = CliToolContext::new(temp_dir.path(), None).await.unwrap();
    
    // Create real MCP server
    let server = McpServer::new(context);
    
    // Create proxy with restrictive filter
    let filter = ToolFilter::new(
        vec!["^files_read$".to_string(), "^files_glob$".to_string()],
        vec![],
    ).unwrap();
    
    let proxy = FilteringMcpProxy::new(Arc::new(server), filter);
    
    // Start proxy HTTP server
    let (port, handle) = start_proxy_server(Arc::new(proxy), None).await.unwrap();
    
    // Test via HTTP MCP client
    let client = create_http_mcp_client(format!("http://127.0.0.1:{}/mcp", port));
    
    // Verify tools are filtered
    let tools = client.list_tools().await.unwrap();
    assert_eq!(tools.tools.len(), 2);
    assert!(tools.tools.iter().any(|t| t.name == "files_read"));
    assert!(tools.tools.iter().any(|t| t.name == "files_glob"));
    
    // Verify allowed tool works
    let result = client.call_tool("files_read", json!({
        "path": format!("{}/test.txt", temp_dir.path().display())
    })).await.unwrap();
    assert!(!result.is_error.unwrap_or(false));
    
    // Verify disallowed tool blocked
    let result = client.call_tool("shell_execute", json!({
        "command": "echo test"
    })).await;
    assert!(result.is_err());
    
    handle.abort();
}
```

#### Phase 4: Performance Tests

**Measure Proxy Overhead** (benches/proxy_bench.rs):
```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_filter_matching(c: &mut Criterion) {
    let filter = ToolFilter::new(
        vec!["^files_.*".to_string()],
        vec![".*_write$".to_string()],
    ).unwrap();
    
    c.bench_function("filter_is_allowed", |b| {
        b.iter(|| {
            black_box(filter.is_allowed("files_read"));
            black_box(filter.is_allowed("files_write"));
            black_box(filter.is_allowed("shell_execute"));
        })
    });
}

#[tokio::main]
async fn bench_proxy_call_overhead(c: &mut Criterion) {
    let mock = Arc::new(MockMcpServer::default());
    let filter = ToolFilter::new(vec![".*".to_string()], vec![]).unwrap();
    let proxy = FilteringMcpProxy::new(mock.clone(), filter);
    
    c.bench_function("proxy_list_tools", |b| {
        b.iter(|| {
            let proxy = proxy.clone();
            async move {
                black_box(proxy.list_tools(None, RequestContext::default()).await)
            }
        })
    });
}

criterion_group!(benches, bench_filter_matching, bench_proxy_call_overhead);
criterion_main!(benches);
```

#### Phase 5: End-to-End Tests with Claude CLI

**Test Real Agent Execution Through Proxy** (manual test script):
```bash
#!/bin/bash
# test_proxy_with_claude.sh

# Start SwissArmyHammer MCP server on port 8080
cargo run --bin sah -- mcp-server --port 8080 &
SERVER_PID=$!

# Start proxy server wrapping main server on port 8081
cargo run --bin mcp-proxy -- \
  --upstream http://127.0.0.1:8080/mcp \
  --port 8081 \
  --allowed '^files_(read|glob|grep)$' \
  --denied '^shell_.*' &
PROXY_PID=$!

sleep 2

# Create MCP config pointing to proxy
cat > /tmp/proxy-mcp-config.json <<EOF
{
  "mcpServers": {
    "sah_filtered": {
      "type": "http",
      "url": "http://127.0.0.1:8081/mcp"
    }
  }
}
EOF

# Run Claude CLI with proxy
claude \
  --mcp-config /tmp/proxy-mcp-config.json \
  --tools "" \
  --print \
  <<< "List all available tools and try to execute shell_execute"

# Cleanup
kill $SERVER_PID $PROXY_PID
```

**Expected Result**: Claude should only see files_read, files_glob, files_grep, and attempts to call shell_execute should fail.

#### Phase 6: Security Tests

**Verify Security Boundaries** (tests/security_test.rs):
```rust
#[tokio::test]
async fn test_cannot_bypass_filter_with_case_variation() {
    let filter = ToolFilter::new(vec!["^files_read$".to_string()], vec![]).unwrap();
    
    assert!(filter.is_allowed("files_read"));
    assert!(!filter.is_allowed("Files_Read"));  // Case sensitive
    assert!(!filter.is_allowed("FILES_READ"));
}

#[tokio::test]
async fn test_cannot_bypass_filter_with_whitespace() {
    let filter = ToolFilter::new(vec!["^files_read$".to_string()], vec![]).unwrap();
    
    assert!(!filter.is_allowed(" files_read"));
    assert!(!filter.is_allowed("files_read "));
    assert!(!filter.is_allowed("files\nread"));
}

#[tokio::test]
async fn test_cannot_bypass_filter_with_prefix_matching() {
    let filter = ToolFilter::new(vec!["^files_read$".to_string()], vec![]).unwrap();
    
    assert!(!filter.is_allowed("files_read_secret"));
    assert!(!filter.is_allowed("files_reader"));
}
```

### Test Execution Plan

1. **Unit tests** (5 min): `cargo test --lib`
2. **Mock server tests** (10 min): `cargo test --test mock_server_test`
3. **Integration tests** (15 min): `cargo test --test integration_test`
4. **Performance tests** (5 min): `cargo bench`
5. **E2E with Claude CLI** (10 min): Run manual script
6. **Security tests** (5 min): `cargo test --test security_test`

**Total Testing Time**: ~50 minutes for full test suite

### Implementation Plan

#### Phase 1: Create Proxy Crate

1. Extend `RuleFrontmatter` struct in `swissarmyhammer-rules/src/frontmatter.rs`:
```rust
pub struct ToolFilter {
    allowed_patterns: Vec<Regex>,
    denied_patterns: Vec<Regex>,
}

pub struct RuleFrontmatter {
    // existing fields...
    tool_filter: Option<ToolFilter>,
}
```

2. Add parsing logic for `allowed_tools_regex` and `denied_tools_regex` fields

#### Phase 2: Filtering Proxy

1. Create `swissarmyhammer-tools/src/mcp/filtering_proxy.rs`:
```rust
pub struct FilteringMcpProxy {
    inner: Arc<McpServer>,
    tool_filter: ToolFilter,
}

impl FilteringMcpProxy {
    fn is_tool_allowed(&self, tool_name: &str) -> bool {
        // Check denied patterns first (deny takes precedence)
        if self.tool_filter.denied_patterns.iter().any(|p| p.is_match(tool_name)) {
            return false;
        }
        
        // If no allowed patterns specified, allow all (except denied)
        if self.tool_filter.allowed_patterns.is_empty() {
            return true;
        }
        
        // Check allowed patterns
        self.tool_filter.allowed_patterns.iter().any(|p| p.is_match(tool_name))
    }
}
```

#### Phase 3: Integration with RuleChecker

1. Modify `RuleChecker::check_file()` to use proxy when rule has tool filter:
```rust
let agent = if let Some(tool_filter) = rule.get_tool_filter() {
    // Create filtering proxy
    let proxy = FilteringMcpProxy::new(self.mcp_server.clone(), tool_filter.clone());
    
    // Create agent with filtered tools
    create_agent_with_proxy(proxy, agent_config)
} else {
    // Use default agent
    self.default_agent.clone()
};
```

#### Phase 4: System Prompt Enhancement

1. Add tool filter info to system prompt in `.check` template:
```handlebars
{{#if allowed_tools}}
You may only use these tools: {{allowed_tools}}.
Do not attempt to use any other tools.
{{/if}}
```

## Alternative: Simpler Per-Check Server

If proxying is too complex, consider simpler approach:

**Spawn temporary filtered MCP server per rule check batch**:

```rust
// For rules with tool filters, batch them together
let rules_with_same_filter = group_by_tool_filter(rules);

for (tool_filter, rules) in rules_with_same_filter {
    // Spawn temporary server with filtered registry
    let filtered_server = spawn_filtered_mcp_server(tool_filter);
    
    // Check all rules with same filter
    for rule in rules {
        check_with_server(rule, filtered_server).await?;
    }
    
    // Shutdown temporary server
    filtered_server.shutdown().await;
}
```

**Pros**:
- Simpler than proxying
- True isolation
- Amortize server startup across multiple rules

**Cons**:
- Still need server lifecycle management
- May be slower if rules have different filters

## Open Questions

1. **Default behavior**: If rule has no `allowed_tools` specified, allow all tools?
2. **Override mechanism**: Should CLI have flag to ignore tool filters for debugging?
3. **Performance**: Is per-rule server spawning acceptable, or must we use single server?
4. **Security**: Do we need both allow-list AND deny-list, or just one?
5. **Tool discovery**: Should filtered tools be hidden from `listTools`, or just blocked on `callTool`?

## Testing Strategy

1. **Unit tests**: Test regex matching logic for tool filters
2. **Integration tests**: Test rule checking with various tool filter configurations
3. **Security tests**: Verify rules cannot bypass tool filters
4. **Performance tests**: Measure overhead of filtering approach

## Next Steps

1. Decide on primary implementation approach (proxy vs per-check server)
2. Prototype frontmatter parsing for tool filters
3. Implement filtering mechanism
4. Update rule checking flow to use filtered tools
5. Document feature in rule authoring guide
