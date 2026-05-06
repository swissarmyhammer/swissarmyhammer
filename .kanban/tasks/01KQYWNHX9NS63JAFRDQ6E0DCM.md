---
assignees:
- claude-code
position_column: todo
position_ordinal: df80
project: keyboard-navigation
title: Disable focus-debug overlays in production
---
## What

The dashed colored boxes around every Layer/Zone/Scope and the small colored corner handle ("the dot that shows the xy debug") are both rendered by `<FocusDebugOverlay>` (`kanban-app/ui/src/components/focus-debug-overlay.tsx`). Visibility is gated by `useFocusDebug()`, which reads from `<FocusDebugProvider>`.

Today both production windows mount with the overlay ENABLED:
- `kanban-app/ui/src/App.tsx:98` — main app: `<FocusDebugProvider enabled>`
- `kanban-app/ui/src/App.tsx:153` — quick-capture window: `<FocusDebugProvider enabled>`

The Jump-To overlay supersedes the need for these debug visualizations. Turn them off:

1. Flip both mount sites to `<FocusDebugProvider enabled={false}>`. Do NOT remove the provider — leaving it in place keeps the wiring symmetrical and the prop ergonomic if a developer wants to toggle it on locally.
2. Update `kanban-app/ui/src/spatial-nav-end-to-end.spatial.test.tsx` (lines 556-579) — this test currently asserts the overlay renders to verify provider mounting. Rewrite the assertion to either:
   - mount its own `<FocusDebugProvider enabled>` wrapping the test tree (test-local override), or
   - remove the assertion and replace it with a check that `<FocusScope>` hosts have rendered without an overlay child element when the provider is disabled.
   Pick whichever fits cleaner with the rest of that file.
3. Search the rest of the test suite for any test that *implicitly* relied on the overlay being visible — `grep -r "focus-debug-overlay" kanban-app/ui/src/**/*.test.*` — and fix any breakage.
4. Search for any other `FocusDebugProvider` mounts in the codebase that should also be flipped off (e.g., a Storybook entry, a test harness with `enabled`).

Do NOT add a runtime preference / settings toggle for this in this task — that's scope creep. The overlay is a developer-only aid; if a developer wants it back, they edit the prop locally.

## Acceptance Criteria

- [ ] `kanban-app/ui/src/App.tsx:98` and `App.tsx:153` both mount `<FocusDebugProvider enabled={false}>`.
- [ ] No dashed border, no colored corner handle, no `(x,y)` label appears on any focusable element when running `pnpm tauri dev`.
- [ ] No other `FocusDebugProvider` in production code paths is `enabled` (test files may mount their own with `enabled` for test-local needs).
- [ ] `kanban-app/ui/src/spatial-nav-end-to-end.spatial.test.tsx` still passes with the new defaults.
- [ ] `kanban-app/ui/src/components/focus-debug-overlay.browser.test.tsx` still passes (it mounts its own provider).

## Tests

- [ ] Update `kanban-app/ui/src/spatial-nav-end-to-end.spatial.test.tsx` — assertion rewritten as described.
- [ ] Add a regression test `kanban-app/ui/src/App.no-debug-overlay.browser.test.tsx`: render `<App />` (or a minimal slice with `<FocusScope>` underneath the App's provider tree), query the document for any element with `data-focus-debug-kind` (or whatever attribute the overlay uses; if there isn't one, add one in this task to make the assertion testable), assert zero matches.
- [ ] Test command: `cd kanban-app/ui && pnpm test spatial-nav-end-to-end App.no-debug-overlay focus-debug-overlay` — all three pass.

## Workflow

- Use `/tdd` — add the new `App.no-debug-overlay` regression test first (it will fail because overlays still render), then flip the prop and update the e2e test. #nav-jump