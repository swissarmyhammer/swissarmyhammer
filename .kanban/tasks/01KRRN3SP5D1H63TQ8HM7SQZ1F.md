---
assignees:
- claude-code
depends_on:
- 01KRRN2M0E5YXA9E24N1BM2BVP
- 01KRRR9R082SKN6HY5DA2EEGBX
position_column: todo
position_ordinal: '8480'
project: ai-panel
title: Model selection and the AI agent endpoint command surface
---
## What
Wire model choice to the in-process ACP agent (the "WebSocket ACP agent" task) and give the webview the two endpoints it needs.

- New `apps/kanban-app/src/ai/models.rs`.
- Detect Claude Code: `claude` resolvable on `PATH` (honor a `CLAUDE_CLI` override).
- Enumerate selectable models: a Claude Code entry (gated on detection — present-but-disabled with a hint when absent) plus configured local llama models from `swissarmyhammer-config` (only when the `ai-local-models` Cargo feature is built).
- Tie selection to the in-process agent: when the webview picks a model, the backend prepares the in-process agent endpoint for that `ModelConfig` and yields the `ws://127.0.0.1:<port>` URL.
- Tauri commands (register in `main.rs`): `ai_list_models() -> Vec<Model>` (`{ id, label, kind, available, hint }`); `ai_start_agent(model_id, board_path) -> { wsUrl, mcpUrl }` — a one-time config handoff: the loopback `ws://` agent URL plus the board's full-SAH-toolset MCP URL (`http://127.0.0.1:<port>/mcp` from the per-board toolset server). The TypeScript client puts `mcpUrl` in `newSession.mcpServers`. This is config discovery, NOT a data channel.
- Lifecycle: stop the in-process agent and its WebSocket server on window/board/app teardown; track in `AppState`.

## Acceptance Criteria
- [ ] `claude` detection returns `Some` when on `PATH`/`CLAUDE_CLI`, else `None`.
- [ ] `ai_list_models` returns a Claude Code entry with `available` reflecting detection, plus local llama models when built with `ai-local-models`.
- [ ] `ai_start_agent` returns a `ws://127.0.0.1:<port>` URL a WebSocket can `initialize` over for the chosen model, plus the board's full-SAH-toolset `mcpUrl`.
- [ ] Teardown stops the in-process agent and WS server — no leaks.
- [ ] `cargo build -p kanban-app` is clean.

## Tests
- [ ] Unit tests: `claude` detection against a fake binary on `PATH`; model enumeration shape (with/without `ai-local-models`).
- [ ] Integration test: `ai_start_agent` returns a `wsUrl` a WebSocket client `initialize`s over for the selected model, and an `mcpUrl` that resolves to the board's SAH toolset; teardown stops it.
- [ ] `cargo test -p kanban-app` is green.

## Workflow
- Use `/tdd` — write the detection and `ai_start_agent` round-trip tests first.