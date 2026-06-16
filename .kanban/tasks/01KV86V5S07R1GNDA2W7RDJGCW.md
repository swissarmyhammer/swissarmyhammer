---
assignees:
- claude-code
position_column: todo
position_ordinal: a380
project: diagnostics
title: Rewire code-context onto the shared LSP session; collapse its get_diagnostics
---
## What
Make code-context a pure *consumer* of the shared session, completing the unification. Today code-context spawns/owns the client lifecycle (`lsp_server.rs`, `lsp_worker.rs`, `lsp_indexer.rs`) and `LayeredContext` opens/closes documents per request. After this task it holds a session handle and never opens documents itself.

- `LayeredContext` (`crates/swissarmyhammer-code-context/src/layered_context.rs`) takes an `LspSession` handle from `swissarmyhammer-lsp` instead of a raw `SharedLspClient`. Replace `lsp_request`/`lsp_request_with_document`/`lsp_multi_request_with_document`/`lsp_notify` internals to route through the session (documents are opened/synced by the session/watcher, not per-request).
- `lsp_worker.rs`/`lsp_indexer.rs`: the indexing worker consumes the shared session for symbol collection; remove duplicate spawn/handshake logic now living in swissarmyhammer-lsp.
- **Collapse `ops/get_diagnostics.rs`**: instead of its own pull request, it delegates to the unified diagnostics path (read latest-per-uri from the session cache / trigger a pull through the session). Keep the op's public result shape (`DiagnosticsResult`) stable for the existing `code_context` MCP tool.
- All existing code-context ops (get_definition/get_references/get_hover/get_implementations/get_inbound_calls/etc.) keep working through the session.

## Depends on
- "Add single owned LSP session with shared open-document set in swissarmyhammer-lsp"
- "Capture publishDiagnostics and add in-process fan-out in swissarmyhammer-lsp"

## Acceptance Criteria
- [ ] `LayeredContext` issues all live LSP work through the shared `LspSession`; code-context no longer opens/closes documents or spawns a client.
- [ ] `ops/get_diagnostics.rs` reads from the unified session path; duplicate diagnostics implementation removed.
- [ ] The `code_context` MCP tool's `get diagnostics`/`get definition`/etc. behave unchanged from the caller's view.

## Tests
- [ ] `cargo test -p swissarmyhammer-code-context` — all existing LSP-layer and ops tests pass against the session-backed `LayeredContext` (adapt fixtures to inject a mock session).
- [ ] Integration (gated on rust-analyzer): drive `get diagnostics` on a fixture crate with a known error via the unified path; assert the same report the old path produced.

## Workflow
- Use `/tdd`. Land after the session + fan-out exist so code-context has something to consume. #diagnostics