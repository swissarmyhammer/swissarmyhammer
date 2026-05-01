---
assignees:
- claude-code
depends_on:
- 01KNESD0630BPJG3BK84KEJMJP
position_column: done
position_ordinal: ffffffffffffffffffffffffae80
title: 'LSP-T2C: get_diagnostics op'
---
## What

Implement `get_diagnostics` — errors and warnings for a file. **Live LSP only** — no meaningful index fallback. All access through `LayeredContext`.

### `get_diagnostics` — live LSP only
- New file: `swissarmyhammer-code-context/src/ops/get_diagnostics.rs`
- Takes `&LayeredContext` + `GetDiagnosticsOptions { file_path, severity_filter }`
- **Layer 1 (live LSP)**: `ctx.lsp_notify()` didOpen → wait for `publishDiagnostics` notification → collect → `ctx.lsp_notify()` didClose. Enrich each diagnostic with enclosing symbol via `ctx.enrich_location()`.
- **No index fallback**: Diagnostics require live analysis. Returns empty + `SourceLayer::None` when `!ctx.has_live_lsp()`.
- **Requires notification handling**: `LayeredContext` needs a method to collect notifications during a time window. Add `ctx.lsp_collect_notifications(duration) -> Vec<Value>` or similar to INFRA (or handle within this card if scoped tightly).

### Result types
```rust
pub enum DiagnosticSeverity { Error, Warning, Info, Hint }
pub struct DiagnosticsResult {
    pub diagnostics: Vec<Diagnostic>,
    pub error_count: usize,
    pub warning_count: usize,
}
pub struct Diagnostic {
    pub range: LspRange,
    pub severity: DiagnosticSeverity,
    pub message: String,
    pub code: Option<String>,
    pub source: Option<String>,
    pub containing_symbol: Option<String>,  // via ctx.enrich_location()
}
```

### Files to create/modify
- `swissarmyhammer-code-context/src/layered_context.rs` — add `lsp_collect_notifications()` if not already in INFRA
- `swissarmyhammer-code-context/src/ops/get_diagnostics.rs` — new
- `swissarmyhammer-code-context/src/ops/mod.rs` — re-export
- `swissarmyhammer-code-context/src/lib.rs` — re-export
- `swissarmyhammer-tools/src/mcp/tools/code_context/mod.rs` — registration + handler

## Acceptance Criteria
- [ ] Returns diagnostics via `ctx.lsp_notify()` + notification collection when live LSP available
- [ ] Severity filter works
- [ ] Each diagnostic enriched with enclosing symbol via `ctx.enrich_location()`
- [ ] Returns empty (not error) when `!ctx.has_live_lsp()`
- [ ] Reports `error_count` and `warning_count` summary

## Tests
- [ ] Unit test: `publishDiagnostics` notification parsing
- [ ] Unit test: severity filter logic
- [ ] Unit test: enrichment maps diagnostic range to enclosing symbol
- [ ] Unit test: empty result when no live LSP
- [ ] `cargo nextest run -p swissarmyhammer-code-context` passes

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.

#lsp-live