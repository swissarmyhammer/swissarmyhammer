---
assignees:
- claude-code
position_column: todo
position_ordinal: ca80
title: 'Bug: Cannot switch between views (view.set has no effect)'
---
## What
Reported by user: switching between views does not work — selecting a different view does not change the displayed view.

View switching dispatches the canonical `view.set` command with `{ view_id }`:
- Left-nav button: `ViewButton` in `apps/kanban-app/ui/src/components/left-nav.tsx:121` — `onPress` calls `dispatch({ args: { view_id: view.id } })` for `view.set` (left-nav.tsx:122,132). Errors are swallowed by `.catch(console.error)`.
- Active view state comes from `useViews()` → `activeView` (left-nav.tsx:44; context in `apps/kanban-app/ui/src/lib/views-context.tsx`). `isActive={activeView?.id === view.id}` (left-nav.tsx:60).
- The palette fan-out that used to emit `view.switch:{id}` was retired; `view.set` is now the only path.
- Rendered views live in `apps/kanban-app/ui/src/components/views-container.tsx`.

Candidate root causes to check:
1. **`view.set` rejects / isn't registered** — the backend `view.set` command handler is missing, errors, or doesn't accept `view_id`. The `.catch(console.error)` hides this; check logs (`log show --predicate 'subsystem == "com.swissarmyhammer.kanban"'`).
2. **Active-view state not updated** — `view.set` succeeds but the change/event that updates `useViews().activeView` is not emitted or not consumed, so `views-container` keeps rendering the old view.
3. **Scope resolution** — `useDispatchCommand("view.set")` resolves board/window identity from the scope chain; if the chain is wrong the dispatch targets nothing.

NOTE: may share a root cause with the other command-dispatch / focus-surfacing bugs reported in this batch (palette, nav menu, drag-drop). Cross-check whether `view.set` is reaching the backend at all.

Reproduce: open a board with ≥2 views, click a non-active view in the left-nav; observe the active highlight and rendered view do not change.

## Acceptance Criteria
- [ ] Clicking a view in the left-nav switches the active view: the rendered content changes and the active highlight moves.
- [ ] Switching via the command palette also works.
- [ ] Root cause identified (handler missing/erroring vs. active-view state not updating vs. scope resolution).

## Tests
- [ ] Extend a views test (`apps/kanban-app/ui/src/components/views-container.test.tsx` and/or `left-nav.browser.test.tsx`) to click a view button, assert `view.set` is dispatched with the right `view_id`, AND assert the rendered active view changes.
- [ ] If backend: integration test that `view.set` updates the active view in UI/board state.
- [ ] Regression test failing before the fix, passing after.

## Workflow
- Use `/tdd` — failing test first, then fix. #bug