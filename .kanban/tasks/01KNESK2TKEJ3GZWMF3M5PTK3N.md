---
assignees:
- claude-code
depends_on:
- 01KNESD0630BPJG3BK84KEJMJP
position_column: todo
position_ordinal: '8980'
position_swimlane: lsp-live
title: 'LSP-T3C: workspace_symbol_live op'
---
## What

Implement `workspace_symbol_live` — live workspace symbol search. Uses layered resolution via `LayeredContext`.

### `workspace_symbol_live` — layered
- New file: `swissarmyhammer-code-context/src/ops/workspace_symbol_live.rs`
- Takes `&LayeredContext` + `WorkspaceSymbolLiveOptions { query, max_results }`
- **Layer 1 (live LSP)**: `ctx.lsp_request("workspace/symbol", ...)`. Most current — finds symbols the indexer hasn't reached yet.
- **Layer 2 (LSP index)**: `ctx.lsp_symbols_by_name(query, max_results)`. Same data as existing `search_symbol` op but accessed through `LayeredContext`.
- **Layer 3 (tree-sitter)**: `ctx.ts_chunks_matching(query, max_results)`. Basic text matching against chunk symbol names.
- Returns `SymbolLocation` type for consistency with indexed search.

### Design note
Consider whether this should eventually replace `search_symbol` rather than coexist. With `LayeredContext`, `search_symbol` is just "layer 2 only" — this op adds layers 1 and 3. For now, keep separate; merge later when existing ops migrate to `LayeredContext`.

### Files to create/modify
- `swissarmyhammer-code-context/src/ops/workspace_symbol_live.rs` — new
- `swissarmyhammer-code-context/src/ops/mod.rs` — re-export
- `swissarmyhammer-code-context/src/lib.rs` — re-export
- `swissarmyhammer-tools/src/mcp/tools/code_context/mod.rs` — registration + handler

## Acceptance Criteria
- [ ] Returns live results via `ctx.lsp_request()` when available
- [ ] Falls back to `ctx.lsp_symbols_by_name()` when no live LSP
- [ ] Falls back to `ctx.ts_chunks_matching()` as last resort
- [ ] Results use `SymbolLocation`/`SymbolInfo` type
- [ ] Respects `max_results`

## Tests
- [ ] Unit test: `workspace/symbol` response parsing (SymbolInformation format)
- [ ] Unit test: fallback to `lsp_symbols_by_name()`
- [ ] Unit test: fallback to `ts_chunks_matching()`
- [ ] Unit test: `max_results` truncation
- [ ] `cargo nextest run -p swissarmyhammer-code-context` passes

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.

#lsp-live