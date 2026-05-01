---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffff9080
title: Remove 'Switch to <View>' entries from context menus
---
## What

The left-nav view buttons currently show a `Switch to <ViewName>` entry in their right-click context menu. This should not appear — view switching is a palette-only / left-click action, not a context-menu action.

`view.switch:{id}` is the only "navigation dynamic" that leaks into the context menu. Its sibling emitters already set `context_menu: false`:

- `emit_board_switch` — palette-only
- `emit_window_focus` — palette-only
- `emit_perspective_goto` — palette-only
- `emit_view_switch` — **currently `context_menu: in_scope`** (the outlier)

### Files to modify

- **`swissarmyhammer-kanban/src/scope_commands.rs`**
  - `emit_view_switch` (near line 247): change `context_menu: in_scope` to `context_menu: false`. Drop the `scope_chain: &[String]` parameter and its callers — it is no longer read. Update the doc comment that describes the "one Switch to <ViewName> entry" contract.
  - Test `view_switch_context_menu_only_emits_in_scope_view` (near line 1800): flip assertions — no `view.switch:*` should appear when `context_menu_only == true`.
  - Test `view_switch_palette_still_emits_all_views` (near line 1858): unchanged — palette must still emit every `view.switch` command.

- **`kanban-app/ui/src/components/left-nav.tsx`**
  - Update the stale docstring on `LeftNav` / `ScopedViewButton` / `ViewButton` that claims the context menu shows `"Switch to <ViewName>"`. The `CommandScopeProvider` wrapper and `useContextMenu` wiring stay — other dynamic commands (e.g. `entity.add:{type}` for views that declare an `entity_type`) still need the `view:{id}` moniker in scope to surface correctly.

- **`kanban-app/ui/src/components/left-nav.browser.test.tsx`**
  - `it("shows native context menu with the backend-supplied Switch to <view> entry", ...)` (near line 131): rewrite — the backend should no longer return a `view.switch:*` context-menu entry. Replace with a test that proves the view button's right-click does not surface `view.switch:*` (and, if desired, that an `entity.add:*` entry still surfaces when the view declares an `entity_type`).
  - `it("right-click on a view button queries commands with that view's scope", ...)`: unchanged — the scope-chain contract still holds.

### Out of scope

- Perspective-tab-bar right-click (`perspective-tab-bar.context-menu.test.tsx`) uses a different mechanism and is not affected.
- Command palette behavior (`context_menu_only == false`) must remain identical — `view.switch:{id}` commands still appear in the palette with names like `Switch to <ViewName>`.

## Acceptance Criteria

- [x] Right-clicking on any view button in the left-nav does not surface a `Switch to <ViewName>` menu item.
- [x] The command palette (e.g. Cmd+K) still lists every `Switch to <ViewName>` entry — one per known view.
- [x] `emit_view_switch` emits `context_menu: false` unconditionally; no view is treated specially by virtue of being the in-scope one.
- [x] Doc comments on `emit_view_switch` and `LeftNav` / `ViewButton` no longer claim a context-menu entry is produced.
- [x] `cargo test -p swissarmyhammer-kanban scope_commands` passes.
- [x] `pnpm -C kanban-app/ui test left-nav.browser` passes.

## Tests

- [x] Update `swissarmyhammer-kanban/src/scope_commands.rs::view_switch_context_menu_only_emits_in_scope_view` — assert `!ids.iter().any(|id| id.starts_with("view.switch:"))` when `context_menu_only == true`, regardless of what `view:*` moniker is in the scope chain.
- [x] `swissarmyhammer-kanban/src/scope_commands.rs::view_switch_palette_still_emits_all_views` — unchanged; acts as regression guard for the palette.
- [x] Update `kanban-app/ui/src/components/left-nav.browser.test.tsx::shows native context menu with the backend-supplied Switch to <view> entry` — rewrite so the `list_commands_for_scope` mock returns no `view.switch:*` and the assertion verifies the native menu contains no such item.
- [x] Command to run: `cargo test -p swissarmyhammer-kanban --lib scope_commands` — all tests pass.
- [x] Command to run: `pnpm -C kanban-app/ui test --run left-nav.browser` — all tests pass.

## Workflow

- Use `/tdd` — flip the Rust test assertion first (expect no `view.switch:*` in context-menu results), watch it fail, then make `emit_view_switch` emit `context_menu: false`. Mirror the process on the TS side. #commands