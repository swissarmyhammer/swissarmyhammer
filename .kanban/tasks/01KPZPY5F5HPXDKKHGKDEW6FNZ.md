---
assignees:
- claude-code
depends_on:
- 01KPZWP4YTYH76XTBH992RV2AS
position_column: done
position_ordinal: ffffffffffffffffffffffff9f80
title: Stop refetching perspectives on every focus change — use view scope + backend events
---
## What

`PerspectiveProvider` refetches the full perspective list from the backend on every focus change anywhere in the app. The log shows `perspective.list` firing ~700 ms after every `ui.setFocus` (arrow key move, cell click, etc.), which keeps the nav-bar `isBusy` progress bar ticking and issues a backend round-trip per keystroke. Perspectives are per-view, not per-focused-element, and should only refetch on mount (once per view kind) and in response to backend events that already exist.

**Root cause** — identity-of-dispatch leak:
`kanban-app/ui/src/lib/perspective-context.tsx::usePerspectivesFetch`:
```ts
const refresh = useCallback(async () => { ...dispatch("perspective.list")... }, [dispatch]);
useEffect(() => { refresh(); }, [refresh]);
```
`dispatch` comes from `useDispatchCommand()` in `kanban-app/ui/src/lib/command-scope.tsx::useDispatchCommand`, whose `useCallback` depends on `effectiveScope = focusedScope ?? treeScope`. `focusedScope` updates from `FocusedScopeContext` on every entity focus change. So every focus change → new `dispatch` identity → new `refresh` identity → mount-effect re-runs → full `perspective.list` IPC.

**Evidence** — `log show --predicate 'subsystem == "com.swissarmyhammer.kanban"'` shows `[filter-diag] perspective REFRESH (full refetch)` and a matching backend `cmd=perspective.list` after every `cmd=grid.moveUp` / `grid.moveDown` / `ui.setFocus`. The same hot pattern also churns sibling effects whose deps include `dispatch`: `useAutoCreateDefaultPerspective` and `useAutoSelectActivePerspective`.

**Approach** — decouple the mount fetch and event-driven refetches from `dispatch` identity:

- Capture `dispatch` in a `useRef` (`dispatchRef`) inside `PerspectiveProvider` and have `refresh` read `dispatchRef.current` instead of closing over `dispatch`. Memoize `refresh` with stable deps (empty or `[]`). The mount `useEffect(() => { refresh(); }, [refresh])` then fires exactly once per `PerspectiveProvider` mount.
- Apply the same `dispatchRef` treatment to `useAutoCreateDefaultPerspective` and `useAutoSelectActivePerspective` so they don't re-run on every focus change either. Their guard-and-early-return behavior is correct; only their dep lists need to stop churning.
- Keep the existing `usePerspectiveEventListeners` as-is — `entity-field-changed` / `entity-created` / `entity-removed` for perspective, and `board-changed`, all remain the refetch signals. That is the authorization the user called out: "when an event comes from the backend indicating I'm ok to refetch all perspectives, just to be thorough."
- Do NOT keep the perspective-field delta fast path (kanban branch already removed it intentionally; backend emits null-value changes for perspectives). Refetch on event is correct; refetch on focus is not.

**Files**
- `kanban-app/ui/src/lib/perspective-context.tsx` — primary change (refresh / auto-create / auto-select).
- `kanban-app/ui/src/lib/perspective-context.test.tsx` — add regression test.

### Subtasks
- [x] Refactor `usePerspectivesFetch` to hold `dispatch` in a ref; make `refresh` stable (no `dispatch` dep).
- [x] Refactor `useAutoCreateDefaultPerspective` to use the same `dispatchRef` pattern.
- [x] Refactor `useAutoSelectActivePerspective` to use the same `dispatchRef` pattern.
- [x] Add a regression test that proves `perspective.list` fires exactly once on mount and does NOT fire when focus changes.
- [x] Confirm existing tests for event-driven refetch (`refreshes on entity-created event for perspective type`, `refetches perspective.list on entity-field-changed for a perspective`, `refreshes on board-changed event`) still pass unchanged.

## Acceptance Criteria
- [ ] Arrow-key navigation in the grid (many `ui.setFocus` dispatches in a row) produces exactly ZERO additional `perspective.list` backend dispatches. Verified by `log show --predicate 'subsystem == "com.swissarmyhammer.kanban"'` — no `cmd=perspective.list` lines after the initial mount-time one for a window, until a backend event arrives.
- [ ] `perspective.list` STILL fires in response to each of: `entity-created` for perspective, `entity-field-changed` for perspective, `entity-removed` for perspective, `board-changed`.
- [ ] `useAutoCreateDefaultPerspective` and `useAutoSelectActivePerspective` do not re-run their effect bodies on focus change (verified via test or targeted `console.warn` probe during manual smoke).
- [ ] Nav-bar progress bar does not relight on arrow-key navigation once initial load has settled.
- [ ] No change to perspective semantics: active perspective still resolves correctly on mount, auto-create still fires when no perspective exists for the view kind, auto-select still coerces a stale `active_perspective_id` to the first matching perspective.

## Tests
- [x] Add test `kanban-app/ui/src/lib/perspective-context.test.tsx::"does not refetch perspective.list when focused scope changes"`. Setup: mount `PerspectiveProvider` under a wrapper that exposes a `FocusedScopeContext.Provider` whose value toggles. Flow: render, wait for initial fetch, reset `mockInvoke`, change the focused scope value (simulating entity focus change), await microtasks, assert `mockInvoke` was NOT called with `{ cmd: "perspective.list" }`.
- [x] Test command to run: `cd kanban-app/ui && npm test -- perspective-context`. Expected: all existing tests pass plus the new case.
- [ ] Manual smoke: open the 2000-row swissarmyhammer board on the kanban branch, hold down ↓ for ~3 seconds, confirm only one `cmd=perspective.list` line appears in `log show` (the one emitted at initial mount).

## Workflow
- Use `/tdd` — write the failing "does not refetch on focus change" test first, then implement the `dispatchRef` refactor to make it pass, then extend to the two sibling hooks. #performance #perspectives #frontend #bug