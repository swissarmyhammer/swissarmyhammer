---
assignees:
- claude-code
depends_on:
- 01KRRN69YDB2B03RB1N9G6RR3J
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffffffe80
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
- [x] Add the `menu: { path: [View], group: 0, order: 0 }` block to the `ai.toggle` command definition in its builtin YAML file.
- [x] In `menu.rs` `build_menu_from_commands`, build the `View` submenu via `build_grouped_submenu` and insert it into the `Menu::with_items` array between Edit and Navigation; update the function doc comment.
- [x] Add the `collect_menu_entries` unit test for the `View` group.

## Acceptance Criteria
- [x] `build_menu_from_commands` constructs a top-level "View" submenu and includes it in the menu bar between Edit and Navigation.
- [x] The `ai.toggle` command carries a `menu: { path: [View], ... }` block; `collect_menu_entries` collects it under the `"View"` key.
- [x] The View menu item dispatches `ai.toggle` through the existing `menu-command` → `executeCommand` path (no new event plumbing).
- [x] `cargo build -p kanban-app` is clean.

## Tests
- [x] Unit test in `apps/kanban-app/src/menu.rs` `#[cfg(test)] mod tests`, modeled on the existing `navigation_submenu_contains_all_nine_nav_commands` test: compose the registry (`compose_registry![swissarmyhammer_commands, swissarmyhammer_focus, swissarmyhammer_kanban]`), call `collect_menu_entries(&registry, &UIState::new())`, assert `menus.get("View")` exists and contains an entry with id `ai.toggle`.
- [x] `cargo test -p kanban-app menu` is green.
- [x] `cargo build -p kanban-app` is clean.

## Workflow
- Use `/tdd` — write the `collect_menu_entries` "View" test first (it fails until the YAML `menu:` block is added), then add the YAML block and the `menu.rs` submenu wiring.

## Implementation Notes
- **YAML** — `crates/swissarmyhammer-kanban/builtin/commands/ai.yaml`: added a `menu:` block to the `ai.toggle` command — `path: [View]`, `group: 0`, `order: 0`. No other command in `ai.yaml` changed. `ai.toggle` was located where the dependency task placed it (kanban crate, not the commands crate).
- **Rust** — `apps/kanban-app/src/menu.rs` `build_menu_from_commands`: built `view_menu` via the existing generic helper `build_grouped_submenu(app, "View", menus.get("View"), &mut menu_items)?` and inserted `&view_menu` into the `Menu::with_items` array between `&edit_menu` and `&nav_menu` (final order: App, File, Edit, View, Navigation, Window). Updated the `build_menu_from_commands` doc comment to enumerate the six submenus; refreshed the inline comments around View and Navigation.
- **Test** — `apps/kanban-app/src/menu.rs` `#[cfg(test)] mod tests`: added `view_submenu_contains_ai_toggle_command`, modeled on `navigation_submenu_contains_all_nine_nav_commands`. Composes `compose_registry![swissarmyhammer_commands, swissarmyhammer_focus, swissarmyhammer_kanban]`, calls `collect_menu_entries`, asserts `menus.get("View")` exists and contains an `ai.toggle` entry. Followed `/tdd`: the test was written first and observed to fail (no `View` key) before the YAML/menu wiring was added.
- **No collector or frontend change** — `collect_menu_entries` already groups any `menu`-carrying command by `path.join("/")`, and `handle_menu_event` already emits `menu-command` → `executeCommand` for arbitrary ids. Confirmed: no event-plumbing change needed.
- **No snapshot regeneration** — adding a `menu:` block does not change command counts or ids; `builtin_commands.rs` and `composed_commands_registry.rs` only assert id presence in lists. Both test files were re-run and pass unchanged (9 passed). The entity `*_full.json` snapshots matched `ai.toggle` only as unrelated substring data, not command definitions.
- **Verification** (actual output): `cargo build -p kanban-app` — Finished, clean. `cargo test -p kanban-app --bin kanban-app menu` — 14 passed, 0 failed (incl. new `view_submenu_contains_ai_toggle_command`). `cargo clippy -p kanban-app --bins --tests -- -D warnings` — Finished, clean. `cargo test -p swissarmyhammer-kanban --test builtin_commands --test composed_commands_registry` — 9 passed, 0 failed.