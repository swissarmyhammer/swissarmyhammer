---
assignees:
- claude-code
depends_on:
- 01KTED5F8DQ2XH5BB0WK1MRR3P
- 01KTEEDA9ZVTZ2R5CERW0WGK97
- 01KTESYQ49JYJB2YT1WXYKK0W4
position_column: review
position_ordinal: '8580'
project: ui-command-cleanup
title: Card A — Register nav.* + palette opener as plugin commands with OS-menu placement
---
## What
This is the owner's "start" card. It SUBSUMES the prior "Navigation menu does not list the motion commands" investigation (this card was rewritten in place) and covers the OS-menu-placement half of the palette bug (01KTCRQ6KJ67FJWYEZFQ6J7R13).

Register the nine `nav.*` commands and reconcile the palette opener as PLUGIN commands carrying `menu` placement + `keys` + availability, so the OS menu is built FROM the CommandService catalogue. The UI stays presentation (renders the menu, opens the jump overlay, dispatches ids).

## Naming (rename fold — owner decision)
Per project decision, commands MOVED by this cleanup adopt their final `app.*` name AT MOVE TIME (folding rename card 01KTEBZSVGAZ881RAZZWWZXGPE). `nav.*` is NOT a `ui.*` command, so it stays `nav.*`. The palette opener IS `ui.*` → register it as `app.palette.open` here. The standalone rename card only mops up `ui.*` ids this project does not move.

### Today's (rejected) state to retire
- `crates/swissarmyhammer-focus/builtin/commands/nav.yaml` defines all nine `nav.*` ids WITH menu placement, but its own header says "Execution lives in React closures in app-shell.tsx (buildNavCommands, buildDrillCommands)" and "the frontend's command merging logic combines YAML-defined and React-defined commands by id." That YAML-merge/overlay is the rejected approach. Migrate these ids into a real plugin and stop merging YAML into the service snapshot.
- Frontend trash in `apps/kanban-app/ui/src/components/app-shell.tsx`: `NAV_COMMAND_SPEC` (line ~263) + `buildNavCommands` (~324), `buildDrillCommands` (~380), and the inline `nav.jump` def (~798) whose execute is `setJumpOpen(true)`.

### Register as plugin commands (new plugin, e.g. `builtin/plugins/nav-commands/index.ts`, mirroring `builtin/plugins/file-commands/index.ts`)
- `nav.up/down/left/right/first/last` → backend op `spatial_navigate(window, fq, direction)` in `crates/swissarmyhammer-focus` (operations.rs/server.rs). Carry `keys` + `menu:{path:["Navigation"],group,order}` exactly as nav.yaml has them.
- `nav.drillIn` / `nav.drillOut` → backend `spatial_drill_in` / `spatial_drill_out`.
- `nav.focus` → backend `spatial_focus` (the def belongs here; the dedup of its two frontend definitions is Card G).
- `nav.jump` → handler bus (Card B): plugin owns id/name/keys(`vim:s`,`cua/emacs:Mod+G`)/menu; the webview registers a handler that opens `<JumpToOverlay>` (`setJumpOpen(true)`). Keep `<JumpToOverlay>` and its dismiss path as presentation.

### Palette reconciliation (OS-menu half)
- Register the palette opener as `app.palette.open` (folding the ui.*→app.* rename) with `menu: { path: ["App"], group, order }`, replacing the current `ui.palette.open` registration in `builtin/plugins/ui-commands/index.ts` (it currently has `keys:{cua:"Mod+K",vim:":"}` but NO `menu` — confirmed; that is why the palette isn't on the OS menu). Routing to the `ui_state` server is unchanged; only the id + menu placement change.
- Reconcile the `app.command` / `app.palette` frontend id split: point `apps/kanban-app/ui/src/lib/keybindings.ts` at `app.palette.open` and delete the STATIC palette entries that duplicate it. (The execution/hotkey-firing failure stays scoped to 01KTCRQ6KJ67FJWYEZFQ6J7R13; THIS card only guarantees the menu affordance + that the catalogue carries the palette command with menu placement.)

### Keep (presentation — do NOT move)
`apps/kanban-app/src/menu.rs` (OS-menu builder, fed by the service), the jump overlay, the keybinding dispatch handler. Do NOT overlay nav.yaml onto the service snapshot.

## Acceptance Criteria
- [ ] The nine `nav.*` commands are defined by a PLUGIN (not nav.yaml-merged, not React closures); each carries keys + Navigation `menu` placement; directional/first/last route to `spatial_navigate`, drill to `spatial_drill_in/out`, focus to `spatial_focus`, jump to the handler bus.
- [ ] The palette opener is registered as `app.palette.open` with an App-menu placement (in the app/ui-commands plugin), routing to `ui_state` unchanged; no `ui.palette.open` id remains.
- [ ] `NAV_COMMAND_SPEC`, `buildNavCommands`, `buildDrillCommands`, and the inline `nav.jump` def are deleted from app-shell.tsx; the jump overlay still opens via the bus.
- [ ] `crates/swissarmyhammer-focus/builtin/commands/nav.yaml` is retired (or reduced to non-overlay use) so the nav metadata reaches the OS menu THROUGH the service catalogue, not a YAML merge.
- [ ] The native Navigation submenu renders all nine nav items AND the palette has an App-menu affordance, both built from the service catalogue at runtime.

## Tests
- [ ] Rust menu test in `apps/kanban-app/src/menu.rs`: build the menu from the SAME catalogue the app composes at runtime (the production composition path, NOT a hand-built `compose_registry!`); assert "Navigation" has 9 entries and `app.palette.open` collects into the App submenu. Regression test that fails before and passes after.
- [ ] Plugin e2e (mirror `crates/swissarmyhammer-command-service/tests/integration/builtin_ui_commands_e2e.rs`): the nav plugin registers nine nav.* ids with the expected menu placements and backend-op routing; `app.palette.open` exposes a menu placement and routes to ui_state.
- [ ] UI test: dispatching `nav.jump` opens `<JumpToOverlay>` via the handler bus (extend `apps/kanban-app/ui/src/components/jump-to-overlay.browser.test.tsx`); dispatching `nav.up`/`nav.drillIn` invokes the spatial backend op (extend an existing `*.spatial.test.tsx`).
- [ ] `cargo test -p kanban-app menu` and the relevant vitest files are green.

## Workflow
- Use `/tdd` — failing tests first, then implement. Automated tests only. #bug