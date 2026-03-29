---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffa880
title: Add "Do This Next" context menu item to promote task to top of todo
---
## What

Add a \"Do This Next\" context menu command on task cards that moves the task to the top of the todo column with one click. This is a frontend-only command — it calls the existing `task.move` backend command with `column: \"todo\"` and a `before_id` of the current first task in todo.

### Approach

The command needs board context (column list, task ordering) that only the board view has. Use the `extraCommands` pattern on `useEntityCommands` to inject a frontend-defined command from the board/column level.

### Files to modify

1. **`kanban-app/ui/src/components/entity-card.tsx`** — accept an optional `extraCommands` prop and pass it to `useEntityCommands` at line ~66
2. **`kanban-app/ui/src/components/column-view.tsx`** — build a \"Do This Next\" `CommandDef` for each task card that calls `dispatch_command` with `cmd: \"task.move\"`, `column: \"todo\"`, and `before_id` of the first todo task. Pass it as `extraCommands` to `EntityCard`. Skip adding the command if the task is already the first item in todo.
3. **`kanban-app/ui/src/components/board-view.tsx`** — pass the todo column's first task ID down to `ColumnView` so it can build the before_id arg

### How the execute function works
```ts
execute: () => {
  invoke(\"dispatch_command\", {
    cmd: \"task.move\",
    args: { id: taskId, column: \"todo\", before_id: firstTodoTaskId },
    ...(boardPath ? { boardPath } : {}),
  });
}
```

## Acceptance Criteria
- [ ] Right-click a task in any column → context menu shows \"Do This Next\"
- [ ] Clicking it moves the task to the top of the todo column
- [ ] If the task is already the first todo item, the command is not shown
- [ ] The board re-renders with the task at the top of todo

## Tests
- [ ] `kanban-app/ui/src/lib/entity-commands.test.ts` — verify extraCommands appear in context menu output
- [ ] Visual: right-click a task in \"doing\" → \"Do This Next\" appears → click → task moves to top of todo
- [ ] Run: `cd kanban-app/ui && npx vitest run` — all tests pass