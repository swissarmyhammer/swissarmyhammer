---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffff180
title: 'Fix: 6 new MCP handlers still pass None LSP client to LayeredContext'
---
## What

The LSP client wiring fix (01KNFEW96FRWSBESQEE8HWCPQC) only updated the original 4 handlers. The 6 handlers added later (01KNFEZ48EPE6RVNGBVBBB6XYM) still use `LayeredContext::new(&db, None)`.

Affected handlers in `swissarmyhammer-tools/src/mcp/tools/code_context/mod.rs`:
- `execute_get_definition`
- `execute_get_type_definition`
- `execute_get_hover`
- `execute_get_references`
- `execute_get_implementations`
- `execute_get_code_actions`

Each needs the same `lsp_client_for_file()` / `any_lsp_client()` pattern used by `execute_get_diagnostics`, `execute_get_rename_edits`, etc.

## Acceptance Criteria
- [ ] All 6 handlers use `lsp_client_for_file()` (or `any_lsp_client()` for workspace-wide ops) instead of `None`
- [ ] Zero instances of `LayeredContext::new(&db, None)` remain in the file
- [ ] `get hover` returns `source_layer: "LiveLsp"` when rust-analyzer is running
- [ ] All tests pass

## Tests
- [ ] `cargo nextest run -p swissarmyhammer-tools` passes

#lsp-live #review-finding