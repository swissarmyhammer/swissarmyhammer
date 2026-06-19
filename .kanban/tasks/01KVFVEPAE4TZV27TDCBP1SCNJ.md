---
assignees:
- claude-code
depends_on:
- 01KVFRHVTABN9JN05G3V3GKVW6
position_column: todo
position_ordinal: a180
project: diagnostics
title: Route follower MULTI-STEP LSP ops (inbound_calls / code_actions / rename_edits) to the leader
---
## Why
^v3gkvw6 routed the follower SINGLE-request live-LSP ops (definition/type_definition/hover/references/implementations/workspace_symbol) to the leader via the existing SessionRequestClient/lsp_request multiplexer (with a leader-side document sync). It deliberately did NOT route the MULTI-STEP ops:
- `get inbound_calls` — `textDocument/prepareCallHierarchy` then `callHierarchy/incomingCalls`, recursive
- `get code_actions` — range query that may chain resolve
- `get rename_edits` — `textDocument/prepareRename` then `textDocument/rename`

These hold the leader session's client lock ACROSS several requests via `LayeredContext::lsp_multi_request_with_document` (so no other consumer interleaves and steals a response off the shared stdio pipe). The current `METHOD_LSP_REQUEST` is a SINGLE `session.request` round-trip and cannot reproduce a locked multi-step exchange. On a follower these ops currently fall back to their documented index/tree-sitter best-effort (rename returns can_rename:false) — correct, no wrong-empty, but not the leader's live answer.

## What
Add a leader-side multi-request capability over the EXISTING request multiplexer (do NOT add a second transport/client): e.g. a new `METHOD_LSP_MULTI_REQUEST` (or a verb on the existing one) whose dispatch runs the whole multi-step exchange under one `session.with_client` lock on the leader (sync_open the doc first, then prepareCallHierarchy+incomingCalls / prepareRename+rename), returning the final parsed payload (or the raw step responses) in ONE IPC round-trip. Then wire the follower path: extend the `LiveLspRouter` seam (or add a sibling) so `LayeredContext::lsp_multi_request_with_document` routes to the leader when session is None, mirroring how the single-request seam already routes. Reuse the per-op method/params construction; keep it DRY.

## Constraints (same as ^v3gkvw6)
- No crate cycle: routing impl lives in the tools layer (depends on diagnostics); code-context owns only the closure/seam TYPE.
- block_in_place needs the multi-thread runtime; guard/degrade as build_follower_router does.
- !Send DbRef must not cross an .await.

## Acceptance
- [ ] On a follower, get inbound_calls / get code_actions / get rename_edits route to the leader's session and return the leader's real multi-step results (not index/tree-sitter degradation), over the EXISTING multiplexer (no second transport).
- [ ] The multi-step exchange runs atomically under one client lock on the leader (no interleave).
- [ ] Unit (model-free) + gated integration (rust-analyzer) proving a follower gets the leader's real inbound-calls/rename via the op, with one leader rust-analyzer and no PPID=1 orphan.

## Refs
- ^v3gkvw6 (single-request routing + LiveLspRouter seam + leader-side doc sync — the pattern to extend)
- crates/swissarmyhammer-code-context/src/layered_context.rs (lsp_multi_request_with_document, LiveLspRouter)
- crates/swissarmyhammer-diagnostics/src/request_api.rs (dispatch, METHOD_LSP_REQUEST)
- crates/swissarmyhammer-tools/src/mcp/tools/code_context/leader_route.rs (build_follower_router pattern)
- crates/swissarmyhammer-code-context/src/ops/{get_inbound_calls,get_code_actions,get_rename_edits}.rs
#diagnostics