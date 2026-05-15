---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffaf80
title: Add indeterminate progress bar to NavBar during long operations
---
## What

Add a thin, horizontal indeterminate progress bar that appears at the bottom edge of the `NavBar` (`kanban-app/ui/src/components/nav-bar.tsx`) whenever a backend command is in-flight. This gives the user immediate visual feedback during slow operations like `task.move` (drag-and-drop, "Do This Next").

**Approach:**
1. Add an `isBusy` signal to `useDispatchCommand` (`kanban-app/ui/src/lib/command-scope.tsx`) that is `true` between dispatch and response for any Tauri IPC call via `invoke("dispatch_command", ...)`.
2. Surface this via a React context (`CommandBusyContext`) so any component can read it.
3. In `NavBar`, render a CSS-animated indeterminate progress bar (`<div>` with `animate-indeterminate` keyframe) at `position: absolute; bottom: 0` of the `<header>` when `isBusy` is true.
4. The bar should be ~2px tall, use the accent color, and animate left-to-right continuously.

**Files to modify:**
- `kanban-app/ui/src/lib/command-scope.tsx` — add busy tracking around `invoke()` calls
- `kanban-app/ui/src/components/nav-bar.tsx` — render the progress bar conditionally
- `kanban-app/ui/src/index.css` (or Tailwind config) — add `animate-indeterminate` keyframe if not using a library

**Existing patterns:**
- Board loading uses `Loader2` spinner in `board-container.tsx` (full-page, not what we want)
- Init progress uses sonner toasts in `init-progress-listener.tsx` (notifications, not inline)
- No existing indeterminate bar pattern — this is new UI

## Acceptance Criteria
- [ ] A thin (~2px) indeterminate progress bar appears at the bottom edge of the NavBar when any `dispatch_command` Tauri IPC call is in flight
- [ ] The bar disappears when the command completes (success or error)
- [ ] The bar is visible during drag-and-drop card moves and "Do This Next" operations
- [ ] The bar does not block interaction — the board remains usable while it's shown
- [ ] No visible layout shift when the bar appears/disappears (use absolute positioning)

## Tests
- [ ] Unit test in `kanban-app/ui/src/lib/__tests__/command-scope.test.ts`: mock `invoke`, dispatch a command, assert `isBusy` transitions true→false
- [ ] Component test for NavBar: when `CommandBusyContext` provides `isBusy=true`, the progress bar element is in the DOM; when false, it is not
- [ ] `pnpm test` passes with no regressions

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.