---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffbd80
title: nav-jump shipped but not working in running app — vim s / cmd+G silent, no Jump To in palette, no Navigation menu
---
## Symptom (user report)

Three observable failures in the running app despite all unit/integration tests passing:

1. Pressing `s` in vim mode does nothing — no overlay opens.
2. Pressing `Cmd+G` in cua mode does nothing — no overlay opens.
3. Command palette (`Mod+Shift+P`) typing "jump" returns no `Jump To` entry.
4. Native menu bar has no `Navigation` submenu (the eight nav.* + nav.jump commands).

(2) (3) (4) are all driven by the registry. (1) is driven by `keybindings.ts` directly but ultimately dispatches to a `nav.jump` CommandDef.

## Hypotheses

A. **Stale Tauri binary** — pnpm tauri dev hot-reloaded the frontend but the Rust binary wasn't rebuilt, so the running registry is from a pre-nav.yaml world. Symptoms (3) and (4) would point this way; (1) and (2) would also fail because the dispatched `nav.jump` id has no execute closure registered before the `globalCommands` memo is hit.
B. **Frontend dispatch path doesn't see globalCommands `nav.jump`** — the React-side CommandDef in `app-shell.tsx::globalCommands` is included but the dispatcher only looks at YAML-loaded entries.
C. **Menu builder defect** — `build_menu_from_commands` builds the Navigation submenu but it's not being inserted into the final menu chain in the running app for some reason (mis-ordering, a code path that bypasses it).
D. **`compose_registry!` order or behavior bug** — focus crate is in the macro list but its YAML isn't actually flowing in.

## First diagnostic steps

1. Restart `pnpm tauri dev` to force a full rebuild and re-test all four symptoms.
2. If symptoms persist after a clean rebuild:
   - Check `cargo build -p kanban-app` reaches the new code (look for the `Navigation` literal in `kanban-app/src/menu.rs`).
   - Add a `tracing::info!` inside `build_menu_from_commands` logging the `menus` HashMap keys at startup. Confirm "Navigation" appears.
   - Add a `tracing::info!` in `state.rs::with_ui_state` logging the registry's `all_commands().count()` after `compose_registry!` runs. Should be 69.
3. If the registry has fewer than 69 commands, instrument `swissarmyhammer_focus::builtin_yaml_sources()` to log how many `(name, content)` tuples it returns. Should be at least one (`nav.yaml`).

## Acceptance Criteria

- [ ] `s` in vim mode opens the Jump-To overlay in the running app.
- [ ] `Cmd+G` in cua mode opens it.
- [ ] `Jump To` appears in the command palette and selecting it opens the overlay.
- [ ] `Navigation > Jump To` appears in the native menu bar and selecting it opens the overlay.
- [ ] All other nav.* commands (`nav.up`/`down`/`left`/`right`/`first`/`last`/`drillIn`/`drillOut`) appear under the same Navigation menu.

## Tags

#bug #nav-jump