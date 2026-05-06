---
assignees:
- claude-code
depends_on:
- 01KQZ2QA21J6ABZ1P1YS1AF209
position_column: todo
position_ordinal: de80
project: keyboard-navigation
title: Land nav.* commands as a YAML contribution from swissarmyhammer-focus
---
## What

Move the universal navigation commands (`nav.up`, `nav.down`, `nav.left`, `nav.right`, `nav.first`, `nav.last`, `nav.drillIn`, `nav.drillOut`) from their frontend-only location in `kanban-app/ui/src/components/app-shell.tsx` (`NAV_COMMAND_SPEC`, lines ~28-70) into a YAML stub owned by the **`swissarmyhammer-focus`** crate. Then add a Navigation top-level menu in the native menu bar.

Rationale: `swissarmyhammer-focus`'s own description in `Cargo.toml:25` is *"Spatial focus and keyboard navigation engine — generic, no domain dependencies"*. The nav.* commands are the user-facing surface of that engine's kernel ops (`spatial_navigate`, `spatial_drill_in`, `spatial_drill_out`). Domain-free, kernel-coupled — they belong with the focus crate, not with the generic `swissarmyhammer-commands` crate.

This task depends on the linkme-based aggregator (prior task) so the focus crate can register itself without editing any other crate.

### Steps

1. **Add `include_dir` and `linkme` deps to `swissarmyhammer-focus/Cargo.toml`.** The crate description promises "no domain dependencies"; `include_dir` and `linkme` are infrastructure crates, not domain — adding them is fine. Update the description if needed to keep it accurate.

2. **Create `swissarmyhammer-focus/builtin/commands/nav.yaml`** with one entry per command:
   - `id`: same id (`nav.up`, etc.)
   - `name`: human label (`Navigate Up`, `Navigate Down`, `Navigate Left`, `Navigate Right`, `Navigate to First`, `Navigate to Last`, `Drill In`, `Drill Out`)
   - `keys`: copy current bindings — vim/cua/emacs blocks per `NAV_COMMAND_SPEC`. Keep `nav.drillIn` mapped to `Enter` and `nav.drillOut` mapped to `Escape` (currently in `kanban-app/ui/src/lib/keybindings.ts:104-114`).
   - `menu`: `{ path: [Navigation], group: 0|1|2, order: 0..N }` — group 0 for up/down/left/right, group 1 for first/last, group 2 for drillIn/drillOut.
   - `undoable: false` for all of them.

3. **Wire the focus crate into the aggregator** by adding to `swissarmyhammer-focus/src/lib.rs` (mirroring the kanban pattern):

   ```rust
   static BUILTIN_COMMANDS: include_dir::Dir =
       include_dir::include_dir!("$CARGO_MANIFEST_DIR/builtin/commands");

   pub fn builtin_yaml_sources() -> Vec<(&'static str, &'static str)> {
       BUILTIN_COMMANDS
           .files()
           .filter(|f| f.path().extension().and_then(|e| e.to_str()) == Some("yaml"))
           .filter(|f| f.path().parent() == Some(std::path::Path::new("")))
           .filter_map(|f| {
               let name = f.path().file_stem()?.to_str()?;
               let content = f.contents_utf8()?;
               Some((name, content))
           })
           .collect()
   }

   #[linkme::distributed_slice(swissarmyhammer_commands::BUILTIN_COMMANDS_CONTRIBUTIONS)]
   static FOCUS_CONTRIBUTION: swissarmyhammer_commands::BuiltinCommandsContribution =
       swissarmyhammer_commands::BuiltinCommandsContribution {
           crate_name: "swissarmyhammer-focus",
           priority: 100,  // between generic commands (0) and domain crates (200)
           sources: builtin_yaml_sources,
       };
   ```

   Note: this introduces a runtime dep from `swissarmyhammer-focus` to `swissarmyhammer-commands`. That's fine — it's an infrastructure dep, not a domain one. The generic-commands crate doesn't import focus, so no cycle.

4. **Update `kanban-app/src/menu.rs:148-170`** (`build_menu_from_commands`) to construct a `Navigation` submenu via `build_grouped_submenu(app, "Navigation", menus.get("Navigation"), &mut menu_items)?` and append it between Edit and Window: `Menu::with_items(app, &[&app_menu, &file_menu, &edit_menu, &nav_menu, &window_menu])`.

5. **Keep the React-side `execute` closures in `app-shell.tsx`'s `buildNavCommands` exactly as they are** — they read live `SpatialFocusActions` from a ref. The YAML stubs are pure metadata (id, name, keys, menu); the closure-bearing CommandDefs in app-shell remain the source of truth for execution. The frontend `useDispatchCommand` already merges YAML-defined and React-defined commands by id.

6. **Verify menu clicks for Navigation entries dispatch the React execute closure.** Trace the existing menu-click path for an existing frontend-resolved command (e.g., `app.about`) and ensure nav.* uses the same path.

## Acceptance Criteria

- [ ] `swissarmyhammer-focus/builtin/commands/nav.yaml` exists with all 8 nav.* entries.
- [ ] `swissarmyhammer-focus` exports `builtin_yaml_sources()` and registers a contribution at priority 100.
- [ ] `swissarmyhammer-focus` is in the binary's dep graph (kanban-app already depends on it transitively via state.rs); the slice element is reachable at runtime.
- [ ] All 8 nav commands appear under a `Navigation` submenu in the native menu bar with their per-mode accelerators rendered.
- [ ] All 8 nav commands appear in the command palette (`Mod+Shift+P`) with their `name` labels and key hints.
- [ ] Pressing a keybinding (e.g., `j` in vim) navigates as before — no behavior regression.
- [ ] No edits required in `swissarmyhammer-commands` or `swissarmyhammer-kanban` to make the focus crate's commands visible — they appear via the linkme aggregator.

## Tests

- [ ] New Rust unit test in `swissarmyhammer-focus/tests/`: `nav_yaml_registers_all_eight_commands` — call `swissarmyhammer_commands::all_builtin_yaml_sources()` (after the aggregator task lands) or directly call `swissarmyhammer_focus::builtin_yaml_sources()`, parse the resulting YAML, assert each id has the expected `keys` and `menu.path == ["Navigation"]`.
- [ ] New integration test in `kanban-app/tests/` (or extend an existing menu builder test): build the menu, assert a `Navigation` submenu exists with 8 grouped items.
- [ ] Frontend test `kanban-app/ui/src/components/app-shell.nav-commands.test.tsx`: mount AppShell, assert `globalCommands` includes all 8 nav ids and each has a non-null `execute`.
- [ ] Existing spatial-nav tests (`board-view.spatial-nav.test.tsx`, `board-view.cross-column-nav.spatial.test.tsx`, `nav-bar.spatial-nav.test.tsx`) still pass.
- [ ] Test command: `cargo nextest run -p swissarmyhammer-focus -p swissarmyhammer-commands && cd kanban-app/ui && pnpm test app-shell.nav-commands` — all green.

## Workflow

- Use `/tdd` — write the focus-crate registry test first; watch it fail; create `nav.yaml` and the slice declaration; re-run.