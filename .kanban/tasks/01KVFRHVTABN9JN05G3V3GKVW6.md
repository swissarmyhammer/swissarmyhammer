---
assignees:
- claude-code
position_column: todo
position_ordinal: a280
project: diagnostics
title: Route ALL follower LSP code-context ops through the leader (not just diagnostics)
---
## Why (evidence, 2026-06-19 investigation)
Follower processes spawn NO LSP supervisor (correct, per ^7a5h2bj). The follower→leader multiplexer (^ref6nj4) + its live wiring (^4rjtgsj) route ONLY the diagnostics tool to the leader. Every OTHER LSP-backed code-context op silently degrades on a follower to tree-sitter / the persisted index, never crossing the socket to the leader's live rust-analyzer.

Evidence:
- `crates/swissarmyhammer-tools/src/mcp/tools/diagnostics/mod.rs::diagnose_via_leader` (`:426`) is the ONLY production caller of `SessionRequestClient`.
- All other ops (`get_definition`, `get_type_definition`, `get_hover`, `get_references`, `get_implementations`, `workspace_symbol_live`, `get_inbound_calls`, `get_code_actions`, `get_rename_edits`, …) build a `LayeredContext` from the LOCAL `LSP_SUPERVISOR` (`lsp_session_for_file`/`any_lsp_session`, `code_context/mod.rs:57/83`), which a follower never initializes → `session = None`.
- `LayeredContext::lsp_request` (`layered_context.rs:234`) short-circuits `None → Ok(None)`, so live ops fall back to tree-sitter/persisted layers with no indication they were degraded by the leader/follower split.
- The multiplexer's generic `lsp_request` method (`request_api.rs::dispatch`) is wired on the LEADER side but has NO production follower-side caller (only tests, `leader_follower_request_ipc.rs:141`).

Net: on a follower (e.g. a stdio-MCP subagent), go-to-definition / hover / references / symbols / implementations / inbound-calls / code-actions / rename return tree-sitter-only or empty results instead of the leader's real rust-analyzer answers.

## What
Make `LayeredContext` (or the op layer) route its live-LSP requests to the leader over the EXISTING `SessionRequestClient`/`lsp_request` multiplexer when there is no local session, instead of returning `Ok(None)`. Reuse the existing transport — do NOT add a second client/socket/envelope. A follower op should: try local session → if none, route `lsp_request(method, params)` to the leader via `SessionRequestClient` (built from `workspace.socket_path()`/`lock_path()`); on connect failure surface the typed not-leader/leader-pid error (or fall back to the index layer where that is the documented best-effort behavior — decide per op and be explicit).

## Depends on
- ^ref6nj4 (multiplexer mechanism), ^4rjtgsj (live wiring + SessionRequestClient construction pattern to mirror) — both done.

## Acceptance Criteria
- [ ] On a follower process, the live code-context LSP ops (definition/type-definition/hover/references/implementations/workspace+document symbols/inbound-calls/code-actions/rename) route to the leader's session over the socket and return the leader's real results — not silent tree-sitter/empty degradation.
- [ ] Reuses the existing `SessionRequestClient`/`lsp_request` path; no second transport/client/envelope.
- [ ] Connect/serve failure surfaces the typed not-leader/leader-pid error (or documented index fallback), never a silent wrong-empty.
- [ ] Integration (gated on rust-analyzer): a follower `sah` process running e.g. get_references / get_hover gets the leader's real answer with only the leader's single rust-analyzer running.

## Workflow
- Use `/tdd`. The `LayeredContext::lsp_request None→Ok(None)` seam is the natural injection point. #diagnostics