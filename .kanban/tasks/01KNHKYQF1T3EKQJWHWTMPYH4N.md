---
assignees:
- claude-code
position_column: todo
position_ordinal: 8b80
title: Profile and fix slow task.move roundtrip on large boards
---
## What

Profile why `task.move` operations (drag-and-drop, "Do This Next") take several seconds on larger boards and identify the bottleneck(s). The full roundtrip is: UI dispatch → Tauri IPC → Rust `dispatch_command` (scope resolution, command execution, entity flush, event emission) → UI event handling → React re-render.

**Investigation targets (ordered by likelihood):**

1. **Rust backend `dispatch_command`** (`kanban-app/src/commands.rs`): entity flush writes to disk — on large boards this may be doing excessive I/O. Profile with `std::time::Instant` instrumentation or `tracing` spans.

2. **React re-render cascade**: `BoardView` (`kanban-app/ui/src/components/board-view.tsx`) recomputes `baseLayout` on every event, causing all `ColumnView` instances to re-render. The `cardClaimPredicates` in `ColumnView` (`kanban-app/ui/src/components/column-view.tsx`) builds O(n×m) predicates which is expensive for large boards.

3. **Drag session overhead**: `DragSessionProvider` (`kanban-app/ui/src/lib/drag-session-context.tsx`) may dispatch 2–3 IPC calls per drag (`drag.start`, `task.move`, `drag.complete`). Each is a full roundtrip.

4. **Virtualization threshold**: `VirtualizedCardList` in `column-view.tsx` only kicks in at 25+ cards per column. Boards with many columns of 10–24 cards render everything directly.

**Approach:**
1. Add timing instrumentation to `dispatch_command` in Rust (log elapsed per phase: scope resolution, execution, flush, event emit)
2. Add `performance.mark`/`performance.measure` around the UI dispatch→re-render cycle in `command-scope.tsx`
3. Use React DevTools Profiler or `<Profiler>` component to identify expensive re-renders in `BoardView`/`ColumnView`
4. Based on findings, apply targeted fixes (memoization, reduced flush scope, batched events, etc.)

**Files to investigate:**
- `kanban-app/src/commands.rs` — Rust command dispatch and entity flush
- `kanban-app/ui/src/lib/command-scope.tsx` — IPC dispatch timing
- `kanban-app/ui/src/components/board-view.tsx` — layout recomputation
- `kanban-app/ui/src/components/column-view.tsx` — cardClaimPredicates O(n×m)
- `kanban-app/ui/src/lib/drag-session-context.tsx` — multi-call drag flow

## Acceptance Criteria
- [ ] Timing instrumentation added to Rust `dispatch_command` (logged per phase)
- [ ] UI-side performance marks added around command dispatch cycle
- [ ] Root cause(s) of multi-second latency identified and documented
- [ ] At least one fix applied that measurably reduces `task.move` latency on a board with 50+ tasks
- [ ] Drag-and-drop and "Do This Next" complete in under 500ms on a 50-task board

## Tests
- [ ] Existing test suite passes: `cargo nextest run` and `pnpm test`
- [ ] Add a benchmark or timing assertion in `kanban-app/src/commands.rs` tests that `task.move` on a 50-task board completes in < 200ms (Rust side)
- [ ] No regressions in drag-and-drop behavior (manual verification)

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.