---
assignees:
- claude-code
position_column: todo
position_ordinal: fc80
project: builtin-commands
title: Surface per-view "Switch to View «name»" in the View context menu (and verify palette coverage everywhere)
---
## Problem

Right-clicking a View (the left-nav view buttons) shows no usable context menu — it should offer **"Switch to View «name»" for that view**. Separately, every view should be switchable from the command palette anywhere ("Switch to View «name»"), as dynamic commands derived from the available views.

This is a deliberate-decision reversal, not a pure bug. Today view switching is **palette-only by design**:

- `crates/swissarmyhammer-kanban/src/scope_commands.rs` → `emit_view_switch` emits one dynamic `view.set` row per view (`name: "Switch to {view.name}"`, `args: { view_id }`) but with **`context_menu: false`**, so it never appears on right-click.
- `apps/kanban-app/ui/src/components/left-nav.tsx` documents this verbatim: *"View switching is palette-only, so the context menu never shows a 'Switch to <ViewName>' entry; the `view:{id}` moniker is still needed for other dynamics (e.g. `entity.add:{type}`)."* Each view button already mounts a `CommandScopeProvider moniker={view:{id}}` and reads `useContextMenu`, so the scope plumbing exists — only the backend emission withholds the entry. (See also the regression marker `apps/kanban-app/ui/src/components/view-switch-commands.retired.node.test.ts`.)

The dynamic rows are NOT collapsed by dedup — `SeenKey = (id, target, args)` includes the args serialization (`scope_commands.rs:233`), so per-view `view.set` rows (same id, distinct `view_id` args) are distinct. The per-perspective `perspective.switch` dynamic uses this exact same-id/different-args pattern and works.

## Decision: the context menu for a given view shows ONLY that view

Right-clicking view X must show exactly **one** view-switch entry — "Switch to View «X»" (X's own) — NOT a list of all views. The palette is the place to switch to *any* view; the per-view context menu is scoped to the view you clicked.

## Critical constraint

`emit_view_switch` emits **unconditionally** (all views, independent of scope) for the palette. The context-menu entry must instead be **scope-resolved to the single `view:{id}` in the scope chain** — mirror `emit_entity_add`, which resolves the view from the `view:{id}` moniker in scope and emits exactly for that view. This both confines the entry to a view context (no leaking into task/column/global menus) and yields exactly one self-referential switch entry per view button.

## What

One cohesive change: per-view "Switch to View «name»" available in (a) the View context menu — scoped to that view only — and (b) the palette everywhere.

- `crates/swissarmyhammer-kanban/src/scope_commands.rs`:
  - Keep the existing palette emission (`emit_view_switch`: all views, `context_menu: false`) so every view is palette-switchable.
  - Add a **scope-resolved** context-menu emission (e.g. `emit_view_switch_context_menu`): for the `view:{id}` moniker present in `scope_chain`, look up that one view in `dyn_src.views` (index by id, as `emit_dynamic_commands` already does for `emit_entity_add`) and emit a single `view.set` row with `context_menu: true`, `args: { view_id }`, caption "Switch to View «name»" — for THAT view only. Do not emit other views' rows in the context menu. Avoid double-naming when `view.name` already contains "View" (caption should read naturally, e.g. `Switch to {{view.name}}` if names are like "Board View", or `Switch to View {{view.name}}` if names are like "Board").
  - Wire the new emission into `emit_dynamic_commands` (alongside `emit_view_switch`).
- `apps/kanban-app/ui/src/components/left-nav.tsx`: update the now-stale "view switching is palette-only … context menu never shows a Switch to <ViewName> entry" docstring; verify `ViewButton`'s `useContextMenu` renders the scoped context_menu row for its own `view:{id}` scope.
- Verify the palette path populates `DynamicSources.views` so the palette switch rows are available "anywhere" (not only when a view is focused). `DynamicSources.views` is built by `crates/swissarmyhammer-kanban/src/dynamic_sources.rs::build_dynamic_sources`; confirm the palette's `commands_for_scope` call receives a populated `dynamic` with `.views` regardless of current focus, and fix if it passes `None`/empty.

## Acceptance Criteria
- [ ] Right-clicking view X in the left-nav shows a context menu with exactly its own "Switch to View «X»" entry (one entry, X's own — NOT entries for other views); selecting it dispatches `view.set` with X's `view_id` and switches the active view.
- [ ] The command palette lists a "Switch to View «name»" entry for every available view, dispatchable from anywhere in the app (not only while a view is focused).
- [ ] View-switch context-menu rows do NOT appear in unrelated context menus (right-click a task/column/board surface shows no view-switch entry) — the context-menu emission is scope-resolved to the `view:{id}` moniker.
- [ ] No new backend command id; reuses the existing `view.set` (kanban-misc-commands) with per-view `args.view_id` and `{{view.name}}`-templated captions.

## Tests
- [ ] `crates/swissarmyhammer-kanban/tests/dynamic_sources_headless.rs` (the headless harness that asserts the exact dynamic rows `commands_for_scope` emits): with `view:{X}` in the scope chain, assert exactly ONE `view.set` row with `context_menu: true` whose `args.view_id == X` (and assert NO other view's id appears as a `context_menu: true` row); with NO `view:` moniker in scope, assert no `context_menu: true` view-switch row at all. Keep asserting the palette (`context_menu: false`) rows for all views.
- [ ] `apps/kanban-app/ui/src/components/left-nav.view-switch.browser.test.tsx`: extend to open the context menu on view button X and assert it contains "Switch to View «X»" and NOT "Switch to View «Y»" for a different view Y; selecting it dispatches `view.set` with X's `view_id`.
- [ ] Update/replace `apps/kanban-app/ui/src/components/view-switch-commands.retired.node.test.ts` to reflect the un-retired (scoped) context-menu behavior, or delete it if fully superseded, so the retired-surface marker doesn't contradict the new behavior.
- [ ] `cargo test -p swissarmyhammer-kanban --test dynamic_sources_headless` and `cd apps/kanban-app/ui && npx vitest run src/components/left-nav.view-switch.browser.test.tsx` both pass (new assertions red before the change, green after).

## Workflow
- Use `/tdd` — add the failing headless assertion (exactly the scoped view's `context_menu:true` `view.set` when `view:{X}` in scope; none otherwise) and the failing left-nav context-menu test first, then implement the scope-resolved emission and make them pass.