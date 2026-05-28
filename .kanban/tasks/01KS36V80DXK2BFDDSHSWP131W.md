---
assignees:
- claude-code
depends_on:
- 01KS36QGEVVP064EKW0JDGD94B
- 01KS3BK37R2P0RYSYSXGZTS0Z3
- 01KS36W7VTKXXS4Z1C0P4SHZDT
- 01KS5E9M7ZNPNA0E7GR1C9N42R
- 01KS5F7BR6850RKT67X4CNHPAZ
- 01KS61511W6EGZ88043S261RSH
position_column: review
position_ordinal: '80'
project: builtin-commands
title: 'Builtin plugin: app + settings + drag commands (port app.yaml + settings.yaml + drag.yaml)'
---
## What

Port three small platform-shell YAML files into one builtin TypeScript plugin (15 commands total, all host-shell concerns).

Source YAMLs:
- `crates/swissarmyhammer-commands/builtin/commands/app.yaml` — 9 commands: `app.about`, `app.help`, `app.quit`, `app.command`, `app.palette`, `app.search`, `app.dismiss`, `app.undo`, `app.redo`
- `crates/swissarmyhammer-commands/builtin/commands/settings.yaml` — 3 commands: `settings.keymap.vim`, `settings.keymap.cua`, `settings.keymap.emacs`
- `crates/swissarmyhammer-commands/builtin/commands/drag.yaml` — 3 commands: `drag.start`, `drag.cancel`, `drag.complete`

Total: 15 commands.

Files:
- `builtin/plugins/app-shell-commands/index.ts` — registers all 15, grouped by source-file domain (`commands/app.ts`, `commands/settings.ts`, `commands/drag.ts`)

Load convention: `load()` calls `ensureServices(this, ["commands", "app", "ui_state", "store"])` before `registerCommands`. Backend routing per the catalog:
- `app.quit`/`app.about`/`app.help` → `app` server
- `app.undo`/`app.redo` → **`store` server** (`store.undo`/`store.redo`) — undo/redo are store-layer, not app-shell
- `app.command`/`app.palette`/`app.search`/`app.dismiss` → `ui_state` server (UI toggles)
- `settings.keymap.*` → `ui_state` `SetKeymapMode`
- `drag.*` → `ui_state` `DragStart`/`DragCancel`/`DragComplete`

Keymap commands change the active keymap — affects how the palette/hotkey wiring resolves `keys` on `list command`.

## Acceptance Criteria
- [ ] `builtin/plugins/app-shell-commands/` discoverable
- [ ] All 15 commands registered with original metadata
- [ ] `app.undo`/`app.redo` invoke the `store` server and revert across all stores (entity/view/perspective) via the one shared stack
- [ ] `app.quit`/`about`/`help` route through the `app` server
- [ ] Keymap switches (vim/cua/emacs) update active keymap state via `ui_state`; hotkey dispatcher rebinds
- [ ] Drag start → complete state transitions land via `ui_state`
- [ ] Metadata fidelity per YAML baseline (every field for every command)

## Tests
- [ ] `crates/swissarmyhammer-command-service/tests/integration/builtin_app_shell_commands_e2e.rs` — load plugin; assert all 15 registered with metadata fidelity; execute `app.undo`/`app.redo` and observe the shared-stack revert (a kanban edit reverts)
- [ ] Keymap switching test: execute `settings.keymap.vim`; verify active keymap; switch to `cua`; verify
- [ ] Drag-cycle test: `drag.start` → `drag.complete`; assert state machine progressed
- [ ] Metadata fidelity table-test across all 15 commands
- [ ] `cargo test -p swissarmyhammer-command-service --test integration builtin_app_shell_commands_e2e` passes

## Workflow
- Use `/tdd`