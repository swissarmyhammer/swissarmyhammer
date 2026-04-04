---
assignees:
- claude-code
position_column: todo
position_ordinal: b580
title: 'Backend: resolve board from scope chain window moniker'
---
## What

`dispatch_command_internal` in `kanban-app/src/commands.rs` resolves the board handle from an explicit `board_path` parameter (line 1015). The scope chain already carries `window:label`, and `UIState::window_board(label)` maps labels to board paths. Replace the explicit parameter with scope-chain-only resolution.

**Change `dispatch_command_internal` (line 1015):**
```rust
// Current:
let active_handle = resolve_handle(state, effective_board_path).await.ok();

// Target: resolve from scope chain window moniker — no boardPath param
let board_path = ctx.window_label_from_scope()
    .and_then(|label| state.ui_state.window_board(label));
let active_handle = resolve_handle(state, board_path).await.ok();
```

When no `window:label` is in the scope chain, `resolve_handle(state, None)` falls back to the active board (single-window/CLI case). The explicit `board_path` parameter is removed from `dispatch_command` and `dispatch_command_internal` signatures.

**Also update the Tauri command signature:**
- `kanban-app/src/commands.rs` — `dispatch_command` #[tauri::command]: remove `board_path` parameter
- Frontend callers already send scope chain with `window:label`; they stop sending `boardPath`

**Files to modify:**
- `kanban-app/src/commands.rs` — remove `board_path` from `dispatch_command` and `dispatch_command_internal`, resolve board from scope chain

**TDD approach:** Write Rust integration tests that set up AppState with UIState window→board mapping and open board handles. Dispatch commands with scope chains containing `window:test` and verify the correct board is used. Test the no-window fallback (CLI/single-window uses active board).

## Acceptance Criteria
- [ ] `dispatch_command` no longer accepts `board_path` parameter
- [ ] Board resolved from `window:label` in scope chain via `UIState::window_board`
- [ ] No-window fallback: when scope chain has no window moniker, uses active board
- [ ] All existing commands still work

## Tests
- [ ] New test: scope chain `[\"window:main\"]` + UIState mapping `main→/path/to/board` → resolves correct handle
- [ ] New test: scope chain `[]` → falls back to active board (CLI/single-window)
- [ ] `cargo test -p swissarmyhammer-kanban` — all pass
- [ ] `cd kanban-app && cargo test` — all pass