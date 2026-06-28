---
assignees:
- claude-code
depends_on:
- 01KRRE4PJV0N1GSE92MF45GGPV
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffffff480
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
- [x] `InProcessServer<S>` exists with `new`/`from_arc`; implements `McpServer`.
- [x] `tools()` returns the wrapped handler's tool list including `_meta` for operation tools.
- [x] `invoke` round-trips a `tools/call` to the rmcp handler and converts the result to `Value`.
- [x] `CallerId` is placed in `RequestContext::extensions` and is fetchable by a handler.

## Tests
- [x] Write a minimal real rmcp server in the test module (`#[tool_router]` / `#[tool]` with one flat tool that echoes its input). Wrap it in `InProcessServer`, register it, and assert `invoke` returns the echoed value — proving the no-IPC path works end to end.
- [x] Test that a `#[tool]` handler reading `ctx.extensions.get::<CallerId>()` sees the caller the adapter inserted.
- [x] Run: `cargo test -p swissarmyhammer-plugin` — all green.

## Workflow
- Use `/tdd` — write the real-rmcp-server round-trip test first, then implement the adapter.

## Depends on
McpServer trait + ToolMetadata.

## Implementation notes
- rmcp 1.5.0. `RequestContext<RoleServer>` requires a `Peer<RoleServer>`, whose constructor (`Peer::new`) is `pub(crate)`. The only public route to a `Peer` is `rmcp::service::serve_directly`, which consumes a handler + transport. The adapter mints a `Peer` once at construction by serving a throwaway `PeerProbe` handler over an immediately-closing in-memory `ClosedTransport` (its `receive()` returns `None`, so rmcp's service loop exits at once). The minted `Peer` is an inert routing token reused for every `RequestContext`; the real call path never crosses a transport.
- `tools()` is sync and cannot await rmcp's async `list_tools`, so the constructors are `async`: they call `list_tools` once and cache the result. `new`/`from_arc` return `Result<Self>`.
- `invoke` checks the cached tool list first (`UnknownTool` when absent), then builds `CallToolRequestParams`, threads `CallerId` into `RequestContext::extensions`, calls `inner.call_tool` directly, and `serde_json::to_value`s the `CallToolResult`.