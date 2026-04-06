---
assignees:
- claude-code
depends_on: []
position_column: todo
position_ordinal: b780
title: 'Drag system: use scope chains for source and target board resolution'
---
## What

The drag system currently passes explicit `boardPath` strings for source and target boards. Instead, source and target should be identified by their scope chains — the same way every other command resolves its board.

A drag is \"this scope chain to that scope chain\":\n- **Source**: `[window:main, board:board, column:backlog, task:task-1]`\n- **Target**: `[window:secondary, board:board, column:done]` (or same window, different column)

Same-board vs cross-board is determined by whether the window monikers resolve to the same board path.

**Frontend changes:**

1. `kanban-app/ui/src/lib/drop-zones.ts` — `DropZoneDescriptor`: replace `boardPath: string` with scope chain information. The drop zone sits inside a column FocusScope which is inside a board FocusScope which is inside a window scope. The descriptor should carry a `scopeChain` or derive it from context at drop time.

2. `kanban-app/ui/src/components/board-view.tsx` — `persistMove` and `handleZoneDrop`: instead of passing `descriptor.boardPath`, dispatch through useDispatchCommand with the drop zone's scope chain as the target context.

3. `kanban-app/ui/src/components/board-view.tsx` — `DragStartCmd` args: stop passing explicit `boardPath`. The source board is the current scope chain's window.

**Backend changes:**

4. `swissarmyhammer-kanban/src/commands/drag_commands.rs` — `DragStartCmd`: derive source board from scope chain window moniker instead of `args.boardPath`.

5. `swissarmyhammer-kanban/src/commands/drag_commands.rs` — `DragCompleteCmd`: derive target board from a target scope chain (new arg, or from the dispatch scope chain). Compare source and target window labels to determine same-board vs cross-board.

6. `kanban-app/src/commands.rs` — cross-board handler (line ~1139): resolve both board handles from window labels instead of explicit path strings.

**Files to modify:**
- `kanban-app/ui/src/lib/drop-zones.ts` — DropZoneDescriptor type
- `kanban-app/ui/src/components/board-view.tsx` — drag start/complete dispatch
- `swissarmyhammer-kanban/src/commands/drag_commands.rs` — DragStartCmd, DragCompleteCmd
- `kanban-app/src/commands.rs` — cross-board handler

**TDD tests:**

Integration test with two temp directories, each initialized as a kanban board. Set up UIState with two window→board mappings. Dispatch `drag.start` with source scope chain, then `drag.complete` with target scope chain. Verify:\n- Same window: task moves between columns\n- Different windows (same board): task moves between columns\n- Different windows (different boards): task transfers to target board with new ID

## Acceptance Criteria
- [ ] `DropZoneDescriptor` no longer carries `boardPath`
- [ ] `DragStartCmd` derives source board from scope chain, not args
- [ ] `DragCompleteCmd` derives target board from scope chain
- [ ] Same-board drag works (same window, different columns)
- [ ] Cross-board drag works (different windows, different boards)
- [ ] No explicit board path strings in drag frontend or backend

## Tests
- [ ] New Rust integration test: two temp board dirs, same-board move via scope chains
- [ ] New Rust integration test: two temp board dirs, cross-board transfer via scope chains
- [ ] `cargo test -p swissarmyhammer-kanban` — all pass
- [ ] `cd kanban-app/ui && pnpm vitest run` — all pass
- [ ] `drop-zones.test.ts` updated for new DropZoneDescriptor shape