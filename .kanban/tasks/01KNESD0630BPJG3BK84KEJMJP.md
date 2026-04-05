---
assignees:
- claude-code
position_column: todo
position_ordinal: '80'
position_swimlane: lsp-live
title: 'LSP-INFRA: Layered resolution, LspContext, shared types'
---
## What

Foundation for all code-context operations. Establishes a **layered resolution** model where ops use the best available data source:

```
Live LSP  >  LSP index (lsp_symbols/lsp_call_edges)  >  Tree-sitter index (ts_chunks)
```

No op should ever block waiting for the index. Ops always proceed with whatever layers are available.

### 1. `LayeredContext` — the single access point for all three layers

In `swissarmyhammer-code-context/src/layered_context.rs`:

```rust
pub struct LayeredContext<'a> {
    conn: &'a Connection,
    lsp_client: Option<&'a SharedLspClient>,
}

impl LayeredContext<'_> {
    // === Layer availability ===
    pub fn has_live_lsp(&self) -> bool;
    pub fn has_lsp_index(&self, file_path: &str) -> bool;  // checks lsp_indexed flag
    pub fn has_ts_index(&self, file_path: &str) -> bool;   // checks ts_indexed flag

    // === Layer 1: Live LSP ===
    /// Send an arbitrary LSP request. Returns None if no live client.
    pub fn lsp_request(&self, method: &str, params: Value) -> Result<Option<Value>>;
    /// Send an LSP notification (didOpen, didClose). No-op if no live client.
    pub fn lsp_notify(&self, method: &str, params: Value) -> Result<()>;

    // === Layer 2: LSP index (lsp_symbols, lsp_call_edges tables) ===
    pub fn lsp_symbol_at(&self, file_path: &str, range: &LspRange) -> Option<SymbolInfo>;
    pub fn lsp_symbols_in_file(&self, file_path: &str) -> Vec<SymbolInfo>;
    pub fn lsp_symbols_by_name(&self, query: &str, max: usize) -> Vec<SymbolInfo>;
    pub fn lsp_callers_of(&self, symbol_id: &str) -> Vec<CallEdgeInfo>;
    pub fn lsp_callees_of(&self, symbol_id: &str) -> Vec<CallEdgeInfo>;

    // === Layer 3: Tree-sitter index (ts_chunks table) ===
    pub fn ts_chunk_at(&self, file_path: &str, line: u32) -> Option<ChunkInfo>;
    pub fn ts_symbols_in_file(&self, file_path: &str) -> Vec<SymbolInfo>;
    pub fn ts_chunks_matching(&self, query: &str, max: usize) -> Vec<ChunkInfo>;
    pub fn ts_callers_of(&self, file_path: &str, symbol: &str) -> Vec<CallEdgeInfo>;

    // === Layered convenience (tries live > lsp_index > ts in order) ===
    pub fn enrich_location(&self, file_path: &str, range: &LspRange) -> EnrichmentResult;
    pub fn find_symbol(&self, file_path: &str, line: u32, char: u32) -> Option<SymbolInfo>;
}

pub struct EnrichmentResult {
    pub symbol: Option<SymbolInfo>,
    pub source_layer: SourceLayer,
}

pub enum SourceLayer {
    LiveLsp,
    LspIndex,
    TreeSitter,
    None,
}
```

Key design points:
- **`conn` and `lsp_client` are private** — ops don't reach into raw DB or raw LSP client. They use typed methods.
- **Each layer has its own method family** — ops choose their own fallback strategy (some try all three, some are live-only).
- **Convenience methods** like `enrich_location()` do the full fallback chain automatically.
- **`SourceLayer` enum** lets results report which layer provided the data.

### 2. Remove `check_ts_readiness()` blocking

- Delete `check_ts_readiness()` calls from all handlers in `swissarmyhammer-tools/src/mcp/tools/code_context/mod.rs`
- Delete or deprecate `check_blocking_status()` in `blocking.rs`
- Ops never block — they degrade gracefully via `LayeredContext`

### 3. Make `send_request` public

On `LspJsonRpcClient` in `swissarmyhammer-code-context/src/lsp_communication.rs`. The `LayeredContext::lsp_request()` method wraps this.

### 4. Shared types

```rust
pub struct LspRange { pub start_line: u32, pub start_character: u32, pub end_line: u32, pub end_character: u32 }
pub struct SymbolInfo { pub name: String, pub qualified_path: Option<String>, pub kind: String, pub detail: Option<String>, pub file_path: String, pub range: LspRange }
pub struct CallEdgeInfo { pub symbol: SymbolInfo, pub call_sites: Vec<LspRange> }
pub struct ChunkInfo { pub text: String, pub file_path: String, pub start_line: u32, pub end_line: u32 }
pub struct FileEdit { pub file_path: String, pub text_edits: Vec<TextEdit> }
pub struct TextEdit { pub range: LspRange, pub new_text: String }
pub struct DefinitionLocation { pub file_path: String, pub range: LspRange, pub source_text: Option<String>, pub symbol: Option<SymbolInfo> }
```

### 5. Availability notice

Extend `maybe_append_lsp_notice` to append layer info when results come from a lower layer:
- No notice for live LSP (best case)
- "Results from LSP index (live LSP not available)"
- "Results from tree-sitter index only (LSP not available)"

### Files to modify
- `swissarmyhammer-code-context/src/lsp_communication.rs` — make `send_request` pub
- `swissarmyhammer-code-context/src/layered_context.rs` — new file: LayeredContext, all layer methods, shared types
- `swissarmyhammer-code-context/src/lib.rs` — re-export
- `swissarmyhammer-tools/src/mcp/tools/code_context/mod.rs` — remove `check_ts_readiness()`, construct `LayeredContext` in dispatcher, pass to handlers
- `swissarmyhammer-tools/src/mcp/tools/code_context/blocking.rs` — deprecate/remove

### Migration note
Existing ops currently take `&Connection` and do their own SQL. This card does NOT rewrite them — it introduces `LayeredContext` for new ops and removes the blocking gate. Migrating existing ops to use `LayeredContext` is a separate follow-up.

## Acceptance Criteria
- [ ] `LayeredContext` struct exists with private `conn`/`lsp_client` fields
- [ ] Layer 1 methods: `lsp_request()`, `lsp_notify()` — wrap `send_request`/notifications, return `None`/no-op when no client
- [ ] Layer 2 methods: `lsp_symbol_at()`, `lsp_symbols_in_file()`, `lsp_symbols_by_name()`, `lsp_callers_of()`, `lsp_callees_of()`
- [ ] Layer 3 methods: `ts_chunk_at()`, `ts_symbols_in_file()`, `ts_chunks_matching()`, `ts_callers_of()`
- [ ] Convenience: `enrich_location()` tries live > LSP index > tree-sitter, returns `EnrichmentResult` with `source_layer`
- [ ] `SourceLayer` enum: `LiveLsp`, `LspIndex`, `TreeSitter`, `None`
- [ ] All shared types defined and serializable (serde)
- [ ] `check_ts_readiness()` removed from all handlers — ops never block
- [ ] All existing tests still pass

## Tests
- [ ] Unit test: `has_live_lsp()` returns false when no client
- [ ] Unit test: `lsp_request()` returns `Ok(None)` when no client (not error)
- [ ] Unit test: `lsp_symbol_at()` returns data from `lsp_symbols` table
- [ ] Unit test: `ts_chunk_at()` returns data from `ts_chunks` table
- [ ] Unit test: `enrich_location()` prefers LSP index over tree-sitter when both available
- [ ] Unit test: `enrich_location()` falls back to tree-sitter when LSP index empty
- [ ] Unit test: `enrich_location()` returns `SourceLayer::None` when all empty
- [ ] `cargo nextest run -p swissarmyhammer-code-context` passes
- [ ] `cargo nextest run -p swissarmyhammer-tools` passes

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.

#lsp-live