---
assignees:
- claude-code
position_column: todo
position_ordinal: '9380'
project: ai-panel
title: Add a per-launch auth token to the in-process WebSocket ACP agent
---
## What

The in-process loopback `ws://127.0.0.1:<port>` ACP agent server (`apps/kanban-app/src/ai/agent_ws.rs`) currently accepts ANY local connection. The accept loop performs no origin or auth check, so any local process that discovers the OS-assigned ephemeral port could connect and drive an in-process agent. The only mitigation today is loopback-only binding, which keeps it off the network but does not isolate it from other local processes.

Harden the channel with a per-launch auth token so only the app's own webview can connect.

## Acceptance Criteria
- [ ] `AgentWebSocketServer::bind_with` (or `bind`) mints a fresh, cryptographically random per-launch token.
- [ ] `ai_start_agent` includes the token in the `wsUrl` handed to the webview — e.g. as a `ws://127.0.0.1:<port>?token=<secret>` query parameter or as a WebSocket subprotocol.
- [ ] The WebSocket server rejects any connection that does not present the correct token (close the connection / fail the upgrade before running the ACP protocol).
- [ ] The TypeScript ACP client passes the token when opening the connection (reads it from the `wsUrl` returned by `ai_start_agent`).
- [ ] Unit/integration test: a connection presenting the wrong/no token is rejected; a connection presenting the correct token completes `initialize`.

## Context
This is the deferred follow-up to review finding on `01KRRN3SP5D1H63TQ8HM7SQZ1F` ("Model selection and the AI agent endpoint command surface"). That task wired the server into Tauri startup but, per the reviewer's accepted resolution, deferred the token handshake to this separate tracked task rather than expanding its scope. The `run()` doc comment in `agent_ws.rs` references this task id as the place the token work is tracked.

## Tests
- [ ] `cargo test -p kanban-app` is green.
- [ ] `cargo clippy -p kanban-app --all-targets -- -D warnings` is clean. #security