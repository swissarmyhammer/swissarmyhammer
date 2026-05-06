---
assignees:
- claude-code
depends_on:
- 01KQZ2QA21J6ABZ1P1YS1AF209
position_column: todo
position_ordinal: de80
project: spatial-nav
title: Land nav.* commands as a YAML contribution from swissarmyhammer-focus
---
## What

Move the universal navigation commands (`nav.up`, `nav.down`, `nav.left`, `nav.right`, `nav.first`, `nav.last`, `nav.drillIn`, `nav.drillOut`) from their frontend-only location in `kanban-app/ui/src/components/app-shell.tsx` (`NAV_COMMAND_SPEC`, lines ~28-70) into a YAML stub owned by the **`swissarmyhammer-focus`** crate. Then add a Navigation top-level menu in the native menu bar.

Rationale: `swissarmyhammer-focus`'s own description in `Cargo.toml:25` is *"Spatial focus and keyboard navigation engine — generic, no domain dependencies"*. The nav.* commands are the user-facing surface of that engine's kernel ops (`spatial_navigate`, `spatial_drill_in`, `spatial_drill_out`). Domain-free, kernel-coupled — they belong with the focus crate, not with the generic `swissarmyhammer-commands` crate.

This task depends on the macro-based aggregator (prior task) so the focus crate just exposes `builtin_yaml_sources()` like every other contributor and the app's `compose_registry![]` invocation lists it.

### Steps

1. **Add `include_dir` to `swissarmyhammer-focus/Cargo.toml`.** Also add `swissarmyhammer-commands` as a dep — needed for the `CommandDef`-related types if the focus crate writes any unit tests that parse its own YAML; pure data shipping does not require it. The dep direction `focus → commands` is fine (no cycle, since commands does not import focus).

2. **Create `swissarmyhammer-focus/builtin/commands/nav.yaml`** with one entry per command:
   - `id`: same id (`nav.up`, etc.)
   - `name`: human label (`Navigate Up`, `Navigate Down`, `Navigate Left`, `Navigate Right`, `Navigate to First`, `Navigate to Last`, `Drill In`, `Drill Out`)
   - `keys`: copy current bindings — vim/cua/emacs blocks per `NAV_COMMAND_SPEC`. Keep `nav.drillIn` mapped to `Enter` and `nav.drillOut` mapped to `Escape` (currently in `kanban-app/ui/src/lib/keybindings.ts:104-114`).
   - `menu`: `{ path: [Navigation], group: 0|1|2, order: 0..N }` — group 0 for up/down/left/right, group 1 for first/last, group 2 for drillIn/drillOut.
   - `undoable: false` for all of them.

3. **Add `builtin_yaml_sources()` to `swissarmyhammer-focus/src/lib.rs`** (mirroring the kanban pattern):

   ```rust
   static BUILTIN_COMMANDS: include_dir::Dir =
       include_dir::include_dir!("$CARGO_MANIFEST_DIR/builtin/commands");

   /// Builtin command YAML sources contributed by the focus crate.
   ///
   /// Same shape every contributor crate exposes — the app's
   /// `compose_registry![]` macro slurps from this function.
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
   ```

4. **In `kanban-app/src/state.rs`**, add `swissarmyhammer_focus` to the `compose_registry![]` invocation set up by the prior task:

   ```rust
   let registry = swissarmyhammer_commands::compose_registry![
       swissarmyhammer_commands,
       swissarmyhammer_focus,    // ← added
       swissarmyhammer_kanban,
   ];
   ```

   Order matters for partial-merge precedence: focus sits between generic UI commands and the domain-specific kanban commands so the kanban crate can still override a focus default if it ever needed to.

5. **Update `kanban-app/src/menu.rs:148-170`** (`build_menu_from_commands`) to construct a `Navigation` submenu via `build_grouped_submenu(app, "Navigation", menus.get("Navigation"), &mut menu_items)?` and append it between Edit and Window: `Menu::with_items(app, &[&app_menu, &file_menu, &edit_menu, &nav_menu, &window_menu])`.

6. **Keep the React-side `execute` closures in `app-shell.tsx`'s `buildNavCommands` exactly as they are** — they read live `SpatialFocusActions` from a ref. The YAML stubs are pure metadata (id, name, keys, menu); the closure-bearing CommandDefs in app-shell remain the source of truth for execution. The frontend `useDispatchCommand` already merges YAML-defined and React-defined commands by id.

7. **Verify menu clicks for Navigation entries dispatch the React execute closure.** Trace the existing menu-click path for an existing frontend-resolved command (e.g., `app.about`) and ensure nav.* uses the same path.

## Acceptance Criteria

- [ ] `swissarmyhammer-focus/builtin/commands/nav.yaml` exists with all 8 nav.* entries.
- [ ] `swissarmyhammer-focus` exports `builtin_yaml_sources()`.
- [ ] `kanban-app/src/state.rs` includes `swissarmyhammer_focus` in its `compose_registry![]` invocation.
- [ ] All 8 nav commands appear under a `Navigation` submenu in the native menu bar with their per-mode accelerators rendered.
- [ ] All 8 nav commands appear in the command palette (`Mod+Shift+P`) with their `name` labels and key hints.
- [ ] Pressing a keybinding (e.g., `j` in vim) navigates as before — no behavior regression.

## Tests

- [ ] New Rust unit test in `swissarmyhammer-focus/tests/`: `nav_yaml_registers_all_eight_commands` — call `swissarmyhammer_focus::builtin_yaml_sources()`, parse each YAML file, assert the union contains all 8 ids with the expected `keys` per mode and `menu.path == ["Navigation"]`.
- [ ] New integration test in `kanban-app/tests/` (or extend an existing menu builder test): build the menu, assert a `Navigation` submenu exists with 8 grouped items.
- [ ] Frontend test `kanban-app/ui/src/components/app-shell.nav-commands.test.tsx`: mount AppShell, assert `globalCommands` includes all 8 nav ids and each has a non-null `execute`.
- [ ] Existing spatial-nav tests (`board-view.spatial-nav.test.tsx`, `board-view.cross-column-nav.spatial.test.tsx`, `nav-bar.spatial-nav.test.tsx`) still pass.
- [ ] Test command: `cargo nextest run -p swissarmyhammer-focus && cd kanban-app/ui && pnpm test app-shell.nav-commands` — all green.

## Workflow

- Use `/tdd` — write the focus-crate `nav_yaml_registers_all_eight_commands` test first; watch it fail; create `nav.yaml` and `builtin_yaml_sources()`; wire focus into the app's `compose_registry![]`; re-run.