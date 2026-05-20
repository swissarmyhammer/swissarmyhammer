---
assignees:
- claude-code
depends_on:
- 01KS36QGEVVP064EKW0JDGD94B
- 01KS3BK37R2P0RYSYSXGZTS0Z3
position_column: todo
position_ordinal: 8d80
project: command-service
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

Load convention (see SDK helpers task): `load()` calls `ensureServices(this, ["commands", "app"])` before `registerCommands`. The `app` service is needed for `app.quit`, `app.undo`, `app.redo`, etc. Idempotency means other plugins also calling `ensureServices` for these names is safe.

App-level commands route through the `app` MCP server (separate task). Keymap commands change the active keymap — this affects how the palette/hotkey wiring task resolves `keys` on `list command`. Drag commands route through whatever state machine hosts drag (today React state; for now `execute` posts to the frontend's drag state via an MCP operation).

## Acceptance Criteria
- [ ] `builtin/plugins/app-shell-commands/` discoverable
- [ ] All 15 commands registered with original metadata
- [ ] `app.undo` and `app.redo` exercise the undo stack end-to-end (the unified `undo_stack.yaml`)
- [ ] `app.quit`, `app.about`, `app.help` route through the `app` MCP server (when registered)
- [ ] Keymap switches (vim/cua/emacs) update the active keymap state and the hotkey dispatcher rebinds
- [ ] Drag start → complete state transitions land
- [ ] Metadata fidelity per YAML baseline (every field for every command)

## Tests
- [ ] `crates/swissarmyhammer-command-service/tests/integration/builtin_app_shell_commands_e2e.rs` — load plugin; assert all 15 registered with metadata fidelity; execute `app.undo` / `app.redo` and observe undo-stack mutation
- [ ] Keymap switching test: execute `settings.keymap.vim`; verify the active keymap state; execute `settings.keymap.cua`; verify the switch
- [ ] Drag-cycle test: `drag.start` → `drag.complete`; assert state machine progressed
- [ ] Metadata fidelity table-test across all 15 commands
- [ ] `cargo test -p swissarmyhammer-command-service --test integration builtin_app_shell_commands_e2e` passes

## Workflow
- Use `/tdd`