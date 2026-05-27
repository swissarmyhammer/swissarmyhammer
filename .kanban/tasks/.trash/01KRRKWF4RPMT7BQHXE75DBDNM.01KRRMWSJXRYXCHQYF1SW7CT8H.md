---
assignees:
- claude-code
depends_on: []
position_column: todo
position_ordinal: '8580'
project: ai-panel
title: AiSession — create agent, connect Client, run ACP session
---
## What
Build the per-window ACP session — stateless: a fresh `new_session` each time, no transcript persistence.

- Create `apps/kanban-app/src/ai/session.rs`. Define `AiSession { window_label, board_path, model_id, connection: ConnectionTo<Agent>, acp_session_id, client_task, cancel }`.
- Add `ai_sessions: RwLock<HashMap<String /*window_label*/, AiSession>>` to `AppState` in `apps/kanban-app/src/state.rs`.
- Session create flow: resolve the window's board -> its `KanbanHttpServer` URL; load the `ModelConfig`; `swissarmyhammer_agent::create_agent(&model_config, None)` (mcp_config = None — MCP travels over ACP); build the Client via `ai/client.rs` and `connect_with` the agent component -> `ConnectionTo<Agent>`.
- Drive ACP: `initialize` (honest `ClientCapabilities`) -> `new_session` with `mcp_servers` = [ the board's kanban HTTP MCP server ] -> `set_session_mode("code")`.
- Teardown: dropping `AiSession` closes the `ConnectionTo<Agent>` (terminates the agent subprocess) and stops `client_task`.

This task stops at an established session — `prompt` and notification streaming are the next task.

Spec: `ideas/kanban/ai_panel.md` — Phase 2 "Per-window session", "Session lifecycle".

## Acceptance Criteria
- [ ] Creating an `AiSession` produces a connected `ConnectionTo<Agent>` and a valid `acp_session_id`.
- [ ] `new_session` is issued with the board's kanban MCP server in `mcp_servers`.
- [ ] `create_agent` is called with `mcp_config: None` — no MCP config baked in at creation.
- [ ] Dropping an `AiSession` closes the ACP connection and stops the client task.
- [ ] `cargo build -p kanban-app` is clean.

## Tests
- [ ] Integration test: create an `AiSession` against an in-process test ACP Agent; assert `initialize` + `new_session` + `set_session_mode` complete and `mcp_servers` carried the kanban URL.
- [ ] Test that dropping the session closes the connection (the agent task ends).
- [ ] `cargo test -p kanban-app` is green.

## Workflow
- Use `/tdd` — write the session-establishment test against a test ACP Agent first.