---
assignees:
- claude-code
position_column: todo
position_ordinal: '80'
project: ai-panel
title: In-process ACP agent served over a localhost WebSocket
---
## What
The ACP agent runs **inside the kanban-app process** — not as an external subprocess. The Tauri Rust backend builds the ACP agent in-process and exposes it on a loopback WebSocket; the webview's TypeScript ACP client connects to that WebSocket. Tauri IPC is NOT in the ACP data path — the data path is a plain WebSocket.

- New `apps/kanban-app/src/ai/agent_ws.rs`. Add to `apps/kanban-app/Cargo.toml`: `swissarmyhammer-agent`, `agent-client-protocol`, `tokio-tungstenite` (workspace deps).
- Host a loopback WebSocket server (`tokio-tungstenite`, bind `127.0.0.1:0`); report the bound port.
- On a WebSocket connection, build the ACP agent in-process via `swissarmyhammer_agent::create_agent(&model_config, None)` — `mcp_config` is `None`; the kanban MCP server is delivered over ACP in `newSession.mcpServers`, never baked in. `create_agent` dispatches Claude Code (claude-agent) vs local llama (llama-agent).
- Run the ACP **agent side** over the WebSocket: adapt the WebSocket to the `AsyncRead`/`AsyncWrite` byte stream the agent server consumes, with newline-delimited JSON framing. The agent servers already expose a `start_with_streams(read, write)` stdio pattern (`llama_agent::acp::AcpServer`, claude-agent's server) — adapt that to the WS stream (e.g. via `ws_stream_tungstenite` or a small adapter).
- Gate `llama-agent` behind a Cargo feature (`ai-local-models`, default off) so the standard build stays light — claude-agent is always available; local llama is opt-in.

## Acceptance Criteria
- [ ] A loopback WebSocket server runs in the kanban-app process; a WebSocket client connecting to it completes the ACP `initialize` handshake against an in-process agent.
- [ ] The agent is built via `swissarmyhammer_agent::create_agent`; Claude Code works without the `ai-local-models` feature.
- [ ] No external agent subprocess — the agent runs in the kanban-app process (claude-agent spawning `claude` internally is fine).
- [ ] `cargo build -p kanban-app` is clean both with and without `--features ai-local-models`.

## Tests
- [ ] Integration test (`apps/kanban-app/tests/`): start the WS agent server, connect a WebSocket client, send `initialize`, assert a valid ACP `initialize` response with a negotiated protocol version.
- [ ] Build/feature test: `kanban-app` builds with and without `ai-local-models`.
- [ ] `cargo test -p kanban-app` is green.

## Workflow
- Use `/tdd` — write the WebSocket `initialize` round-trip test first.