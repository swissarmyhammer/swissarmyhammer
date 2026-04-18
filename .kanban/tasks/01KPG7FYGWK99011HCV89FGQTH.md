---
assignees:
- claude-code
depends_on:
- 01KPEMYJV7BMTJB6GZ8MGTD04J
position_column: todo
position_ordinal: ed80
title: 'Commands: menu bar dedupe for cross-cutting commands (Edit menu shows once, not per target)'
---
## What

Auto-emit fires `entity.cut` / `entity.copy` / `entity.paste` once per entity moniker in the scope chain. That's correct for the context menu (different targets, different actions). But the **Edit menu** in the macOS menu bar is global — it should show Cut / Copy / Paste exactly once, not three times because `[tag, task, column]` is in scope.

### The rule

A command with `menu: {path: [...], ...}` metadata surfaces in the menu bar based on the declared path, regardless of how many entity targets emit it. Menu-bar dedupe key: `(id, menu.path)` — ignore `target`. The entry's `target` for dispatch is the *innermost* entity moniker (most specific).

For the context menu, the existing `(id, target)` dedup stays. Both menus read the same `ResolvedCommand` list; the menu renderer in `kanban-app` (or the part of `scope_commands` that produces menu-bar output) needs a secondary dedupe pass keyed on `menu.path`.

### Files to touch

- `swissarmyhammer-kanban/src/scope_commands.rs` — either filter output for menu-bar callers, or add a helper `dedupe_for_menu_bar(commands: &mut Vec<ResolvedCommand>)` that keeps the first entry per `(id, menu.path)` and uses its target.
- Frontend menu bar renderer (audit `kanban-app/src/menu.rs` or similar Tauri menu-building code) — confirm it uses the right list.

### Subtasks

- [ ] Identify whether menu-bar emission and context-menu emission use the same call to `commands_for_scope` or different ones.
- [ ] Add menu-bar dedupe — either filter at call site or as a helper.
- [ ] Confirm the innermost-target chosen is the right one for dispatch (should be the same target the user's selection would use).

## Acceptance Criteria

- [ ] Edit menu shows "Cut", "Copy", "Paste" exactly once each, never duplicated per entity in scope.
- [ ] Clicking an Edit menu item dispatches to the innermost entity in the current scope (matches the user's selection).
- [ ] Right-click context menu is unchanged — still shows per-target Cut/Copy/Paste for nested entities.

## Tests

- [ ] Add `menu_bar_dedupes_cross_cutting_commands` in `scope_commands.rs` tests: scope `["tag:01T", "task:01X", "column:todo"]`; menu-bar output contains `entity.copy` exactly once; context-menu output contains it three times (one per target).
- [ ] Add `menu_bar_entry_targets_innermost` — assert the menu-bar entry for `entity.copy` has `target: "tag:01T"` (innermost).
- [ ] Run command: `cargo nextest run -p swissarmyhammer-kanban scope_commands menu` — all green.

## Workflow

- Use `/tdd` — write `menu_bar_dedupes_cross_cutting_commands` first; it fails until the dedupe is added.

#commands

Depends on: 01KPEMYJV7BMTJB6GZ8MGTD04J (mechanism)