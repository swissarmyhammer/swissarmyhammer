---
assignees:
- claude-code
depends_on:
- 01KNESD0630BPJG3BK84KEJMJP
position_column: done
position_ordinal: ffffffffffffffffffde80
title: 'LSP-T3A: get_rename_edits op'
---
## What

Implement `get_rename_edits` — preview a rename without applying. **Live LSP only**. All access through `LayeredContext`. Goes beyond Claude Code's LSP tool.

### `get_rename_edits` — live LSP only
- New file: `swissarmyhammer-code-context/src/ops/get_rename_edits.rs`
- Takes `&LayeredContext` + `GetRenameEditsOptions { file_path, line, character, new_name }`
- **Layer 1 only**: `ctx.lsp_request("textDocument/prepareRename", ...)` → `ctx.lsp_request("textDocument/rename", ...)`. Returns structured edits, does NOT apply.
- **No index fallback**: Rename requires live semantic analysis. Returns `can_rename: false` when `!ctx.has_live_lsp()`.

### Result types (shared `FileEdit`/`TextEdit` from INFRA)
```rust
pub struct RenameEditsResult {
    pub can_rename: bool,
    pub edits: Vec<FileEdit>,
    pub files_affected: usize,
}
```

### Files to create/modify
- `swissarmyhammer-code-context/src/ops/get_rename_edits.rs` — new
- `swissarmyhammer-code-context/src/ops/mod.rs` — re-export
- `swissarmyhammer-code-context/src/lib.rs` — re-export
- `swissarmyhammer-tools/src/mcp/tools/code_context/mod.rs` — registration + handler

## Acceptance Criteria
- [ ] `ctx.lsp_request("textDocument/prepareRename", ...)` validates position first
- [ ] `ctx.lsp_request("textDocument/rename", ...)` computes edits
- [ ] Returns `can_rename: false` when no live LSP or rename not possible
- [ ] Edits grouped by file; `files_affected` accurate
- [ ] Preview only — does NOT apply edits

## Tests
- [ ] Unit test: `prepareRename` response parsing (range, placeholder variants)
- [ ] Unit test: `rename` WorkspaceEdit parsing (documentChanges and changes formats)
- [ ] Unit test: `can_rename: false` when prepareRename returns null/error
- [ ] Unit test: `can_rename: false` when `!ctx.has_live_lsp()`
- [ ] `cargo nextest run -p swissarmyhammer-code-context` passes

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.

#lsp-live