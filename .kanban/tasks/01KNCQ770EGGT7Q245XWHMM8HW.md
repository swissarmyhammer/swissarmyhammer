---
assignees:
- claude-code
depends_on:
- 01KNC7R0H7DXWBDFZ7CZDVDN3E
position_column: todo
position_ordinal: b580
title: 'Drag-drop: resolve source and target boards from scope chain'
---
## What

After the container refactor lands `StoreContainer` (path moniker) and `BoardContainer` (board entity moniker), the drag system should use scope chains for source and target board resolution instead of explicit `boardPath` strings.

A drag is \"source scope chain → target scope chain\":\n- **Source**: `[window:main, store:/path/a, board:01ABC, column:backlog, task:task-1]`\n- **Target**: `[window:secondary, store:/path/b, board:02DEF, column:done]`

Same-board vs cross-board is determined by whether the store/board monikers match.

**Frontend:**\n- `DropZoneDescriptor` drops `boardPath`, uses scope chain from its position in the FocusScope tree\n- `DragStartCmd`/`DragCompleteCmd` args carry scope chains, not explicit paths\n- `persistMove` dispatches via `useDispatchCommand`, scope chain provides board context

**Backend:**\n- `DragStartCmd` derives source board from scope chain store moniker\n- `DragCompleteCmd` derives target board from scope chain\n- Cross-board handler resolves both handles from scope chain store monikers

**TDD:** Integration test with two temp directories, each initialized as a kanban board. Dispatch drag commands with different scope chains. Verify same-board move and cross-board transfer.

#drag-refactor

## Acceptance Criteria
- [ ] `DropZoneDescriptor` no longer carries `boardPath`
- [ ] Drag commands derive boards from scope chain, not explicit args
- [ ] Same-board and cross-board drag both work
- [ ] No explicit board path strings in drag frontend or backend

## Tests
- [ ] New Rust integration test: two temp board dirs, same-board move via scope chains
- [ ] New Rust integration test: two temp board dirs, cross-board transfer via scope chains
- [ ] `cargo test -p swissarmyhammer-kanban` — all pass
- [ ] `cd kanban-app/ui && pnpm vitest run` — all pass