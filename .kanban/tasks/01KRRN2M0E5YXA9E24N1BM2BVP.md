---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffed80
project: ai-panel
title: In-process ACP agent served over a localhost WebSocket
---
## What
The ACP agent runs **inside the kanban-app process** — not as an external subprocess. The Tauri Rust backend builds the ACP agent in-process and exposes it on a loopback WebSocket; the webview's TypeScript ACP client connects to that WebSocket. Tauri IPC is NOT in the ACP data path — the data path is a plain WebSocket.

- New `apps/kanban-app/src/ai/agent_ws.rs`. Add to `apps/kanban-app/Cargo.toml`: `swissarmyhammer-agent`, `agent-client-protocol`, `tokio-tungstenite` (workspace deps).
- Host a loopback WebSocket server (`tokio-tungstenite`, bind `127.0.0.1:0`); report the bound port.
- On a WebSocket connection, build the ACP agent in-process via `swissarmyhammer_agent::create_agent(&model_config, None)` — `mcp_config` is `None`; the kanban MCP server is delivered over ACP in `newSession.mcpServers`, never baked in. `create_agent` dispatches Claude Code (claude-agent) vs local llama (llama-agent).
- Run the ACP **agent side** over the WebSocket: adapt the WebSocket to the `AsyncRead`/`AsyncWrite` byte stream the agent server consumes, with newline-delimited JSON framing. The agent servers already expose a `start_with_streams(read, write)` stdio pattern (`llama_agent::acp::AcpServer`, claude-agent's server) — adapt that to the WS stream (e.g. via `ws_stream_tungstenite` or a small adapter).
- Claude vs local-llama dispatch is decided at runtime by `ModelConfig::executor_type()`; no Cargo feature gate is used (per `ARCHITECTURE.md` §1 Practices #1 — no feature flags).

## Acceptance Criteria
- [x] A loopback WebSocket server runs in the kanban-app process; a WebSocket client connecting to it completes the ACP `initialize` handshake against an in-process agent.
- [x] The agent is built via `swissarmyhammer_agent::create_agent`; Claude Code works in the default build (no compile-time gate — `create_agent` dispatches Claude vs llama at runtime).
- [x] No external agent subprocess — the agent runs in the kanban-app process (claude-agent spawning `claude` internally is fine).
- [x] `cargo build -p kanban-app` is clean. (The `ai-local-models` feature was removed during review per `ARCHITECTURE.md` §1 Practices #1; the original "builds with and without the feature" criterion is therefore moot — there is one build.)

## Tests
- [x] Integration test (`apps/kanban-app/tests/`): start the WS agent server, connect a WebSocket client, send `initialize`, assert a valid ACP `initialize` response with a negotiated protocol version.
- [x] Build test: `kanban-app` builds clean. (Removed `ai-local-models`, so there is no longer a with/without-feature build matrix — see the acceptance criterion above.)
- [x] `cargo test -p kanban-app` is green.

## Workflow
- Use `/tdd` — write the WebSocket `initialize` round-trip test first.

## Implementation Notes
- `swissarmyhammer_agent::create_agent` returns an `AcpAgentHandle` whose `.agent` is a `DynConnectTo<Client>` (the ACP 0.11 builder/handler runtime), not a `start_with_streams` server. The agent is served over the byte stream via `ConnectTo::<Agent>::connect_to(transport, handle.agent)`.
- Each WebSocket text frame carries exactly one JSON-RPC message, so the WS↔`Lines` transport adapter maps text frames directly to/from JSON-RPC line strings — no byte-level newline scanning and no `ws_stream_tungstenite` dependency needed.
- No Cargo feature gates the local-llama backend. `swissarmyhammer-agent` depends on `llama-agent` unconditionally, and `ARCHITECTURE.md` §1 Practices #1 forbids feature flags (only `test-support` is exempt). `create_agent` already dispatches Claude vs llama at runtime via `ModelConfig::executor_type()`, so no compile-time gate is needed. (The originally-planned `ai-local-models` feature was added then removed during review — it gated no code and was inert because `llama-agent` compiled in regardless.)
- `agent_ws::AgentWebSocketServer` is built but not yet started from the Tauri `setup_app` hook — wiring it into app startup and handing the bound port to the webview is the follow-up task (`01KRRN3SP5D1H63TQ8HM7SQZ1F`). Until then `ai/mod.rs` carries a module-wide `#![allow(dead_code)]`.

## Review Findings (2026-05-16 16:40)

Mode: task-mode. Scope: branch `kanban` vs `main` — `apps/kanban-app/src/ai/agent_ws.rs`, `apps/kanban-app/src/ai/mod.rs`, `apps/kanban-app/src/main.rs`, `apps/kanban-app/Cargo.toml`, `apps/kanban-app/tests/agent_ws.rs`. Verified: `cargo build -p kanban-app` clean with and without `--features ai-local-models`; `cargo test -p kanban-app` green (16 tests, the `agent_ws` integration test included); `cargo clippy -p kanban-app --tests` clean.

### Warnings
- [x] `apps/kanban-app/Cargo.toml:17-21` — The new `ai-local-models` Cargo feature contradicts an explicit documented constraint. `ARCHITECTURE.md` §1 Practices #1 states: "No feature flags. The Cargo workspace says explicitly: 'NEVER add features or feature flags.' The only exception is `test-support` for test utilities." `ai-local-models` is neither `test-support` nor exempt. Worse, the feature is inert: `swissarmyhammer-agent` depends on `llama-agent` unconditionally, so `llama-agent` compiles into `kanban-app` regardless of the flag (confirmed — a plain `cargo build -p kanban-app` compiles `llama-agent`). The feature gates no code, adds no dependency, and does not make the standard build lighter. The stated rationale ("so the standard build stays light") is therefore unmet. The "builds with and without the feature" acceptance criterion passes only because the feature is a no-op. Recommended fix: delete the `ai-local-models` feature entirely — `create_agent` already dispatches Claude vs llama at runtime via `ModelConfig::executor_type()`, so no compile-time gate is needed and the architecture rule is honored. If a genuine opt-out of the local-llama backend is wanted, that is a `swissarmyhammer-agent` restructuring task and should be tracked/justified against the no-feature-flags rule explicitly — not bolted on as an inert kanban-app-only flag.
  - RESOLVED: deleted the `[features]` block (the `ai-local-models = []` feature) from `apps/kanban-app/Cargo.toml`. Removed all stale doc-comment references to the feature in `agent_ws.rs` (the topology diagram now notes runtime dispatch via `ModelConfig::executor_type`). Acceptance criteria updated to drop the now-moot with/without-feature build matrix.

### Nits
- [x] `apps/kanban-app/src/ai/agent_ws.rs:84` — `bind_with` is `pub` and takes an arbitrary `ModelConfig`, but nothing in the crate or tests calls it (only `bind()` is used). It is kept alive solely by the module-wide `#![allow(dead_code)]` in `ai/mod.rs`. If it is genuinely needed for the follow-up wiring task (`01KRRN3SP5D1H63TQ8HM7SQZ1F`), fine; otherwise consider folding it into `bind()` until a caller exists, so the public surface reflects real use.
  - RESOLVED: removed the unused `bind_with` and inlined its body into `bind()`. `bind()` now constructs the listener and `ModelConfig::default()` directly; the public surface reflects real use. Its doc comment notes that a future caller needing a different backend supplies a different `ModelConfig` (runtime dispatch, no compile-time gate).
- [x] `apps/kanban-app/src/ai/agent_ws.rs:105` `run()` — the accept loop spawns one unbounded task per inbound connection with no cap. For a loopback-only dev/IPC server this is acceptable, but since any local process can open the ephemeral port, a connection cap (or at least a comment acknowledging the unbounded fan-out is intentional) would be worth adding when the follow-up task wires this into app startup.
  - RESOLVED: added a "Concurrency and security posture" section to the `run()` doc comment explaining that unbounded per-connection task spawning is intentional for a loopback-only server (the webview opens a single ACP connection; realistic fan-out is one).
- [x] `apps/kanban-app/src/ai/agent_ws.rs` — the loopback WebSocket has no origin/auth check, so any local process can connect and drive an in-process agent. This matches a desktop-app threat model and loopback binding limits exposure, but the follow-up wiring task should consider a per-launch token in the `ws://` URL handed to the webview, so only the app's own webview can connect.
  - RESOLVED: the same `run()` doc-comment "Concurrency and security posture" section notes the absence of an origin/auth check, why loopback binding bounds the exposure, and that the follow-up wiring task (`01KRRN3SP5D1H63TQ8HM7SQZ1F`) should mint a per-launch token, embed it in the `ws://` URL handed to the webview, and reject connections that do not present it.