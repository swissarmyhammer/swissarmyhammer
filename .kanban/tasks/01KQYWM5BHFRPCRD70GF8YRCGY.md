---
assignees:
- claude-code
depends_on:
- 01KQZ2QA21J6ABZ1P1YS1AF209
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffb980
project: spatial-nav
title: Land nav.* commands as a YAML contribution from swissarmyhammer-focus
---
## What

Move the universal navigation commands from their frontend-only location in `kanban-app/ui/src/components/app-shell.tsx` into a YAML stub owned by the **`swissarmyhammer-focus`** crate, then add a Navigation top-level menu in the native menu bar.

### Current frontend layout (post-cutover)

The 8 nav.* commands are split across two builders in `app-shell.tsx`:

- `NAV_COMMAND_SPEC` (find via `grep "NAV_COMMAND_SPEC"`) — 6 directional + first/last entries: `nav.up`, `nav.down`, `nav.left`, `nav.right`, `nav.first`, `nav.last`. Built into CommandDefs by `buildNavCommands`, which now uses `runNavWithScrollOnEdge` (from `@/lib/scroll-on-edge`) rather than calling `actions.navigate` directly.
- `buildDrillCommands` — `nav.drillIn` and `nav.drillOut`. Distinct because they thread the snapshot through `actions.drillIn(focusedFq, focusedFq)` / `actions.drillOut(focusedFq, focusedFq)` (per the snapshot-cutover commits `58fa22ee6`, `0c0ba30e2`).

Both code paths stay — this task only moves *metadata* (id, name, keys, menu placement) into YAML. The execute closures remain in app-shell because they need live access to `SpatialFocusActions` via a ref.

### Rationale

`swissarmyhammer-focus`'s description (`Cargo.toml`) is *"Spatial focus and keyboard navigation engine — generic, no domain dependencies"*. The nav.* commands are the user-facing surface of that engine's kernel ops (`spatial_navigate`, `spatial_drill_in`, `spatial_drill_out`). Domain-free, kernel-coupled — they belong with the focus crate.

This task depends on the macro-based aggregator (prior task) so the focus crate just exposes `builtin_yaml_sources()` like every other contributor and the app's `compose_registry![]` invocation lists it.

### Steps

1. **Add `include_dir` to `swissarmyhammer-focus/Cargo.toml`.** No `swissarmyhammer-commands` dep needed — pure data shipping.

2. **Create `swissarmyhammer-focus/builtin/commands/nav.yaml`** with one entry per command (8 entries):
   - `id`: same id (`nav.up`, `nav.down`, `nav.left`, `nav.right`, `nav.first`, `nav.last`, `nav.drillIn`, `nav.drillOut`)
   - `name`: human label (`Navigate Up`, `Navigate Down`, `Navigate Left`, `Navigate Right`, `Navigate to First`, `Navigate to Last`, `Drill In`, `Drill Out`)
   - `keys`: copy current bindings from `NAV_COMMAND_SPEC` (vim/cua/emacs blocks) and from `BINDING_TABLES` in `kanban-app/ui/src/lib/keybindings.ts` for drillIn/drillOut. The drill commands map to `Enter` / `Escape` in all three modes.
   - `menu`: `{ path: [Navigation], group: 0|1|2, order: 0..N }` — group 0 for up/down/left/right, group 1 for first/last, group 2 for drillIn/drillOut.
   - `undoable: false` for all of them.

3. **Add `builtin_yaml_sources()` to `swissarmyhammer-focus/src/lib.rs`** (mirroring the kanban pattern):

   ```rust
   static BUILTIN_COMMANDS: include_dir::Dir =
       include_dir::include_dir!("$CARGO_MANIFEST_DIR/builtin/commands");

   /// Builtin command YAML sources contributed by the focus crate.
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

   Order matters for partial-merge precedence: focus sits between generic UI commands and the domain-specific kanban commands so the kanban crate can still override a focus default if needed.

5. **Update `kanban-app/src/menu.rs`** — find `build_menu_from_commands` (current top-of-file function). Construct a `Navigation` submenu via `build_grouped_submenu(app, "Navigation", menus.get("Navigation"), &mut menu_items)?` and append it between Edit and Window: `Menu::with_items(app, &[&app_menu, &file_menu, &edit_menu, &nav_menu, &window_menu])`.

6. **Keep the React-side `execute` closures in `app-shell.tsx`'s `buildNavCommands` and `buildDrillCommands` exactly as they are** — they read live `SpatialFocusActions` from a ref. The YAML stubs are pure metadata (id, name, keys, menu); the closure-bearing CommandDefs in app-shell remain the source of truth for execution. The frontend's command merging logic combines YAML-defined and React-defined commands by id.

7. **Verify menu clicks for Navigation entries dispatch the React execute closure.** Trace the existing menu-click path for an existing frontend-resolved command (e.g., `app.about`) and ensure nav.* uses the same path.

## Acceptance Criteria

- [x] `swissarmyhammer-focus/builtin/commands/nav.yaml` exists with all 8 nav.* entries.
- [x] `swissarmyhammer-focus` exports `builtin_yaml_sources()`.
- [x] `kanban-app/src/state.rs` includes `swissarmyhammer_focus` in its `compose_registry![]` invocation.
- [x] All 8 nav commands appear under a `Navigation` submenu in the native menu bar with their per-mode accelerators rendered.
- [x] All 8 nav commands appear in the command palette (`Mod+Shift+P`) with their `name` labels and key hints.
- [x] Pressing a keybinding (e.g., `j` in vim) navigates as before — no behavior regression. (Existing 131 spatial-nav tests still pass.)

## Tests

- [x] New Rust unit test in `swissarmyhammer-focus/tests/`: `nav_yaml_registers_all_eight_commands` — call `swissarmyhammer_focus::builtin_yaml_sources()`, parse each YAML file, assert the union contains all 8 ids with the expected `keys` per mode and `menu.path == ["Navigation"]`.
- [x] New integration test in `kanban-app/tests/` (or extend an existing menu builder test): build the menu, assert a `Navigation` submenu exists with 8 grouped items. (Added `kanban-app/src/menu.rs::tests::navigation_submenu_contains_all_eight_nav_commands` — the menu builder's `Menu::with_items` step requires a Tauri AppHandle, so the test exercises `collect_menu_entries` directly, which is the pure-data input that `build_grouped_submenu(app, "Navigation", …)` consumes.)
- [x] Frontend test `kanban-app/ui/src/components/app-shell.nav-commands.test.tsx`: mount AppShell, assert `globalCommands` includes all 8 nav ids and each has a non-null `execute`.
- [x] Existing spatial-nav tests (`board-view.spatial-nav.test.tsx`, `board-view.cross-column-nav.spatial.test.tsx`, `nav-bar.spatial-nav.test.tsx`, plus the post-cutover `spatial-nav-end-to-end.spatial.test.tsx`) still pass.
- [x] Test command: `cargo nextest run -p swissarmyhammer-focus && cd kanban-app/ui && pnpm test app-shell.nav-commands` — all green.

## Workflow

- Use `/tdd` — write the focus-crate `nav_yaml_registers_all_eight_commands` test first; watch it fail; create `nav.yaml` and `builtin_yaml_sources()`; wire focus into the app's `compose_registry![]`; re-run. #nav-jump

## Implementation Notes

- The pre-existing snapshot tests `swissarmyhammer-kanban/tests/composed_commands_registry.rs` and `swissarmyhammer-kanban/tests/builtin_commands.rs::composed_builtins_register_all_sixty_commands` were updated in lockstep — the composed registry now contains 68 ids (60 → 68 from the 8 nav.* additions), the snapshot list was extended to include the 8 nav.* ids in alphabetical order, and the latter test was renamed to `composed_builtins_register_all_sixty_eight_commands`. Both compose_registry! and compose_yaml_sources! call sites in `kanban-app/src/state.rs` were updated to include `swissarmyhammer_focus` between commands and kanban.
- `swissarmyhammer-focus/Cargo.toml` gained `include_dir` as a dependency and `swissarmyhammer-commands` + `serde_yaml_ng` as dev-dependencies (needed by the test that parses the YAML through the canonical `CommandDef` type).
- `swissarmyhammer-kanban/Cargo.toml` gained `swissarmyhammer-focus` as a dev-dependency so the composed-registry tests can compose all three contributors.