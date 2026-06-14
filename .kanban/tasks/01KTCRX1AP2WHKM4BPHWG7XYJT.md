---
assignees:
- claude-code
depends_on:
- 01KTECWA8D05FVKJ80MA3H0FFY
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffff8e80
title: 'Bug: Cannot switch between views (view.set has no effect)'
---
## What
Reported by user: switching between views does not work — selecting a different view does not change the displayed view.

View switching dispatches the canonical `view.set` command with `{ view_id }`:
- Left-nav button: `ViewButton` in `apps/kanban-app/ui/src/components/left-nav.tsx` — `onPress` calls `dispatch({ args: { view_id: view.id } })` for `view.set`. Errors are swallowed by `.catch(console.error)`.
- Active view state comes from `useViews()` → `activeView` (context in `apps/kanban-app/ui/src/lib/views-context.tsx`), derived from `UIState.windows[<label>].active_view_id`.
- Rendered views live in `apps/kanban-app/ui/src/components/views-container.tsx`.

## RESOLUTION (2026-06-10)

**Current state: the bug is fixed by an already-landed chain of fixes; this card closed the remaining test gap.** Traced the path end to end:

1. **Dispatch (frontend)** — `ViewButton` → `useDispatchCommand("view.set")`. The scope chain is window-rooted by construction: tree scope walks `view:{id}` (ScopedViewButton) → `ui:left-nav` → … → `window:{label}` (`WindowContainer`'s `CommandScopeProvider` in App.tsx); focused-scope chains also terminate at `window:{label}` because `FocusScope` links `parent = CommandScopeContext` at its mount point.
2. **Routing (fixed in a2002c330)** — `view.set` is registered by the `kanban-misc-commands` builtin plugin and routes to ui_state `set active_view` (per-window active-view recording + `view:*` scope-chain rewrite), not the views server's definition write.
3. **Window resolution (hardened by 01KTECWA8D05FVKJ80MA3H0FFY, in working tree)** — `window_from_scope` now REQUIRES a `window:<label>` moniker and errors loudly instead of silently writing to `main`. `crates/swissarmyhammer-ui-state/tests/integration/ui_state_e2e.rs` pins reject + targets-the-named-window; `builtin_kanban_misc_e2e.rs` (3d) pins the full plugin path with a non-"main" window.
4. **Emit (fixed in af9e6e965)** — `emit_ui_state_change_if_needed` unwraps the `{ok,change}` envelope at `structuredContent.change`, classifies `ActiveView` → `kind: "active_view"`, and `emit_to`s every webview (6a07f9c4b). Unit-pinned by `ui_state_change_kind_active_view` in `apps/kanban-app/src/commands.rs`.
5. **Apply (frontend)** — `UIStateProvider` applies `active_view` events (`active_view` is NOT in `FRONTEND_AUTHORITATIVE_KINDS`); `ViewsProvider` reads this window's `active_view_id` slice; highlight + `ViewContainer` re-render.

**Root cause (of the original report)**: composite — the a2002c330 routing bug (view.set wrote a ViewDef definition instead of the per-window active view) plus the af9e6e965 emit bug (`{ok,change}` envelope never classified → no `ui-state-changed` event reached the webview). Both landed before this card was picked up; the per-window hardening (blocker card) converted the remaining silent-default-to-main failure class into a loud error.

**Gap closed by this card**: no frontend test pinned the loop. Added `apps/kanban-app/ui/src/components/left-nav.view-switch.browser.test.tsx` (real `UIStateProvider` + `ViewsProvider` + `LeftNav`, no views-context mocking):
- *Producer guarantee*: clicking a view button dispatches `view.set` with `args {view_id}` AND a scope chain carrying `window:main` + `view:{id}` (the hardened backend rejects without `window:`, and the click handler swallows the error — silent re-break vector). Red verified by simulating `scopeChain: []` in the dispatch → fails on the missing `window:main`.
- *Consumer loop*: a backend `ui-state-changed` `{kind: "active_view"}` event moves the rendered active highlight. Red verified by adding `active_view` to `FRONTEND_AUTHORITATIVE_KINDS` (candidate cause 1) → fails on the unmoved highlight.

**Verification**: scoped vitest 21/21 (new file + left-nav.browser + views-container + view-container + ui-state-context), `tsc --noEmit` clean, `cargo nextest -p swissarmyhammer-ui-state` 136/136, `-p swissarmyhammer-command-service` 125/125.

**Palette path**: palette rows emit `view.set` with pre-filled args through the same `useDispatchCommand` ambient-scope mechanism under `WindowContainer`, so the same producer guarantee covers it; the backend half is pinned by `builtin_kanban_misc_e2e`.

## Related / coordinate (do not duplicate)
- `01KTECWA8D05FVKJ80MA3H0FFY` — window-moniker harden (landed; shared root-cause class).
- `01KTCQF326FAQTQMHVV5QPG8VZ` — per-window emit_to (landed).
- Card H `01KTED8XDX4728QR4WT9EZ0WRF` — removes the `view.switch:${id}` client indirection in the SAME view-switch path; the new test exercises `view.set` directly and is refactor-safe.

## Acceptance Criteria
- [x] Clicking a view in the left-nav switches the active view: the rendered content changes and the active highlight moves.
- [x] Switching via the command palette also works.
- [x] Root cause identified (window-moniker default-to-main vs. active-view read/emit vs. handler) — composite: routing (a2002c330) + emit envelope (af9e6e965), with the window-moniker default hardened into a loud error.

## Tests
- [x] Frontend: `left-nav.view-switch.browser.test.tsx` clicks a view button, asserts `view.set` dispatched with the right `view_id` + window-rooted scope chain, AND the rendered active view changes on the backend event.
- [x] Backend: `set active_view` with a non-"main" `window:` scope chain updates THAT window's `active_view_id` (not `main`) — `ui_state_e2e::per_window_op_targets_the_scope_chain_window_not_main` + `builtin_kanban_misc_e2e` (3d); emit classification pinned by `ui_state_change_kind_active_view`.
- [x] Regression test failing before the fix, passing after — red verified by revert-simulation of both candidate causes (empty scope chain; suppressed active_view kind).

## Workflow
- Use `/tdd` — failing test first, then fix. #bug