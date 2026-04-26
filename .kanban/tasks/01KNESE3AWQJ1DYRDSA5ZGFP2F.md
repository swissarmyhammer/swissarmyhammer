---
assignees:
- claude-code
depends_on:
- 01KNESD0630BPJG3BK84KEJMJP
position_column: done
position_ordinal: ffffffffffffffffffffffffa680
title: 'LSP-T1A: get_definition + get_type_definition ops'
---
## What

Implement two location-returning ops that share the `DefinitionLocation` result type. Both use **layered resolution** via `LayeredContext`.

### `get_definition` — layered
- New file: `swissarmyhammer-code-context/src/ops/get_definition.rs`
- Takes `&LayeredContext` + `GetDefinitionOptions { file_path, line, character, include_source }`
- **Layer 1 (live LSP)**: `ctx.lsp_notify()` didOpen → `ctx.lsp_request("textDocument/definition", ...)` → `ctx.lsp_notify()` didClose. Parse location(s), read source from disk if `include_source`, enrich each via `ctx.enrich_location()`.
- **Layer 2 (LSP index)**: `ctx.lsp_symbol_at(file_path, range)` — look up symbol at the cursor position. Returns the definition location from indexed data. Less precise (no cross-file jump) but works offline.
- **Layer 3 (tree-sitter)**: `ctx.ts_chunk_at(file_path, line)` — return the chunk containing the cursor. Basic — just shows what's at that position.
- Returns `Vec<DefinitionLocation>` + `SourceLayer` indicating which layer provided results.

### `get_type_definition` — live LSP only
- New file: `swissarmyhammer-code-context/src/ops/get_type_definition.rs`  
- Takes `&LayeredContext` + same options
- **Layer 1 only**: `ctx.lsp_request("textDocument/typeDefinition", ...)`. Type definition is inherently a live LSP feature — no index equivalent.
- Returns empty + `SourceLayer::None` when `ctx.has_live_lsp()` is false.

### MCP registration
- Add both to dispatcher in `swissarmyhammer-tools/src/mcp/tools/code_context/mod.rs`
- Handlers pass `&LayeredContext` constructed by the dispatcher

### Files to create/modify
- `swissarmyhammer-code-context/src/ops/get_definition.rs` — new
- `swissarmyhammer-code-context/src/ops/get_type_definition.rs` — new
- `swissarmyhammer-code-context/src/ops/mod.rs` — re-export
- `swissarmyhammer-code-context/src/lib.rs` — re-export
- `swissarmyhammer-tools/src/mcp/tools/code_context/mod.rs` — registration + handlers

## Acceptance Criteria
- [ ] `get_definition` returns results via `ctx.lsp_request()` when live LSP available
- [ ] `get_definition` falls back to `ctx.lsp_symbol_at()`, then `ctx.ts_chunk_at()`
- [ ] `get_type_definition` uses `ctx.lsp_request()` only; returns empty when `!ctx.has_live_lsp()`
- [ ] Both include source text when `include_source` is true
- [ ] Both enrich results with `ctx.enrich_location()`
- [ ] `SourceLayer` correctly reported in results

## Tests
- [ ] Unit test: response parsing handles single Location, Location array, and LocationLink
- [ ] Unit test: fallback to `lsp_symbol_at()` when no live LSP
- [ ] Unit test: fallback to `ts_chunk_at()` when LSP index empty
- [ ] Unit test: `get_type_definition` returns empty (not error) when `has_live_lsp() == false`
- [ ] `cargo nextest run -p swissarmyhammer-code-context` passes

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.

#lsp-live