---
assignees:
- claude-code
depends_on:
- 01KNS1WP3ZEAKNNAD6G3WAGSEK
position_column: done
position_ordinal: ffffffffffffffffffffffffff9d80
project: code-context-cli
title: Implement serve command — CodeContextServer MCP server
---
## What
Create `code-context-cli/src/serve.rs` implementing an MCP server that exposes `CodeContextTool` over stdio, mirroring `shelltool-cli/src/serve.rs`.

```rust
pub struct CodeContextServer {
    tool: CodeContextTool,
    context: ToolContext,
}
```

- `CodeContextServer::new()` — construct with `ToolContext::new(Arc::new(ToolHandlers::new()), Arc::new(Mutex::new(None)), Arc::new(ModelConfig::default()))`
- Implement `rmcp::ServerHandler`:
  - `get_info()` → `ServerInfo::new(ServerCapabilities::builder().enable_tools().build()).with_server_info(Implementation::new("code-context", env!("CARGO_PKG_VERSION")))`
  - `list_tools()` → wrap `CodeContextTool.schema()` in a single `Tool`
  - `call_tool()` → dispatch to `CodeContextTool.execute()`
- `pub async fn run_serve() -> Result<(), String>` — same pattern as shelltool's `run_serve()`

Key import: `swissarmyhammer_tools::mcp::tools::code_context::CodeContextTool`

## Acceptance Criteria
- [ ] `cargo check -p code-context-cli` passes
- [ ] `run_serve()` compiles as an async fn returning `Result<(), String>`
- [ ] `CodeContextServer` implements `Clone`

## Tests
- [ ] `test_new` — `CodeContextServer::new()` doesn't panic
- [ ] `test_get_info_has_server_name` — verify `get_info()` contains "code-context"
- [ ] Run `cargo test -p code-context-cli serve` — all pass

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.