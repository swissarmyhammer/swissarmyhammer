---
position_column: done
position_ordinal: ad80
title: Implement LSP call hierarchy for real call edges
---
## What

`write_edges` in `lsp_indexer.rs` (source='lsp') is never called in production. Only tree-sitter heuristic edges exist. Real LSP provides `callHierarchy/incomingCalls` and `callHierarchy/outgoingCalls` which give accurate call graphs.

**Key files:**
- `swissarmyhammer-code-context/src/lsp_indexer.rs` — `write_edges` (works, never called from LSP)
- `swissarmyhammer-code-context/src/lsp_communication.rs` — needs call hierarchy methods
- `swissarmyhammer-code-context/src/ts_callgraph.rs` — tree-sitter heuristic (fallback)

**Approach:**
1. After document symbols are collected, use `textDocument/prepareCallHierarchy` on each function/method
2. Then `callHierarchy/incomingCalls` and `callHierarchy/outgoingCalls` for edges
3. Write edges with `source='lsp'` via existing `write_edges`
4. LSP edges take priority over tree-sitter heuristic edges

## Acceptance Criteria
- [ ] Call edges with `source='lsp'` appear in `lsp_call_edges` table
- [ ] `get callgraph` returns LSP-sourced edges when available
- [ ] Tree-sitter heuristic edges still work as fallback for languages without LSP

## Tests
- [ ] Unit test: parse call hierarchy response into edges
- [ ] `cargo test -p swissarmyhammer-code-context` passes