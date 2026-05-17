---
assignees:
- claude-code
depends_on:
- 01KRRN2M0E5YXA9E24N1BM2BVP
- 01KRRR9R082SKN6HY5DA2EEGBX
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffffff380
project: ai-panel
title: Model selection and the AI agent endpoint command surface
---
## What
Wire model choice to the in-process ACP agent (the "WebSocket ACP agent" task) and give the webview the two endpoints it needs.

- New `apps/kanban-app/src/ai/models.rs`.
- Detect Claude Code: `claude` resolvable on `PATH` (honor a `CLAUDE_CLI` override).
- Enumerate selectable models: a Claude Code entry (gated on detection — present-but-disabled with a hint when absent) plus configured local llama models from `swissarmyhammer-config`.
- Tie selection to the in-process agent: when the webview picks a model, the backend prepares the in-process agent endpoint for that `ModelConfig` and yields the `ws://127.0.0.1:<port>` URL.
- Tauri commands (register in `main.rs`): `ai_list_models() -> Vec<Model>` (`{ id, label, kind, available, hint }`); `ai_start_agent(model_id, board_path) -> { wsUrl, mcpUrl }` — a one-time config handoff: the loopback `ws://` agent URL plus the board's full-SAH-toolset MCP URL (`http://127.0.0.1:<port>/mcp` from the per-board toolset server). The TypeScript client puts `mcpUrl` in `newSession.mcpServers`. This is config discovery, NOT a data channel.
- Lifecycle: stop the in-process agent and its WebSocket server on window/board/app teardown; track in `AppState`.

## Acceptance Criteria
- [x] `claude` detection returns `Some` when on `PATH`/`CLAUDE_CLI`, else `None`.
- [x] `ai_list_models` returns a Claude Code entry with `available` reflecting detection, plus local llama models (enumerated unconditionally from `swissarmyhammer-config` — see Implementation Notes re: the non-existent `ai-local-models` feature).
- [x] `ai_start_agent` returns a `ws://127.0.0.1:<port>` URL a WebSocket can `initialize` over for the chosen model, plus the board's full-SAH-toolset `mcpUrl`.
- [x] Teardown stops the in-process agent and WS server — no leaks.
- [x] `cargo build -p kanban-app` is clean.

## Tests
- [x] Unit tests: `claude` detection against a fake binary on `PATH`; model enumeration shape (Claude entry available/unavailable, local llama models enumerated, embedding models excluded).
- [x] Integration test: `ai_start_agent`'s round trip — `RunningAgents::start` returns a `wsUrl` a WebSocket client `initialize`s over for the selected model; `stop`/`stop_all` teardown frees the port; re-selecting a model replaces the endpoint. The board's `mcpUrl` is covered by `state.rs`'s `test_open_board_serves_full_sah_mcp_toolset`.
- [x] `cargo test -p kanban-app` is green.

## Workflow
- Use `/tdd` — write the detection and `ai_start_agent` round-trip tests first.

## Implementation Notes

### The `ai-local-models` Cargo feature does NOT exist
The original description referred to a `ai-local-models` Cargo feature ("plus configured local llama models … only when the `ai-local-models` Cargo feature is built"). **That feature does not exist and was never added.** It had been deliberately removed during the WebSocket ACP agent task (`01KRRN2M0E5YXA9E24N1BM2BVP`)'s review because it violated `ARCHITECTURE.md`'s no-feature-flags rule and was inert. Claude-vs-local-llama dispatch is decided at RUNTIME by `create_agent` / `ModelConfig::executor_type()`, so a compile-time gate would both break the rule and be meaningless.

**How local-model enumeration was handled instead:** `ai_list_models` calls `ModelManager::list_agents()` unconditionally. That returns every configured agent (built-in + project + user, with precedence) from `swissarmyhammer-config`. Each agent's YAML is parsed via `parse_model_config` and its `executor_type()` inspected: only `ModelExecutorType::LlamaAgent` entries become `LocalLlama` models. The built-in `claude-code` agent file is skipped (the Claude Code entry is synthesized separately with live CLI detection), and embedding executors (`llama-embedding`, `ane-embedding`) are excluded because `create_agent` rejects them as chat agents. Enumeration is therefore driven entirely by what configuration defines on the machine — no Cargo feature, consistent with the architecture.

### Files changed
- New `apps/kanban-app/src/ai/models.rs` — `detect_claude_cli`, `ModelKind`, `Model`, `ai_list_models`, `resolve_model_config`, `AgentEndpoint`, `RunningAgent`, `RunningAgents`, `ai_start_agent`, plus inline unit + integration tests.
- `apps/kanban-app/src/ai/agent_ws.rs` — added `AgentWebSocketServer::bind_with(ModelConfig)` so the server can be bound for a specific selected model; `bind()` now delegates to it.
- `apps/kanban-app/src/ai/mod.rs` — declares `pub mod models`.
- `apps/kanban-app/src/state.rs` — `AppState` gained a `running_agents: RunningAgents` registry; `close_board` stops the board's agent.
- `apps/kanban-app/src/main.rs` — registered `ai_list_models` and `ai_start_agent` Tauri commands; `handle_run_event` (ExitRequested) calls `running_agents.stop_all()`.
- `apps/kanban-app/Cargo.toml` — added the workspace `which` dependency for PATH-based `claude` detection.
- `apps/kanban-app/tests/agent_ws.rs` — now `#[path]`-includes `agent_ws.rs` directly instead of `ai/mod.rs`, because `ai/models.rs` references `crate::state::AppState` (only resolvable in the full binary).

### Lifecycle
`RunningAgents` keys one `RunningAgent` per board path. `RunningAgent::Drop` aborts the accept-loop task (releasing the loopback port). Teardown is driven three ways: `close_board` → `stop(board)`; app exit → `stop_all()`; and re-selecting a model for a board replaces (and drops) the prior endpoint.

### Test isolation
Tests that mutate `PATH`/`CLAUDE_CLI` use a static `Mutex`-backed `EnvGuard` (the same RAII-guard pattern as `swissarmyhammer-common`'s `HOME_ENV_LOCK`/`CURRENT_DIR_LOCK`) that saves and restores the env on drop. The crate had no prior `serial_test` usage; this self-contained guard matches the established workspace convention for env isolation and keeps the real environment and source tree untouched. `git status` stays clean after the suite runs.

### Review-finding follow-up (2026-05-17)
The `run()` doc comment's stale security note (which pointed at this task as future work for a per-launch auth token) was rewritten to state the accepted risk plainly: the loopback `ws://` agent has no per-connection auth and is mitigated only by loopback-only binding. The per-launch token hardening is genuine deferred work and is now tracked as a separate kanban task — **`01KRV7GFHKD1FFGNY8C6X8BZZ4`** ("Add a per-launch auth token to the in-process WebSocket ACP agent", `ai-panel` project, `security` tag) — which the `agent_ws.rs` comment now references.

## Review Findings (2026-05-17 14:52)

### Warnings
- [x] `apps/kanban-app/src/ai/agent_ws.rs:125-129` — The `run()` doc comment's security note says "The follow-up task that wires this server into Tauri startup (`01KRRN3SP5D1H63TQ8HM7SQZ1F`) should mint a per-launch token, embed it in the `ws://` URL handed to the webview, and reject connections that do not present it." But `01KRRN3SP5D1H63TQ8HM7SQZ1F` *is this task* — it is the one that just wired the server into Tauri startup (`ai_start_agent`, `RunningAgents`, command registration). The comment now points a completed task at itself as if it were pending, and the per-launch auth token it prescribes was not implemented here. The comment's own threat statement ("any local process that discovers the ephemeral port could connect and drive an in-process agent") therefore stands unmitigated, and no downstream task (`01KRRN463S53X13YE1PQ1H8P53`, `01KRRN5HWYA0Z6P7H2BNS1E33B`) tracks the token work. Suggestion: either implement the per-launch token in `bind_with`/`ai_start_agent` (mint a secret, append it as a `ws://…?token=` query or subprotocol, reject connections lacking it), or — if accepting the loopback-only posture as sufficient for the single-user desktop threat model — update the comment to drop the stale task reference and state the accepted risk plainly, and file a separate tracked task for the token if it is still wanted. RESOLVED: comment rewritten to state the accepted loopback-only posture plainly and drop the self-referential task id; per-launch token work filed as separate task `01KRV7GFHKD1FFGNY8C6X8BZZ4`.