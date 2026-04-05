---
assignees:
- claude-code
depends_on:
- 01KNESD0630BPJG3BK84KEJMJP
position_column: todo
position_ordinal: '8880'
position_swimlane: lsp-live
title: 'LSP-T3B: get_code_actions op'
---
## What

Implement `get_code_actions` — available fixes and refactors. **Live LSP only**. All access through `LayeredContext`. Goes beyond Claude Code's LSP tool.

### `get_code_actions` — live LSP only
- New file: `swissarmyhammer-code-context/src/ops/get_code_actions.rs`
- Takes `&LayeredContext` + `GetCodeActionsOptions { file_path, start_line, start_character, end_line, end_character, filter_kind }`
- **Layer 1 only**: `ctx.lsp_request("textDocument/codeAction", ...)` with range and optional `only` filter. Optionally `ctx.lsp_request("codeAction/resolve", ...)` for actions without inline edits.
- **No index fallback**: Code actions require live analysis. Returns empty when `!ctx.has_live_lsp()`.

### Result types (reuses shared `FileEdit` from INFRA)
```rust
pub struct CodeActionsResult {
    pub actions: Vec<CodeAction>,
}
pub struct CodeAction {
    pub title: String,
    pub kind: Option<String>,
    pub edits: Option<Vec<FileEdit>>,
    pub is_preferred: bool,
}
```

### Files to create/modify
- `swissarmyhammer-code-context/src/ops/get_code_actions.rs` — new
- `swissarmyhammer-code-context/src/ops/mod.rs` — re-export
- `swissarmyhammer-code-context/src/lib.rs` — re-export
- `swissarmyhammer-tools/src/mcp/tools/code_context/mod.rs` — registration + handler

## Acceptance Criteria
- [ ] Returns code actions via `ctx.lsp_request()` when live LSP available
- [ ] `filter_kind` limits to specific kinds (quickfix, refactor, source)
- [ ] Resolved edits via `ctx.lsp_request("codeAction/resolve", ...)` when available
- [ ] Returns empty (not error) when `!ctx.has_live_lsp()`

## Tests
- [ ] Unit test: codeAction response parsing (Command vs CodeAction variants)
- [ ] Unit test: filter_kind filtering logic
- [ ] Unit test: WorkspaceEdit extraction from resolved code actions
- [ ] Unit test: empty result when no live LSP
- [ ] `cargo nextest run -p swissarmyhammer-code-context` passes

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.

#lsp-live