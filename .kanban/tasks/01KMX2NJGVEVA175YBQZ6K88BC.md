---
assignees:
- claude-code
depends_on:
- 01KMX2N13T0WEDMK9V93DCJ7VH
position_column: done
position_ordinal: ffffffffffffffec80
title: Build native menu from command registry
---
## What

Replace the frontend-driven menu manifest with Rust-side menu building directly from the command registry. The menu is just another dispatch surface for commands.

### Files to modify
- `kanban-app/src/menu.rs` — new `build_menu_from_commands()` that reads CommandsRegistry, filters to commands with `menu` set, groups by path, builds native MenuItem items
- `kanban-app/src/commands.rs` — remove `rebuild_menu_from_manifest` Tauri command, add `build_menu_from_commands` call at startup and on keymap change
- `kanban-app/src/state.rs` — add `menu_items: Mutex<HashMap<String, MenuItem>>` to AppState for enable/disable, remove `last_menu_manifest`
- `kanban-app/src/main.rs` — call menu build after app setup
- `kanban-app/ui/src/lib/menu-sync.ts` — delete
- `kanban-app/ui/src/components/app-shell.tsx` — remove syncMenuToNative calls and menuPlacement from global commands

### How it works
1. Read all `CommandDef` from `CommandsRegistry` that have `menu` set
2. Group by `menu.path[0]` (top-level menu), sort by `(group, order)`
3. For nested paths like `[File, Export]`, create submenus
4. Build `MenuItem::with_id` for each, resolve accelerator from `keys` + current keymap mode
5. Store all MenuItem handles in `AppState.menu_items` HashMap keyed by command ID
6. Inject OS chrome (About, Quit, Hide, etc.) in their standard positions

### When to rebuild
- App startup
- Keymap mode change (accelerators change)
- Board switch (command registry may have overrides)

## Acceptance Criteria
- [ ] Menu bar built entirely from YAML command definitions
- [ ] No frontend menu manifest or syncMenuToNative
- [ ] All menu items dispatch through command system via handle_menu_event
- [ ] Accelerators reflect current keymap mode
- [ ] MenuItem handles cached in AppState for enable/disable
- [ ] `cargo check -p kanban-app` compiles

## Tests
- [ ] `cargo nextest run -p kanban-app` passes
- [ ] Manual: all existing menu items still appear and work"
<parameter name="assignees">[]