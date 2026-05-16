---
assignees:
- claude-code
depends_on:
- 01KRRE4PJV0N1GSE92MF45GGPV
position_column: todo
position_ordinal: '8780'
project: plugin-arch
title: 'plugin: InProcessServer&lt;S&gt; rmcp adapter'
---
## What
Implement `InProcessServer<S>` — the adapter that wraps any `rmcp::ServerHandler` as an `McpServer` with no serialization and no IPC. This is the transport for host Rust code.

In `crates/swissarmyhammer-plugin/src/server.rs` (or a `server/in_process.rs` submodule):
- `pub struct InProcessServer<S> { inner: Arc<S> }` with `new(S)` and `from_arc(Arc<S>)` constructors.
- `impl<S: rmcp::ServerHandler + Send + Sync + 'static> McpServer for InProcessServer<S>`:
  - `tools()` → enumerate the rmcp router's tool descriptors into `Vec<ToolMetadata>`, carrying `name`/`description`/`inputSchema`/`_meta`. Cache once at construction (rmcp `list_tools` is a router descriptor enumeration).
  - `invoke(caller, tool, input)` → build an rmcp `CallToolRequestParam { name: tool, arguments: input.as_object().cloned() }`, construct a `RequestContext::<RoleServer>`, insert `caller` into `context.extensions` (so rmcp handlers can fetch `CallerId`), call `inner.call_tool(...)`, convert the `CallToolResult` back into a `Value`.
  - The adapter does NOT parse or special-case `op` — that is the tool handler's job.

## Acceptance Criteria
- [ ] `InProcessServer<S>` exists with `new`/`from_arc`; implements `McpServer`.
- [ ] `tools()` returns the wrapped handler's tool list including `_meta` for operation tools.
- [ ] `invoke` round-trips a `tools/call` to the rmcp handler and converts the result to `Value`.
- [ ] `CallerId` is placed in `RequestContext::extensions` and is fetchable by a handler.

## Tests
- [ ] Write a minimal real rmcp server in the test module (`#[tool_router]` / `#[tool]` with one flat tool that echoes its input). Wrap it in `InProcessServer`, register it, and assert `invoke` returns the echoed value — proving the no-IPC path works end to end.
- [ ] Test that a `#[tool]` handler reading `ctx.extensions.get::<CallerId>()` sees the caller the adapter inserted.
- [ ] Run: `cargo test -p swissarmyhammer-plugin` — all green.

## Workflow
- Use `/tdd` — write the real-rmcp-server round-trip test first, then implement the adapter.

## Depends on
McpServer trait + ToolMetadata.