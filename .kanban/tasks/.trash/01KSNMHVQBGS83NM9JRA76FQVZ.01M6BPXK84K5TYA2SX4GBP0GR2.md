---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffc480
project: ai-panel
title: 'E2E test: prove llama-agent works through the full kanban-app stack (qwen-0.6b-test)'
---
## DONE (2026-05-28)

New integration test `apps/kanban-app/tests/ai_panel_e2e.rs` drives the real production path — in-process ACP agent over a loopback WebSocket, the exact transport the webview uses — against the small `qwen-0.6b-test` model.

### What landed
- `test_ai_panel_e2e_qwen_generates_tokens_and_second_prompt_succeeds` (~3-7s): connects an ACP ws client, `initialize` → `session/new` → `session/prompt`. Asserts `tokens_generated > 0` AND the reply text is non-empty (catches the 0-token bug, which was a *successful* call with empty output). Then a **second prompt on the same session** also generates tokens — proving the single worker is released (the "Queue is full" / queue-lifecycle half of the bug).
- `test_ai_panel_e2e_mcp_tool_reachable_in_session` (~39-50s): starts the board's real MCP server via the same `start_mcp_server_with_options` call `start_board_mcp_server` uses, attaches it through the ACP `mcpServers` list (the `ai_start_agent` `mcpUrl` path), and asserts the model actually invokes a board kanban tool (`tool_calls_executed >= 1`). Logs confirm "Discovered 10 tools from MCP client / Adding 10 MCP tools to session". Reliable: 3/3 runs called the tool on attempt 1; a bounded 4-attempt retry absorbs sampling variance (same proven shape as llama-agent's `acp_multi_turn_dispatches_tool_and_threads_result`).
- Added `swissarmyhammer-operations` to kanban-app dev-deps (for `Execute` to init the test board).
- Added a dedicated `kanban-ai-panel-e2e` nextest test-group (max-threads=1) so the two real-model tests don't cold-load qwen concurrently — consistent with the embedding-serial groups.

### Honest deviations from the card's literal wording (all verified, not shortcuts)
- **"streams non-empty agent text"**: this in-process ACP agent does NOT emit `session/update` chunks over the ws (verified: zero notification frames arrive); the full reply + token count come in the response `_meta` (`llama_response` / `tokens_generated`). The content assertion is on `_meta.llama_response` — same guarantee (non-empty output), just not via a streaming channel this path doesn't use.
- **MCP "advertised tools" assertion**: the agent never sends its discovered tool list to the client (no `AvailableCommandsUpdate` is emitted — confirmed in `acp/server.rs`), so "tools advertised" is not observable over the wire. The only ws-observable MCP signal is `tool_calls_executed` (the model actually calling a tool), so the test uses the "stretch" form. The in-process "advertised" leg is already covered by llama-agent's `acp_new_session_attaches_mcp_servers_and_advertises_tools`.
- **30s budget**: test 1 is well under (~3-7s). Test 3 is ~39-50s — investigated per the card: it is NOT cold-start, it's the genuine multi-round agentic tool-loop (the 0.6B calls the tool, gets the result, generates again — several real ~100-token rounds). The 90s per-op hang-guard and the nextest group's 360s slow-timeout accommodate it.
- **`#[path]` include**: kanban-app is a bin crate with no lib target, so the test pulls `ai/agent_ws.rs` in via `#[path]` (the established `tests/agent_ws.rs` pattern) and reaches `ai/models.rs` / `state.rs` logic through the same public APIs they call (`ModelManager` + `unified_server`), since those modules reference `crate::` and can't be included standalone.

### Acceptance criteria
- [x] New integration test in `apps/kanban-app/tests/` driving the real `qwen-0.6b-test` path to a non-empty response.
- [x] Asserts `tokens_generated > 0` (not just "no error").
- [x] Asserts a second prompt queues after the first completes.
- [x] Asserts the kanban MCP tools are reachable — via the model actually invoking one (`tool_calls_executed >= 1`); MCP-discovery confirmed in logs (10 tools).
- [x] Would have failed before bug `01KSNJ7CBK9333J0T9G4TCA7DH` (0 tokens) — the token + content assertions catch it. (The `update.board` bug `01KSNJ6AE18EQYDC2WSYFSSAY1` is a frontend-dispatch bug not on this Rust ACP path; that registration gap is guarded by the `update.board` command-dispatch test added with its fix.)
- [x] No env-var gate, no `#[ignore]`. Narrow runtime skip only on model-unavailable (HF rate-limit/offline first-download), matching the repo's `try_init_agent` idiom; the model is cached on the self-hosted CI runner so assertions always run there.
- [x] clippy clean; passes under nextest (default profile).