---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffff9180
title: 'Cleanup: remove dead BoardHandle::open wrapper (dead_code warning)'
---
## What
Compiler warning (working tree, 2026-06-09): `associated function 'open' is never used` — `apps/kanban-app/src/state.rs:353` `BoardHandle::open`.

It is genuinely dead, NOT a wiring gap. `BoardHandle::open` is a thin convenience wrapper that delegates to `BoardHandle::open_with(.. BoardOpenOptions::default())` (state.rs:358). Every production caller bypasses it:
- `AppState::open_board` → `open_board_with` → `BoardHandle::open_with(...)` directly (state.rs:1085).
- Menu handlers call `AppState::open_board`, not `BoardHandle::open`.

All remaining references to `BoardHandle::open` are doc/test COMMENTS (state.rs:117, 336, 369, 375, 1553, 1888, 2315, 2329, 2346), not code. So nothing calls it — a leftover from the board-open refactor that routed callers straight to `open_with`.

## Fix
- Delete `pub async fn open(...)` (state.rs:353–365).
- Repoint the doc/test comments that say "BoardHandle::open" to `BoardHandle::open_with` (esp. the test comments at 2315–2346 describing `ensure_workspace_tools` — that call lives in `open_with`).
- (Alternative if it must stay as public API: `#[allow(dead_code)]` with a note — but deletion is preferred; `open_with` is the canonical entry.)

## Acceptance Criteria
- [ ] No `dead_code` warning for `BoardHandle::open`.
- [ ] Boards still open via `open_board` → `open_board_with` → `open_with` (unchanged behavior).
- [ ] No dangling doc reference to a removed `BoardHandle::open`.

## Tests
- [ ] `cargo build -p kanban-app` is warning-clean for this item.
- [ ] Existing board-open tests (`open_board_for_test` / the `.skills` store test at state.rs:2346) stay green.

## Note
Low priority / trivial. `state.rs` is in the other session's active 60-file working tree — coordinate so this cleanup doesn't collide with the board-management/command-cutover work (cards `01KT7QA0YH…`, `01KT7QA2RY…`). #tech-debt