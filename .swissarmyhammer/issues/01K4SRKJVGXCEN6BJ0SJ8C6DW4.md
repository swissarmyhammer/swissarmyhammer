# Consolidate and Clean Up MCP Server Implementation

## Problem
The MCP server implementation is **fragmented and inconsistent** across multiple locations with overlapping functionality. We have:

1. **Stdio MCP server** - Split between `swissarmyhammer-tools` and `swissarmyhammer-cli`
2. **HTTP MCP server** - Degenerate implementation in `swissarmyhammer-tools`
3. **Scattered functionality** - MCP server logic spread across multiple crates
4. **Reimplementation issues** - Not clearly using `rmcp` library consistently

## Current Fragmented State

### **Stdio MCP Server (Split Implementation)**
- `swissarmyhammer-cli/src/commands/serve/mod.rs` - CLI serve command with stdio transport
- Uses `rmcp::transport::io::stdio` (correct rmcp usage)
- Handles graceful shutdown and CLI integration

### **HTTP MCP Server (Degenerate Implementation)**  
- `swissarmyhammer-tools/src/mcp/http_server.rs` - HTTP transport implementation
- Custom HTTP handling with axum
- May not be using rmcp properly for HTTP transport
- Limited/incomplete functionality

### **MCP Server Core**
- `swissarmyhammer-tools/src/mcp/server.rs` - Core MCP server logic
- Tool registration and handling
- Shared between both stdio and HTTP implementations

## Proposed Solution
Create a **unified, clean MCP server** that:

1. **Uses rmcp clearly** without reimplementing MCP protocol functionality
2. **Supports enumerated modes**:
   - `stdio` - Standard input/output transport
   - `http(known_port)` - HTTP server on specified port  
   - `http(random_port)` - HTTP server on random port, returns connection info
3. **Consolidated in single location** - Not split across multiple crates
4. **Clean separation** - Core server logic separate from transport layers

## Implementation Plan

### Phase 1: Analyze Current MCP Server Implementations
- [ ] Review `swissarmyhammer-cli/src/commands/serve/mod.rs` stdio implementation
- [ ] Review `swissarmyhammer-tools/src/mcp/http_server.rs` HTTP implementation  
- [ ] Review `swissarmyhammer-tools/src/mcp/server.rs` core server logic
- [ ] Identify what functionality is properly using rmcp vs reimplementing
- [ ] Map out current transport handling and tool registration

### Phase 2: Design Unified MCP Server Architecture
- [ ] Define clean interface for MCP server with enumerated modes:
  ```rust
  pub enum McpServerMode {
      Stdio,
      Http { port: Option<u16> }, // None = random port
  }
  
  pub struct McpServerConfig {
      mode: McpServerMode,
      // ... other config
  }
  
  pub struct McpServerInfo {
      mode: McpServerMode,
      connection_info: ConnectionInfo, // Port for HTTP, etc.
  }
  ```

### Phase 3: Consolidate Core MCP Server Logic
- [ ] Decide where unified MCP server should live (recommend: swissarmyhammer-tools)
- [ ] Consolidate core server logic from existing implementations
- [ ] Ensure proper rmcp usage throughout - no protocol reimplementation
- [ ] Create clean separation between server core and transport layers

### Phase 4: Implement Clean Transport Layers

#### **Stdio Transport**
- [ ] Use `rmcp::transport::io::stdio` properly
- [ ] Move stdio-specific logic from CLI to unified server
- [ ] Ensure clean integration with rmcp stdio transport

#### **HTTP Transport**  
- [ ] Research proper rmcp HTTP transport usage
- [ ] Replace custom axum implementation with proper rmcp HTTP if available
- [ ] If rmcp doesn't provide HTTP, implement minimal clean HTTP wrapper
- [ ] Support both known and random port modes

### Phase 5: Update CLI Integration
- [ ] Update `swissarmyhammer-cli/src/commands/serve/mod.rs` to use unified server
- [ ] Remove duplicate MCP server logic from CLI
- [ ] CLI should just configure and start the unified server
- [ ] Preserve all current CLI functionality and interface

### Phase 6: Clean Up Degenerate HTTP Implementation
- [ ] Remove or replace `swissarmyhammer-tools/src/mcp/http_server.rs`
- [ ] Eliminate custom HTTP MCP protocol handling
- [ ] Replace with proper rmcp-based implementation
- [ ] Ensure HTTP transport works correctly

### Phase 7: Implement Mode Enumeration and Response
- [ ] Server should return connection information based on mode:
  ```rust
  match mode {
      McpServerMode::Stdio => McpServerInfo { mode, connection_info: ConnectionInfo::Stdio },
      McpServerMode::Http { port: Some(p) } => McpServerInfo { mode, connection_info: ConnectionInfo::Http { port: p } },
      McpServerMode::Http { port: None } => McpServerInfo { mode, connection_info: ConnectionInfo::Http { port: actual_bound_port } },
  }
  ```

### Phase 8: Testing and Verification
- [ ] Test stdio mode works correctly with rmcp
- [ ] Test HTTP mode with known port
- [ ] Test HTTP mode with random port assignment
- [ ] Verify connection information is returned correctly
- [ ] Test integration with existing CLI and workflow usage
- [ ] Ensure no regressions in MCP functionality

## Success Criteria

### **Unified MCP Server Should:**
- [ ] Use rmcp library properly without reimplementing MCP protocol
- [ ] Support stdio transport mode
- [ ] Support HTTP transport with known port
- [ ] Support HTTP transport with random port (returning actual port)
- [ ] Return clear connection information for each mode
- [ ] Be consolidated in single location (not fragmented)
- [ ] Have clean separation between core logic and transport layers

### **Elimination Targets:**
- [ ] Remove fragmented stdio/HTTP implementations
- [ ] Remove custom MCP protocol handling
- [ ] Remove degenerate HTTP server implementation
- [ ] Consolidate scattered MCP server logic

## COMPLETION CRITERIA - How to Know This Issue is REALLY Done

**This issue is complete when:**

1. **Single unified MCP server implementation** exists
2. **Clean rmcp usage** without protocol reimplementation
3. **Enumerated modes work**: stdio, http(known), http(random)
4. **Connection info returned** appropriately for each mode
5. **Fragmented implementations removed**

```bash
# Should have clean, unified implementation:
find /Users/wballard/github/sah -name "*mcp*server*" -type f

# Should use rmcp properly:
rg "use rmcp::" swissarmyhammer-tools/src/mcp/
```

## Benefits
- **Clean Architecture**: Unified server instead of fragmented implementations
- **Proper rmcp Usage**: Library used correctly without reimplementation  
- **Flexible Deployment**: Support for multiple transport modes
- **Better Maintainability**: Single implementation to maintain
- **Clear Interface**: Enumerated modes with clear connection information

## Files to Investigate/Consolidate
- `swissarmyhammer-cli/src/commands/serve/mod.rs` - Stdio implementation
- `swissarmyhammer-tools/src/mcp/http_server.rs` - Degenerate HTTP implementation
- `swissarmyhammer-tools/src/mcp/server.rs` - Core server logic
- Any other scattered MCP server functionality

## Notes
This will create a clean, professional MCP server implementation that properly leverages the rmcp library instead of reimplementing MCP protocol functionality. The enumerated mode approach will provide clear, flexible deployment options while returning appropriate connection information.

The goal is a single, well-designed MCP server that can be deployed in multiple modes without fragmentation or protocol reimplementation.
## rmcp HTTP Transport - Simple Implementation Pattern

Based on the official rmcp example at https://github.com/modelcontextprotocol/rust-sdk/blob/main/examples/servers/src/counter_streamhttp.rs, implementing HTTP MCP transport is **extremely straightforward**:

```rust
use rmcp::transport::streamable_http_server::{
    StreamableHttpService, 
    session::local::LocalSessionManager,
};

// Simple HTTP MCP server setup:
let service = StreamableHttpService::new(
    || Ok(Counter::new()), // Server factory function
    LocalSessionManager::default().into(),
    Default::default(),
);

let router = axum::Router::new().nest_service("/mcp", service);
let tcp_listener = tokio::net::TcpListener::bind("127.0.0.1:8000").await?;
axum::serve(tcp_listener, router)
    .with_graceful_shutdown(async { tokio::signal::ctrl_c().await.unwrap() })
    .await;
```

## Key Insights from Official Example:

### **✅ No Protocol Reimplementation Needed:**
- `StreamableHttpService` handles all MCP protocol over HTTP
- Just need to provide server factory function and session manager
- rmcp handles all the transport layer complexity

### **✅ Simple Axum Integration:**
- Use `Router::new().nest_service("/mcp", service)`
- Standard Rust HTTP patterns
- No custom MCP protocol handling needed

### **✅ Standard Port Binding:**
- Use `TcpListener::bind()` for port management
- Can bind to specific port or use 0 for random port assignment
- Get actual bound port from listener for random port mode

## Updated Implementation Approach

### **Simplified Architecture Using rmcp Patterns:**
```rust
pub enum McpServerMode {
    Stdio,
    Http { port: Option<u16> },
}

// Server factory function (our existing MCP server)
fn create_mcp_server() -> Result<OurMcpServer> {
    // Existing server creation logic
}

// For HTTP mode:
let service = StreamableHttpService::new(
    create_mcp_server,
    LocalSessionManager::default().into(),
    Default::default(),
);
```

### **Port Handling for Random Ports:**
```rust
// For random port (port = 0):
let tcp_listener = TcpListener::bind("127.0.0.1:0").await?;
let actual_port = tcp_listener.local_addr()?.port();

// Return connection info:
McpServerInfo {
    mode: McpServerMode::Http { port: Some(actual_port) },
    connection_info: format!("http://127.0.0.1:{}/mcp", actual_port),
}
```

## Revised Implementation Plan

### **Phase 1-3: [Keep previous analysis phases]**

### **Phase 4: Implement Clean Transport Layers Using rmcp**

#### **Stdio Transport (Already Good)**
- ✅ Current `swissarmyhammer-cli` uses `rmcp::transport::io::stdio` correctly
- Keep this pattern, just consolidate location

#### **HTTP Transport (Use rmcp Pattern)**
- [ ] Replace custom axum implementation with `StreamableHttpService`
- [ ] Use `LocalSessionManager` for session handling
- [ ] Implement server factory function that returns our MCP server
- [ ] Use standard `TcpListener::bind()` for port management
- [ ] No custom MCP protocol handling needed

### **Phase 5: [Update rest of plan with rmcp-based approach]**

## Benefits of rmcp StreamableHttpService Approach
- ✅ **No Protocol Reimplementation**: rmcp handles all MCP over HTTP
- ✅ **Simple Integration**: Standard axum router patterns  
- ✅ **Official Pattern**: Uses the recommended rmcp approach
- ✅ **Minimal Code**: Very few lines of transport code needed
- ✅ **Port Management**: Standard Rust patterns for port binding/discovery

This shows our current HTTP implementation is likely **over-engineered** and should be replaced with the simple rmcp `StreamableHttpService` pattern.