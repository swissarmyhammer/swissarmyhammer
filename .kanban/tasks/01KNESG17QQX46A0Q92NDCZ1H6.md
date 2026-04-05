---
assignees:
- claude-code
depends_on:
- 01KNESD0630BPJG3BK84KEJMJP
position_column: todo
position_ordinal: '8380'
position_swimlane: lsp-live
title: 'LSP-T1C: get_references op'
---
## What

Implement `get_references` — find all references to a symbol. Uses layered resolution via `LayeredContext` with grouping and enclosing-symbol enrichment.

### `get_references` — layered
- New file: `swissarmyhammer-code-context/src/ops/get_references.rs`
- Takes `&LayeredContext` + `GetReferencesOptions { file_path, line, character, include_declaration, max_results }`
- **Layer 1 (live LSP)**: `ctx.lsp_notify()` didOpen → `ctx.lsp_request("textDocument/references", ...)` → `ctx.lsp_notify()` didClose. Full cross-file reference finding.
- **Layer 2 (LSP index)**: `ctx.lsp_callers_of(symbol_id)` — reverse call edges as a proxy for references. Covers call sites but misses type references, field accesses, etc.
- **Layer 3 (tree-sitter)**: `ctx.ts_chunks_matching(symbol_name, max)` — text search for the symbol name across chunks. Noisy (comments, strings) but provides something.
- **Enrichment**: For each reference location, `ctx.enrich_location()` to find the enclosing function — answers "what function references this?" not just "where?"
- Groups results by file into `FileReferenceGroup`

### Result types
```rust
pub struct ReferencesResult {
    pub references: Vec<ReferenceLocation>,
    pub total_count: usize,
    pub by_file: Vec<FileReferenceGroup>,
    pub source_layer: SourceLayer,
}
```

### Files to create/modify
- `swissarmyhammer-code-context/src/ops/get_references.rs` — new
- `swissarmyhammer-code-context/src/ops/mod.rs` — re-export
- `swissarmyhammer-code-context/src/lib.rs` — re-export
- `swissarmyhammer-tools/src/mcp/tools/code_context/mod.rs` — registration + handler

## Acceptance Criteria
- [ ] Returns live references via `ctx.lsp_request()` when available
- [ ] Falls back to `ctx.lsp_callers_of()` for call-edge based references
- [ ] Falls back to `ctx.ts_chunks_matching()` for text search
- [ ] Each reference enriched with enclosing symbol via `ctx.enrich_location()`
- [ ] Grouped by file; respects `max_results`; reports `total_count`

## Tests
- [ ] Unit test: grouping logic buckets correctly by file
- [ ] Unit test: `max_results` truncation preserves `total_count`
- [ ] Unit test: enclosing symbol enrichment via `enrich_location()`
- [ ] Unit test: fallback to `lsp_callers_of()` returns call-site references
- [ ] `cargo nextest run -p swissarmyhammer-code-context` passes

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.

#lsp-live