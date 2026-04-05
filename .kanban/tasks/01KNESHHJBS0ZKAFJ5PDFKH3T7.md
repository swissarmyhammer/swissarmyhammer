---
assignees:
- claude-code
depends_on:
- 01KNESD0630BPJG3BK84KEJMJP
position_column: todo
position_ordinal: '8580'
position_swimlane: lsp-live
title: 'LSP-T2B: get_inbound_calls op'
---
## What

Implement `get_inbound_calls` — "who calls this function?" Uses layered resolution via `LayeredContext`.

### `get_inbound_calls` — layered
- New file: `swissarmyhammer-code-context/src/ops/get_inbound_calls.rs`
- Takes `&LayeredContext` + `GetInboundCallsOptions { file_path, line, character, depth }`
- **Layer 1 (live LSP)**: `ctx.lsp_request("textDocument/prepareCallHierarchy", ...)` → `ctx.lsp_request("callHierarchy/incomingCalls", ...)`. Recursive up to `depth` (max 5). Cross-reference with indexed edges — `ctx.lsp_callers_of()` may have edges in files the live LSP missed.
- **Layer 2 (LSP index)**: `ctx.lsp_callers_of(symbol_id)` — reverse-traverse indexed outbound call edges. Recursive for `depth > 1` by following each caller's own callers.
- **Layer 3 (tree-sitter)**: `ctx.ts_callers_of(file_path, symbol_name)` — tree-sitter-derived call edges (`source = 'treesitter'`). Same reverse traversal.

### Result types
```rust
pub struct InboundCallsResult {
    pub target: String,
    pub callers: Vec<InboundCallEntry>,
    pub source_layer: SourceLayer,
}
pub struct InboundCallEntry {
    pub symbol_name: String,
    pub file_path: String,
    pub range: LspRange,
    pub call_sites: Vec<LspRange>,
    pub depth: u32,
    pub callers: Vec<InboundCallEntry>,
}
```

### Files to create/modify
- `swissarmyhammer-code-context/src/ops/get_inbound_calls.rs` — new
- `swissarmyhammer-code-context/src/ops/mod.rs` — re-export
- `swissarmyhammer-code-context/src/lib.rs` — re-export
- `swissarmyhammer-tools/src/mcp/tools/code_context/mod.rs` — registration + handler

## Acceptance Criteria
- [ ] Returns callers via `ctx.lsp_request()` when live LSP available
- [ ] Falls back to `ctx.lsp_callers_of()` from LSP index
- [ ] Falls back to `ctx.ts_callers_of()` from tree-sitter
- [ ] Recursive depth traversal (1-5, clamped)
- [ ] Cross-references live results with `ctx.lsp_callers_of()` for completeness

## Tests
- [ ] Unit test: `prepareCallHierarchy` + `incomingCalls` response parsing
- [ ] Unit test: reverse traversal via `lsp_callers_of()`
- [ ] Unit test: depth clamping to max 5
- [ ] Unit test: recursive result tree construction
- [ ] `cargo nextest run -p swissarmyhammer-code-context` passes

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.

#lsp-live