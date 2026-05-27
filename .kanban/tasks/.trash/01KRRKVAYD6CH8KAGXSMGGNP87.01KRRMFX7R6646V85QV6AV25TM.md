---
assignees:
- claude-code
depends_on: []
position_column: todo
position_ordinal: '8380'
project: ai-panel
title: Wire KanbanHttpServer into BoardHandle lifecycle + get_kanban_mcp_url command
---
## What
Give every open board its own running `KanbanHttpServer`, and expose its URL to the webview.

- In `apps/kanban-app/src/state.rs`: add a `kanban_http: KanbanHttpServer` field to `BoardHandle`. Start the server in `BoardHandle::open` (pass the board's `Arc<KanbanContext>`). Stop it when the `BoardHandle` is dropped — extend the existing `Drop for BoardHandle` (which already aborts `bridge_task`) to cancel the server's `cancellation_token`.
- Add a Tauri command `get_kanban_mcp_url(board_path: String) -> Result<String, String>` in `apps/kanban-app/src/commands.rs`: resolve the board via `AppState`, return its `KanbanHttpServer::url()`. Register the command in the `tauri::generate_handler!` list in `apps/kanban-app/src/main.rs`.

Spec: `ideas/kanban/ai_panel.md` — Phase 1, "What Phase 1 delivers".

## Acceptance Criteria
- [ ] Opening a board starts its `KanbanHttpServer`; the server is reachable while the board is open.
- [ ] Closing a board (dropping the `BoardHandle`) stops that board's server.
- [ ] `get_kanban_mcp_url` returns the `http://127.0.0.1:<port>/mcp` URL for an open board and an error for an unknown board.
- [ ] `cargo build -p kanban-app` is clean.

## Tests
- [ ] Unit/integration test (`apps/kanban-app/`): open a `BoardHandle`, assert its `kanban_http.url()` resolves and an HTTP `kanban` call mutates the board; drop the handle, assert the server no longer accepts connections.
- [ ] Test `get_kanban_mcp_url` against `AppState` with an open board (Ok) and an unknown path (Err).
- [ ] `cargo test -p kanban-app` is green.

## Workflow
- Use `/tdd` — write the lifecycle test (open -> reachable, drop -> stopped) first.