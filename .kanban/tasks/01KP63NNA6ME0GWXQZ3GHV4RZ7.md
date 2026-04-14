---
assignees:
- claude-code
position_column: todo
position_ordinal: cb80
title: 'Fix Do This Next: delete frontend workaround, route through backend task.doThisNext, add tests'
---
## What

The "Do This Next" context menu on a task card does not reliably move the task to the top of the first column, and it's slow. The root cause is that the frontend has a workaround in `kanban-app/ui/src/components/column-view.tsx:172-199` that was supposed to have been deleted in commit `963762a7d` ("feat(kanban): implement DoThisNext backend command") but has since been re-introduced via a later merge. There is also zero frontend test coverage — no `*.test.*` file anywhere in the repo mentions `doThisNext` — which is why the regression wasn't caught.

### Why the current workaround is wrong

`column-view.tsx:172-189`:

```typescript
const buildDoThisNextCommand = useCallback(
  (taskId: string): CommandDef | null => {
    if (taskId === firstTodoTaskId) return null;
    return {
      id: "task.doThisNext",
      name: "Do This Next",
      contextMenu: true,
      execute: () => {
        const args: Record<string, unknown> = { id: taskId, column: "todo" };
        if (firstTodoTaskId) args.before_id = firstTodoTaskId;
        dispatchTaskMove({ args }).catch(console.error);
      },
    };
  },
  [firstTodoTaskId, dispatchTaskMove],
);
```

Four concrete defects:

1. **Hardcoded `column: "todo"`** — the frontend assumes the first column's id is literally `"todo"`. It happens to match the default board, but any board whose order-0 column has a different id (rename, custom template) silently moves the task to the wrong column or fails.
2. **Dispatches `task.move` instead of `task.doThisNext`** — the proper backend command `DoThisNextCmd` (`swissarmyhammer-kanban/src/commands/task_commands.rs:323-349`), which already has 5 passing backend tests including `do_this_next_moves_to_first_column`, is never called from the UI.
3. **Stale `before_id`** — `firstTodoTaskId` is derived from a React `useMemo` in `board-view.tsx:156-161`. When the user clicks "Do This Next" on multiple tasks in rapid succession, each click reads the same stale value, so all three tasks target the same `before_id`. Ordinals collide / wrong order — this is the "not reliably moving to the start" symptom.
4. **Mass re-renders (the "slow" symptom)** — `buildDoThisNextCommand` depends on `firstTodoTaskId`. Every time the first task changes (i.e., every successful Do-This-Next click), `buildDoThisNextCommand`'s identity changes, `taskExtraCommands` map is rebuilt, every card in the column gets a new `extraCommands` array reference, and `React.memo` on `DraggableTaskCard` can't skip them. The whole column re-renders on every click.

The backend command `task.doThisNext` is already correctly registered and wired:
- `swissarmyhammer-commands/builtin/commands/entity.yaml:56-64` — has `scope: "entity:task"`, `context_menu: true`, `undoable: true`
- `swissarmyhammer-kanban/src/commands/mod.rs:51-54` — registered in the command map
- `swissarmyhammer-kanban/src/commands/task_commands.rs:297-349` — implementation computes `first_column_id` correctly via numeric `order` sort with deterministic tiebreak on id, and reads the first task fresh each call (no staleness)
- `useEntityCommands("task", ...)` in `kanban-app/ui/src/lib/entity-commands.ts:110-142` already pulls this command from the schema (it has `context_menu: true`), so once the frontend override is deleted, the backend command flows through automatically.

### The fix

Delete the frontend override and let `useEntityCommands` surface `task.doThisNext` from the schema. The dispatch goes through the normal backend path using the right-clicked task's scope chain (`entity:task`), which is exactly what the backend `DoThisNextCmd::available` checks for.

Subtasks:

- [ ] Delete `buildDoThisNextCommand` (`kanban-app/ui/src/components/column-view.tsx:172-189`), the `taskExtraCommands` useMemo (`column-view.tsx:191-199`), the `firstTodoTaskId` prop (`column-view.tsx:39-40, 96`), and the `dispatchTaskMove = useDispatchCommand("task.move")` call (`column-view.tsx:109`) — all unused after this change.
- [ ] Remove `taskExtraCommands` from `VirtualizedCardListProps` and from both render paths (small-list and virtualized), along with the `extraCommands={taskExtraCommands.get(entity.id)}` prop on `DraggableTaskCard` (`column-view.tsx:487, 508, 529, 578, 709`).
- [ ] Delete the `firstTodoTaskId` plumbing in `kanban-app/ui/src/components/board-view.tsx:85, 156-161, 200, 522, 577`.
- [ ] If `extraCommands` on `DraggableTaskCard` / `EntityCard` becomes unused across the repo after this, remove it; otherwise leave the general mechanism alone (don't widen the scope).
- [ ] Add a Vitest unit test in `kanban-app/ui/src/components/column-view.test.tsx` (create if missing) that renders `<ColumnView>` with a task, invokes the "Do This Next" context-menu item, and asserts the dispatch layer sees `cmd: "task.doThisNext"` with a scope chain containing the clicked task moniker (no `column`, no `before_id` args — the backend resolves those).
- [ ] Add a browser-mode integration test in `kanban-app/ui/src/components/board-integration.browser.test.tsx` (already exists) that seeds a board with 3 columns and a task in the middle column, clicks the task's "Do This Next" context-menu item, and asserts the task ends up at the top of the order-0 column after the state refresh.
- [ ] Manual regression check: open the running app, rapidly invoke "Do This Next" on three tasks from different columns — all three should land at the top of column 0 in last-click-wins order (reverse of click order = bottom of the newly-stacked trio). Verify there is no visible flicker / mass re-render in the column now that `extraCommands` is stable.

### Design note: suppressing the command on the already-first task

The deleted frontend code hid the command when `taskId === firstTodoTaskId`. The backend does not suppress; a Do-This-Next on the already-first task is effectively a no-op move. That's fine — showing the menu item on a task that's already first is marginally noisy but not incorrect, and adding a schema-level `visible_when` predicate is out of scope for this card. If the user wants the suppression back, open a separate card for a generic "command visibility predicate" mechanism.

## Acceptance Criteria

- [ ] Invoking "Do This Next" from a task's context menu moves that task to the top of the order-0 column, no matter what the column's id is (not hardcoded to `"todo"`).
- [ ] The task is placed before any existing task in that column (its `position_ordinal` sorts before the previously-first task's ordinal).
- [ ] Rapidly invoking "Do This Next" on N different tasks results in all N tasks at the top of the order-0 column with the last-clicked task in position 0.
- [ ] Sibling task cards in the same column do NOT re-render when Do-This-Next is invoked on one of them (verified by React DevTools profiler or a test-only render counter).
- [ ] `firstTodoTaskId`, `buildDoThisNextCommand`, `taskExtraCommands`, and the `extraCommands` prop plumbing from column → virtualized list → card are all removed from `column-view.tsx` and `board-view.tsx`.
- [ ] `task.doThisNext` is dispatched by the new path and reaches `DoThisNextCmd::execute` in the Rust backend (verify via existing tracing or a new trace log).

## Tests

- [ ] `kanban-app/ui/src/components/column-view.test.tsx` (new) — `renders ColumnView, invokes "Do This Next" on a non-first task, asserts mock dispatch receives cmd="task.doThisNext" with scopeChain containing the task moniker and no column/before_id args`.
- [ ] `kanban-app/ui/src/components/board-integration.browser.test.tsx` — add test `do this next moves middle-column task to top of first column`: seed 3 columns + task in col 1, invoke command, await refresh, assert task's `position_column` in DOM matches col-0 id and is the first child of col-0's list.
- [ ] `kanban-app/ui/src/components/column-view.test.tsx` — add re-render guard: render column with 5 tasks, invoke Do-This-Next on task[3], assert that the `DraggableTaskCard` memoized render count for tasks[0..2] is unchanged (use `vi.fn()` passed through a test-only `onRender` prop or spy on `React.memo` behavior via `renderCount` ref).
- [ ] `cargo nextest run -p swissarmyhammer-kanban -- do_this_next` still passes (these are already green; the change is UI-side only, but run them to confirm no accidental regression in the Rust crate).
- [ ] `cd kanban-app/ui && bun run test` passes with the new frontend tests green.
- [ ] `cd kanban-app/ui && bun run test:browser` passes the new browser-mode integration test.

## Workflow
- Use `/tdd` — write failing tests first (start with the unit test that asserts `cmd: "task.doThisNext"` is dispatched, which will fail against the current workaround that dispatches `task.move`), then delete the workaround to make them pass.
