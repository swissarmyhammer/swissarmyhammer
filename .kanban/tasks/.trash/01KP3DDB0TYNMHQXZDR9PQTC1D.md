---
assignees:
- wballard
position_column: todo
position_ordinal: c680
title: Perspective switch should show indeterminate progress bar
---
## What

Clicking a perspective tab in `PerspectiveTabBar` does not visually trigger the indeterminate progress bar in `NavBar`. Verify the dispatch path, align with the project's idiomatic `useDispatchCommand` pattern, and ensure the busy-state indicator is actually shown for fast commands.

### Current flow

- `kanban-app/ui/src/components/perspective-tab-bar.tsx` — `TabButton` `onClick` calls `onSelect()` → `setActivePerspectiveId(p.id)` (around where `ScopedPerspectiveTab` is rendered inside the tab map).
- `kanban-app/ui/src/lib/perspective-context.tsx` — `setActivePerspectiveId` uses the **generic** form of `useDispatchCommand()`:

  ```tsx
  const dispatch = useDispatchCommand();
  const setActivePerspectiveId = useCallback(
    (id: string) => {
      dispatch("ui.perspective.set", {
        args: { perspective_id: id },
      }).catch(console.error);
    },
    [dispatch],
  );
  ```

- `kanban-app/ui/src/lib/command-scope.tsx` — `useDispatchCommand` increments `inflightCount` via `CommandBusySetterContext` only when the command resolves to the **backend** path (no client-side `execute`). `ui.perspective.set` has no client-side registration, so it should take the backend path.
- `kanban-app/ui/src/components/nav-bar.tsx` — reads `useCommandBusy()`; renders the `animate-indeterminate` bar while `isBusy === true`.
- Backend handler: `swissarmyhammer-kanban/src/commands/ui_commands.rs` — `SetActivePerspectiveCmd::execute` mutates `UIState` in memory only. The Tauri IPC round-trip is likely fast enough that `inflightCount` goes 0 → 1 → 0 inside a single paint, so the bar never visibly renders.

### Tasks

- [ ] Verify empirically that clicking a perspective tab does or does not flip `isBusy` in `CommandBusyProvider`. Add a temporary `console.warn` in `command-scope.tsx` around `setInflightCount` to confirm the backend path is taken and the counter increments (check the macOS unified log — `log show --predicate 'subsystem == "com.swissarmyhammer.kanban"'`).
- [ ] Refactor `kanban-app/ui/src/lib/perspective-context.tsx` to use the **pre-bound** form of `useDispatchCommand` to match the idiomatic codebase pattern (`quick-capture.tsx`, `column-view.tsx`):

  ```tsx
  const dispatchPerspectiveSet = useDispatchCommand("ui.perspective.set");
  const setActivePerspectiveId = useCallback(
    (id: string) => {
      dispatchPerspectiveSet({ args: { perspective_id: id } }).catch(console.error);
    },
    [dispatchPerspectiveSet],
  );
  ```

  Do the same for the `perspective.save` auto-create call in the same file for consistency.
- [ ] If after step 1 the dispatch IS hitting the backend path but the bar still doesn't render, add a **minimum display duration** to `CommandBusyProvider` in `kanban-app/ui/src/lib/command-scope.tsx` so fast commands still flash the bar. Target: when `inflightCount` transitions 0 → ≥1, keep `isBusy` true for at least 200ms after it drops back to 0. Use a timeout + ref; don't leak if the provider unmounts.

### Out of scope

- Do not change `SetActivePerspectiveCmd` on the Rust side — the handler is correct.
- Do not alter the perspective UI/layout.

## Acceptance Criteria

- [ ] Clicking a perspective tab visibly shows the indeterminate bar at the bottom of `NavBar` for at least ~200ms.
- [ ] `perspective-context.tsx` uses `useDispatchCommand("ui.perspective.set")` pre-bound form; matches the pattern used in `quick-capture.tsx` and `column-view.tsx`.
- [ ] Busy state behavior is consistent for other short in-memory commands (`ui.view.set`, `ui.mode.set`): they also flash the bar briefly now.
- [ ] `perspective-context.test.tsx` still passes — the new test (below) proves the busy state transitions.
- [ ] `bun run typecheck` and `bun run test` pass.
- [ ] Manual verification: launch the app, click through perspective tabs, observe the bar flashing under the nav bar on each click.

## Tests

- [ ] Update `kanban-app/ui/src/lib/perspective-context.test.tsx` — the existing "dispatches ui.perspective.set to backend" test still asserts the backend IPC is called; update the dispatch assertion if the signature changes due to the pre-bound refactor (args-only payload).
- [ ] Add a test in `kanban-app/ui/src/lib/command-scope.test.tsx` under the existing `CommandBusyProvider` describe block that asserts:
  1. `isBusy` becomes true when a backend dispatch is in-flight.
  2. After the dispatch resolves, `isBusy` stays true for the configured minimum duration (~200ms), then drops to false.
  3. If a second dispatch starts during the hold window, `isBusy` remains true seamlessly.
  - Use `vi.useFakeTimers()` + `advanceTimersByTime` to assert the minimum-duration behavior.
- [ ] Add an integration-style test that renders `NavBar` + `PerspectiveTabBar` inside `CommandBusyProvider`, clicks a tab (mocking the invoke to resolve immediately), and asserts `role="progressbar"` becomes visible and then hides — place it in `kanban-app/ui/src/components/perspective-tab-bar.test.tsx`.
- [ ] Run `bun run test -- perspective-context command-scope perspective-tab-bar` and expect all tests green.

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass. #bug #unified-commands