---
assignees:
- claude-code
depends_on:
- 01KNESD0630BPJG3BK84KEJMJP
position_column: todo
position_ordinal: '8280'
position_swimlane: lsp-live
title: 'LSP-T1B: get_hover op'
---
## What

Implement `get_hover` — type info and documentation. Uses layered resolution via `LayeredContext`.

### `get_hover` — layered
- New file: `swissarmyhammer-code-context/src/ops/get_hover.rs`
- Takes `&LayeredContext` + `GetHoverOptions { file_path, line, character }`
- **Layer 1 (live LSP)**: `ctx.lsp_notify()` didOpen → `ctx.lsp_request("textDocument/hover", ...)` → `ctx.lsp_notify()` didClose. Parse markdown contents + range. Enrich with `ctx.enrich_location()`.
- **Layer 2 (LSP index)**: `ctx.lsp_symbol_at(file_path, range)` — return `detail` field (e.g. "fn() -> MyStruct") as hover content. Type signature without full documentation.
- **Layer 3 (tree-sitter)**: `ctx.ts_chunk_at(file_path, line)` — return chunk text. Shows the code itself as a last resort.

### Result type
```rust
pub struct HoverResult {
    pub contents: String,
    pub range: Option<LspRange>,
    pub symbol: Option<SymbolInfo>,
    pub source_layer: SourceLayer,
}
```

### Files to create/modify
- `swissarmyhammer-code-context/src/ops/get_hover.rs` — new
- `swissarmyhammer-code-context/src/ops/mod.rs` — re-export
- `swissarmyhammer-code-context/src/lib.rs` — re-export
- `swissarmyhammer-tools/src/mcp/tools/code_context/mod.rs` — registration + handler

## Acceptance Criteria
- [ ] Returns live hover markdown via `ctx.lsp_request()` when available
- [ ] Falls back to `ctx.lsp_symbol_at().detail` when no live LSP
- [ ] Falls back to `ctx.ts_chunk_at().text` as last resort
- [ ] Handles `MarkupContent`, `MarkedString`, and array response formats
- [ ] `source_layer` correctly indicates which layer provided the result

## Tests
- [ ] Unit test: hover response parsing for all LSP format variants
- [ ] Unit test: fallback to LSP index detail field
- [ ] Unit test: fallback to tree-sitter chunk text
- [ ] Unit test: `source_layer` correctly set for each layer
- [ ] `cargo nextest run -p swissarmyhammer-code-context` passes

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.

#lsp-live