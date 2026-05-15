---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffad80
title: 'Fix: "Add task" Plus button in todo column appears to do nothing'
---
## What

Bug: clicking the Plus (+) button in the todo column header does not produce any visible result. The user sees no new card, no editor, and no error feedback.

**Root cause investigation points (in order of likelihood):**

1. **Silent error swallowing** — The click handler in `kanban-app/ui/src/components/column-view.tsx` (~line 460) dispatches `task.add` with `.catch(console.error)`. If the Rust backend rejects the command (e.g., `AddTaskCmd.available()` fails because `KanbanContext` isn't in scope), the error goes to the dev console and the user sees nothing.

2. **Event not received by UI** — After Rust creates the task, it emits an `entity-created` Tauri event. `RustEngineContainer` (`kanban-app/ui/src/components/rust-engine-container.tsx` ~line 225) listens for this but silently skips events where `board_path` doesn't match `activeBoardPathRef.current`. A mismatch here would mean the task is created on disk but never appears in the UI.

3. **No inline editor opens** — Even if the task IS created successfully, it gets the hardcoded title `"New task"` and there's no follow-up action to open an inline title editor or select the new card. The user may not notice a new card appearing at the bottom of a long todo column.

**The fix should address all three concerns:**
- Surface dispatch errors as toast notifications (use the existing sonner toast pattern from `init-progress-listener.tsx`)
- After successful `task.add`, scroll the new card into view and open it for editing (focus the title)
- Add diagnostic logging to confirm the event pipeline is working

**Files to modify:**
- `kanban-app/ui/src/components/column-view.tsx` — the Plus button click handler: replace `.catch(console.error)` with toast error feedback, and on success scroll-to + open the new card
- `kanban-app/ui/src/components/board-view.tsx` — the `handleAddTask` function that provides the `onAddTask` prop (ensure the new task ID is propagated back so we can scroll to it)
- `kanban-app/ui/src/components/rust-engine-container.tsx` — verify `entity-created` event matching; add a warning log if `board_path` doesn't match

**Existing patterns to follow:**
- Toast notifications: `sonner` is already used in `init-progress-listener.tsx`
- Command dispatch: `useDispatchCommand` in `command-scope.tsx` returns a promise with the result

## Acceptance Criteria
- [ ] Clicking the Plus button in the todo column creates a new task that is immediately visible in the UI
- [ ] The newly created task is scrolled into view and opened for title editing
- [ ] If the `task.add` dispatch fails, a toast notification shows the error message (not just console.error)
- [ ] The fix works regardless of column ordering (the Plus button currently only renders on `i === 0` in `board-view.tsx` line 519 — this is fine as long as todo is first, but document the assumption)

## Tests
- [ ] Test in `kanban-app/ui/src/components/__tests__/column-view.test.tsx`: clicking the Plus button dispatches `task.add` with `{ title: "New task", column: "todo" }` and the new card appears in the DOM
- [ ] Test in `kanban-app/ui/src/components/__tests__/column-view.test.tsx`: when `task.add` dispatch rejects, a toast error is shown (mock sonner toast)
- [ ] `pnpm test` passes with no regressions

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.