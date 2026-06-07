---
assignees:
- claude-code
depends_on:
- 01KTECWA8D05FVKJ80MA3H0FFY
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

## PRIME SUSPECT (added during dedup sweep): residual after a2002c330 → window-moniker
Commit `a2002c330` (2026-06-05) ALREADY repaired `view.set` ROUTING: "view.set was also broken — the cutover repointed it to the views server's `set view` (a ViewDef definition write), dropping the per-window active-view recording entirely. view.set now routes to ui_state `set active_view`, which records the active view AND rewrites the scope chain's `view:*` monikers." Yet the user still observes view switching broken AFTER that commit → this is a **residual**, not the routing bug a2002c330 fixed.

Most likely residual cause: `set active_view` is a **per-window** ui_state op, and `window_from_scope` silently defaults to the `main` window when the scope chain lacks a `window:<label>` moniker (see harden card `01KTECWA8D05FVKJ80MA3H0FFY`). If the view-switch dispatch's scope chain has no window moniker, `set active_view` records the active view on the `main` slot while the real board window's `active_view_id` never changes — so the rendered view and highlight don't move. **Check this first**: log the scope chain on a `view.set` dispatch and confirm it carries `window:<label>`.

## Other candidate causes
1. Active-view read path — `view.set` succeeds but the `ui-state-changed` event (kind `active_view`) doesn't reach `useViews().activeView`. Cross-check `01KTCQF326` (ui-state-changed must `emit_to(window_label)`, not global emit) — a global emit could update the wrong window's listener.
2. `view.set` rejects/unregistered — verify it's the `ui-commands`/`kanban-misc` plugin `view.set` and accepts `view_id` (logs: `subsystem == "com.swissarmyhammer.kanban"`).

## Related / coordinate (do not duplicate)
- `01KTECWA8D05FVKJ80MA3H0FFY` — window-moniker harden (likely shared root cause).
- `01KTCQF326FAQTQMHVV5QPG8VZ` — per-window emit_to.
- Card H `01KTED8XDX4728QR4WT9EZ0WRF` — removes the `view.switch:${id}` client indirection in the SAME view-switch path; coordinate so the bug fix and the refactor don't collide.

## Acceptance Criteria
- [ ] Clicking a view in the left-nav switches the active view: the rendered content changes and the active highlight moves.
- [ ] Switching via the command palette also works.
- [ ] Root cause identified (window-moniker default-to-main vs. active-view read/emit vs. handler).

## Tests
- [ ] Extend a views test (`apps/kanban-app/ui/src/components/views-container.test.tsx` and/or `left-nav.browser.test.tsx`) to click a view button, assert `view.set` dispatched with the right `view_id`, AND the rendered active view changes.
- [ ] Backend: `set active_view` with a non-"main" `window:` scope chain updates THAT window's `active_view_id` (not `main`) and emits `active_view`.
- [ ] Regression test failing before the fix, passing after.

## Workflow
- Use `/tdd` — failing test first, then fix. #bug