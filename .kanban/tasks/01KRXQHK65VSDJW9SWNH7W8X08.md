---
assignees:
- claude-code
depends_on:
- 01KRRN69YDB2B03RB1N9G6RR3J
position_column: todo
position_ordinal: '9580'
project: ai-panel
title: View menu — native menu-bar "View" submenu, AI panel toggle wired in via YAML
---
## What
Add a top-level **View** menu to the app's native menu bar, and surface the AI panel toggle in it — driven entirely by YAML `menu:` declarations (the documented four-surface command model in `ARCHITECTURE.md`: a command's YAML controls which of native-menu-bar / context-menu / palette / button surfaces it appears on).

Current state (researched):
- `apps/kanban-app/src/menu.rs` `build_menu_from_commands` builds exactly five top-level submenus — App, File, Edit, Navigation, Window — via `Menu::with_items(app, &[&app_menu, &file_menu, &edit_menu, &nav_menu, &window_menu])`. There is **no** View submenu.
- `collect_menu_entries` already groups *any* `CommandDef` carrying a `menu` placement by `menu.path.join("/")` — generically. A command with `menu: { path: [View], ... }` is auto-collected under the `"View"` key with **no collector change**.
- The generic `build_grouped_submenu(app, label, menus.get(key), &mut menu_items)` helper already builds Edit and Navigation; it builds View identically.
- `handle_menu_event` emits a `menu-command` event for any non-special menu id, which the webview routes through `executeCommand(id)`; `update_menu_enabled_state` already maintains menu-item enabled state via `commands_for_scope`. So **no frontend or event-plumbing change is needed** — wiring the submenu + the YAML `menu:` block is sufficient.
- The AI panel toggle command `ai.toggle` is created by the dependency task `01KRRN69YDB2B03RB1N9G6RR3J` ("AI panel command scope and keybindings"). This task adds its `menu:` placement.

Work:
1. **YAML** — give `ai.toggle` a `menu:` block in the builtin YAML file that defines it (locate it; the dependency task places it under `swissarmyhammer-commands` or `swissarmyhammer-kanban` builtin commands):
   ```yaml
   menu:
     path: [View]
     group: 0
     order: 0
   ```
2. **Rust** — in `apps/kanban-app/src/menu.rs` `build_menu_from_commands`, build a `View` submenu with the existing helper — `let view_menu = build_grouped_submenu(app, "View", menus.get("View"), &mut menu_items)?;` — and insert `&view_menu` into the `Menu::with_items` array in conventional position: after Edit, before Navigation (Edit → View → Navigation → Window). Update the doc comment on `build_menu_from_commands` that enumerates the submenus.
3. Confirm no frontend change is required (the `menu-command` → `executeCommand` path already handles arbitrary command ids).

Scope note: this task establishes the View menu and places the AI panel toggle in it. Other view-related commands (inspector toggles, etc.) can later gain a `menu: { path: [View] }` block — out of scope here; one concern per task.

## Subtasks
- [ ] Add the `menu: { path: [View], group: 0, order: 0 }` block to the `ai.toggle` command definition in its builtin YAML file.
- [ ] In `menu.rs` `build_menu_from_commands`, build the `View` submenu via `build_grouped_submenu` and insert it into the `Menu::with_items` array between Edit and Navigation; update the function doc comment.
- [ ] Add the `collect_menu_entries` unit test for the `View` group.

## Acceptance Criteria
- [ ] `build_menu_from_commands` constructs a top-level "View" submenu and includes it in the menu bar between Edit and Navigation.
- [ ] The `ai.toggle` command carries a `menu: { path: [View], ... }` block; `collect_menu_entries` collects it under the `"View"` key.
- [ ] The View menu item dispatches `ai.toggle` through the existing `menu-command` → `executeCommand` path (no new event plumbing).
- [ ] `cargo build -p kanban-app` is clean.

## Tests
- [ ] Unit test in `apps/kanban-app/src/menu.rs` `#[cfg(test)] mod tests`, modeled on the existing `navigation_submenu_contains_all_nine_nav_commands` test: compose the registry (`compose_registry![swissarmyhammer_commands, swissarmyhammer_focus, swissarmyhammer_kanban]`), call `collect_menu_entries(&registry, &UIState::new())`, assert `menus.get("View")` exists and contains an entry with id `ai.toggle`.
- [ ] `cargo test -p kanban-app menu` is green.
- [ ] `cargo build -p kanban-app` is clean.

## Workflow
- Use `/tdd` — write the `collect_menu_entries` "View" test first (it fails until the YAML `menu:` block is added), then add the YAML block and the `menu.rs` submenu wiring.