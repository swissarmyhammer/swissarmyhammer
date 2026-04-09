---
assignees:
- claude-code
depends_on:
- 01KNS10MMDVZG731XKM390C682
position_column: todo
position_ordinal: ab80
project: kanban-mcp
title: 'kanban-cli: implement serve.rs — KanbanMcpServer over stdio'
---
## What

Create `kanban-cli/src/serve.rs` implementing a minimal `rmcp::ServerHandler` that exposes the single `kanban` operation tool over stdio.

Build directly on `swissarmyhammer-kanban` (already a dependency), NOT on `swissarmyhammer-tools::KanbanTool`, to avoid the `ToolContext`/`ToolHandlers` dependency.

```rust
pub struct KanbanMcpServer;

impl ServerHandler for KanbanMcpServer {
    fn get_info(&self) -> ServerInfo { ... }
    async fn list_tools(...) -> Result<ListToolsResult, McpError> {
        // Use swissarmyhammer_kanban::schema::generate_kanban_mcp_schema to build Tool
    }
    async fn call_tool(request, ...) -> Result<CallToolResult, McpError> {
        // parse_input(request.arguments) → execute_operation(&ctx, op)
        // ctx: KanbanContext::new(cwd/.kanban)
    }
}

pub async fn run_serve() -> Result<(), String> {
    let server = KanbanMcpServer;
    let running = serve_server(server, stdio()).await...;
    running.waiting().await...;
}
```

The `call_tool` handler must:
1. Get current working dir → `.kanban` path
2. `parse_input(arguments.unwrap_or_default())`
3. `execute_operation(&ctx, op)` for each parsed op
4. Return results as `CallToolResult` with text content

Model error handling on `shelltool-cli/src/serve.rs`.

## Acceptance Criteria
- [ ] `kanban serve` starts without panicking and speaks MCP over stdio
- [ ] `cargo check -p kanban-cli` passes
- [ ] A client calling `tools/list` receives exactly one tool named `kanban`
- [ ] A client calling `tools/call` with `{op: "list tasks"}` returns a valid response

## Tests
- [ ] Unit test in `kanban-cli/src/serve.rs`: `KanbanMcpServer::get_info()` returns correct server name
- [ ] Unit test: `KanbanMcpServer` list_tools returns a single tool named `"kanban"`
- [ ] Test file: `kanban-cli/src/serve.rs` in `#[cfg(test)]` module

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.
