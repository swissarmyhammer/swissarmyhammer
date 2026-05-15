---
assignees:
- claude-code
depends_on:
- 01KMNKX8H3HMV3SXSV5E0FBBVP
position_column: done
position_ordinal: ffffffffffffffa380
title: End-to-end drag reorder tests — vitest + Rust integration
---
## What

Write comprehensive automated tests that verify the full drag-and-drop pipeline: zone rendering → drop event → backend move → re-render. Tests cover same-column reorder, cross-column move, and cross-board move, all without manual testing.

### Frontend tests (vitest)

Create `kanban-app/ui/src/components/board-drag-drop.test.tsx` with integration-style tests that:
1. Render a board with multiple columns and cards
2. Assert correct drop zones exist with correct descriptors
3. Simulate drops on specific zones
4. Verify `dispatch_command` is called with correct `task.move` args

Specific scenarios to cover:
- **Move 3rd card to 2nd position**: 3 cards [A,B,C] → drop C on zone `before-B` → backend called with `{ before_id: B }`
- **Move 1st card to last**: drop A on zone `after-C` → `{ after_id: C }`
- **Move card to different column**: drop A on doing column's zone → `{ column: 'doing', before_id: X }`
- **Move card to empty column**: drop A on doing's empty zone → `{ column: 'doing' }` (no before/after)

### Backend tests (Rust)

Already well-covered in `tests/dispatch_move_placement.rs`. Add one more test:

In `swissarmyhammer-kanban/tests/dispatch_move_placement.rs`:
- **Move 3rd card to 2nd position via dispatch**: [A,B,C] → move C with `before_id: B` → verify ordinals: A < C < B

### Notification flow test

Verify that after a move, the backend emits `entity-field-changed` with updated `position_ordinal`. This is already tested in `kanban-app/src/watcher.rs` (`test_flush_and_emit_detects_task_position_ordinal_change`), but add a comment cross-referencing it.

### Files
- **Create**: `kanban-app/ui/src/components/board-drag-drop.test.tsx`
- **Modify**: `swissarmyhammer-kanban/tests/dispatch_move_placement.rs` (add 1 test)

## Acceptance Criteria
- [ ] Frontend test verifies: 3 cards produce 4 zones per column with correct descriptors
- [ ] Frontend test verifies: dropping on zone calls backend with zone's before/after
- [ ] Frontend test verifies: cross-column drop includes target column ID
- [ ] Rust test verifies: move 3rd to 2nd produces correct ordinal ordering
- [ ] Zero manual testing required — all automated

## Tests
- [ ] `pnpm vitest run src/components/board-drag-drop.test.tsx` passes
- [ ] `cargo nextest run -p swissarmyhammer-kanban dispatch_move` passes
- [ ] `pnpm vitest run` full suite passes
- [ ] `cargo nextest run -p swissarmyhammer-kanban` full suite passes