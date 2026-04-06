---
assignees:
- claude-code
depends_on:
- 01KNESD0630BPJG3BK84KEJMJP
position_column: done
position_ordinal: ffffffffffffffffffdb80
title: 'LSP-T2A: get_implementations op'
---
## What

Implement `get_implementations` — find implementations of a trait/interface. Uses layered resolution via `LayeredContext`.

### `get_implementations` — layered
- New file: `swissarmyhammer-code-context/src/ops/get_implementations.rs`
- Takes `&LayeredContext` + `GetImplementationsOptions { file_path, line, character, max_results }`
- **Layer 1 (live LSP)**: `ctx.lsp_notify()` didOpen → `ctx.lsp_request("textDocument/implementation", ...)` → `ctx.lsp_notify()` didClose. Full cross-file implementation finding.
- **Layer 2 (LSP index)**: No direct equivalent — implementation relationships aren't stored in `lsp_symbols`/`lsp_call_edges`. Skip.
- **Layer 3 (tree-sitter)**: `ctx.ts_chunks_matching("impl TraitName", max)` — heuristic pattern search. Low quality but better than nothing for Rust.
- Enriches results with `ctx.enrich_location()`; reuses `DefinitionLocation` type.

### Files to create/modify
- `swissarmyhammer-code-context/src/ops/get_implementations.rs` — new
- `swissarmyhammer-code-context/src/ops/mod.rs` — re-export
- `swissarmyhammer-code-context/src/lib.rs` — re-export
- `swissarmyhammer-tools/src/mcp/tools/code_context/mod.rs` — registration + handler

## Acceptance Criteria
- [ ] Returns implementation locations via `ctx.lsp_request()` when live LSP available
- [ ] Falls back to `ctx.ts_chunks_matching()` heuristic when no live LSP
- [ ] Returns empty (not error) when `!ctx.has_live_lsp()` and no heuristic matches
- [ ] Enriches results via `ctx.enrich_location()`
- [ ] Respects `max_results`

## Tests
- [ ] Unit test: response parsing handles Location and LocationLink formats
- [ ] Unit test: empty result when no layers available (not error)
- [ ] Unit test: `max_results` truncation
- [ ] `cargo nextest run -p swissarmyhammer-code-context` passes

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.

#lsp-live