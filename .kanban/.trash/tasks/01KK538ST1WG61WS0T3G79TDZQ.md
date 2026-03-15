---
position_column: done
position_ordinal: r5
title: LSP symbol extraction + call edge extraction
---
## What
Connect the running LSP server to the unified DB. Extract symbols via `textDocument/documentSymbol`, build call edges via `callHierarchy/incomingCalls` and `callHierarchy/outgoingCalls`. Feed `didOpen` for indexed files on startup.

Files: `swissarmyhammer-code-context/src/lsp_indexer.rs`

Spec: `ideas/code-context-architecture.md` — "LSP layer: incremental invalidation" section.

## Acceptance Criteria
- [ ] `LspIndexer` queries LSP for document symbols, writes to `lsp_symbols` with qualified path IDs
- [ ] For each function/method symbol, queries outgoing call hierarchy, writes edges to `lsp_call_edges` with `source: 'lsp'`
- [ ] `textDocument/didOpen` sent for each file on initial indexing
- [ ] `lsp_indexed` flag set to 1 on `indexed_files` when a file completes
- [ ] Handles LSP server being unavailable (logs warning, skips gracefully)

## Tests
- [ ] Integration test with mock LSP server: send documentSymbol response, verify lsp_symbols rows
- [ ] Integration test: send callHierarchy response, verify lsp_call_edges rows with correct caller/callee IDs
- [ ] Unit test: qualified path construction from nested LSP symbols
- [ ] `cargo test -p swissarmyhammer-code-context`