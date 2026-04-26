---
position_column: done
position_ordinal: af80
title: Implement LSP document symbols collection
---
## What

`collect_file_symbols` in `lsp_communication.rs` is a stub — returns 0 symbols without parsing the LSP response. `collect_and_persist_symbols` is never called from production code. Need to implement real `textDocument/documentSymbol` requests and response parsing.

**Key files:**
- `swissarmyhammer-code-context/src/lsp_communication.rs` — `collect_file_symbols` (stub), `collect_and_persist_symbols` (never called)
- `swissarmyhammer-code-context/src/lsp_indexer.rs` — `write_symbols`, `flatten_symbols`, `mark_lsp_indexed` (all work, never used in production)

**Approach:**
1. Fix `send_request` to actually read JSON-RPC responses (parse Content-Length header, read body)
2. Implement `collect_file_symbols` to parse DocumentSymbol response
3. Call `collect_and_persist_symbols` from the indexing pipeline after LSP server is ready
4. Mark `lsp_indexed=1` after successful symbol collection

## Acceptance Criteria
- [ ] `textDocument/documentSymbol` requests sent and responses parsed
- [ ] Symbols written to `lsp_symbols` table with real LSP data
- [ ] `lsp_indexed=1` set for files with successful symbol collection
- [ ] Hierarchical symbols flattened correctly (nested structs/methods)

## Tests
- [ ] Unit test: parse a real DocumentSymbol JSON response
- [ ] Unit test: flatten_symbols produces correct hierarchy
- [ ] `cargo test -p swissarmyhammer-code-context` passes