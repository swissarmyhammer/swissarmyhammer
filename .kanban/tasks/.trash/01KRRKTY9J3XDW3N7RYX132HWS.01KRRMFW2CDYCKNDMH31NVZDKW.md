---
assignees:
- claude-code
depends_on: []
position_column: todo
position_ordinal: '8280'
project: ai-panel
title: KanbanHttpServer — per-board in-process HTTP MCP server
---
## What
Build the in-process HTTP MCP server that exposes the `kanban` tool to an ACP agent. ACP delivers MCP servers as HTTP endpoints, and `StreamableHttpService`'s service factory receives no request context — so there is ONE server PER OPEN BOARD, each on its own random loopback port, with the board bound into the factory by `move`. (A single server keyed by URL path does not work — see spec.)

- Create `apps/kanban-app/src/ai/mod.rs` and `apps/kanban-app/src/ai/kanban_http.rs`; add `mod ai;` to `apps/kanban-app/src/main.rs`.
- `KanbanMcpHandler` — an `rmcp::ServerHandler` holding an `Arc<KanbanContext>`. `list_tools` returns the shared `build_list_tools_result()`; `call_tool` parses input and runs `swissarmyhammer_kanban::dispatch::execute_operation(&ctx, op)`, mapping errors with the shared `classify_kanban_error` (both from task "Extract kanban MCP tool schema...").
- `KanbanHttpServer` — binds `TcpListener` to `127.0.0.1:0`, records the port, builds `StreamableHttpService::new(move || Ok(KanbanMcpHandler::new(Arc::clone(&ctx))), LocalSessionManager::default().into(), StreamableHttpServerConfig::default())`, mounts it on `axum::Router::new().nest_service("/mcp", svc)`, spawns `axum::serve`. Holds the `cancellation_token` so the server can be stopped.
- Add `rmcp` (streamable-http-server feature), `axum`, and any needed `tokio` features to `apps/kanban-app/Cargo.toml` — all already in the workspace lock.

Reference implementation: `crates/agent-client-protocol-extras/src/test_mcp_server.rs`. Spec: `ideas/kanban/ai_panel.md` — Phase 1.

## Acceptance Criteria
- [ ] `KanbanHttpServer::start(Arc<KanbanContext>)` returns a running server bound to a random `127.0.0.1` port and exposes `url()` -> `http://127.0.0.1:<port>/mcp`.
- [ ] The service factory `move`-captures the board's live `KanbanContext` — no fresh context per call.
- [ ] Dropping / cancelling the server stops accepting connections.
- [ ] `cargo build -p kanban-app` is clean.

## Tests
- [ ] Integration test (`apps/kanban-app/tests/`): start a `KanbanHttpServer` over a temp board, connect an `rmcp` HTTP MCP client, call `kanban` with `op: "add task"`, then assert the task is present in the SAME `KanbanContext` (live dispatch, not a fresh context).
- [ ] Test that `list_tools` returns exactly one tool named `kanban`.
- [ ] Test that a cancelled server rejects new connections.
- [ ] `cargo test -p kanban-app` is green.

## Workflow
- Use `/tdd` — write the failing HTTP round-trip test first, then implement the server to make it pass.