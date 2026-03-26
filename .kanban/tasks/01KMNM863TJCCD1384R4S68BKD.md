---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffc980
title: 'Backend: add before_id/after_id to cross_board::transfer_task'
---
## What

`cross_board::transfer_task()` currently only accepts `drop_index: Option<u64>` for positioning. The new drop zone architecture provides `before_id`/`after_id` instead. Add these parameters so cross-board drops use the same ID-based placement as same-board drops.

### Current signature
```rust
pub async fn transfer_task(
    source_ctx: &KanbanContext,
    target_ctx: &KanbanContext,
    task_id: &str,
    target_column: &str,
    drop_index: Option<u64>,       // ← index-based, being eliminated
    copy_mode: bool,
) -> Result<Value, String>
```

### New signature
```rust
pub async fn transfer_task(
    source_ctx: &KanbanContext,
    target_ctx: &KanbanContext,
    task_id: &str,
    target_column: &str,
    drop_index: Option<u64>,       // keep for backward compat, lowest priority
    before_id: Option<&str>,       // ← NEW: place before this task ID
    after_id: Option<&str>,        // ← NEW: place after this task ID
    copy_mode: bool,
) -> Result<Value, String>
```

### Ordinal resolution priority (same as MoveTask)
1. `before_id`/`after_id` — compute ordinal from neighbors
2. `drop_index` — compute from position (legacy fallback)
3. Neither — append at end

### Changes needed in 3 files

**`swissarmyhammer-kanban/src/cross_board.rs`** (lines 50-80):
- Add `before_id` and `after_id` parameters
- Add ordinal computation from `before_id`/`after_id` (reuse `compute_ordinal_for_neighbors` pattern from `MoveTask`)
- Keep `drop_index` as fallback

**`swissarmyhammer-kanban/src/commands/drag_commands.rs`** (line 356):
- Pass `before_id` and `after_id` through in the cross-board `DragComplete` JSON response (currently only passes `drop_index`)

**`kanban-app/src/commands.rs`** (lines 1039-1046):
- Extract `before_id`/`after_id` from `drag_complete` payload
- Pass them to `transfer_task()`

### Files
- **Modify**: `swissarmyhammer-kanban/src/cross_board.rs`
- **Modify**: `swissarmyhammer-kanban/src/commands/drag_commands.rs`
- **Modify**: `kanban-app/src/commands.rs`

## Acceptance Criteria
- [ ] `transfer_task()` accepts `before_id` and `after_id` parameters
- [ ] When `before_id` is provided, task lands before that ID on the target board
- [ ] When `after_id` is provided, task lands after that ID on the target board
- [ ] When neither is provided, falls back to `drop_index` then append
- [ ] `DragCompleteCmd` passes `before_id`/`after_id` in cross-board response
- [ ] Tauri dispatch extracts and forwards `before_id`/`after_id`

## Tests
- [ ] `cross_board.rs` — new test: transfer with `before_id` places task before existing card
- [ ] `cross_board.rs` — new test: transfer with `after_id` places task after existing card
- [ ] `cross_board.rs` — existing tests still pass (drop_index path unchanged)
- [ ] `cargo nextest run -p swissarmyhammer-kanban cross_board` passes
- [ ] `cargo nextest run -p swissarmyhammer-kanban` full suite passes