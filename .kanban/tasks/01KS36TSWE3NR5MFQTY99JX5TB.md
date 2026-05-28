---
assignees:
- claude-code
depends_on:
- 01KS36QGEVVP064EKW0JDGD94B
- 01KS3BK37R2P0RYSYSXGZTS0Z3
- 01KS5E9M7ZNPNA0E7GR1C9N42R
- 01KS36VTN9K8C41P20SJ2WQA6X
- 01KS5MYQRB1E5HQ9JJ6TC7Z59S
position_column: review
position_ordinal: '80'
project: builtin-commands
title: 'Builtin plugin: ui commands (port ui.yaml — 10 commands incl. window.new)'
---
## What

Port `crates/swissarmyhammer-commands/builtin/commands/ui.yaml` (10 commands incl. `window.new`) to a builtin TypeScript plugin.

Commands: `ui.inspect`, `ui.inspector.close`, `ui.inspector.close_all`, `ui.inspector.set_width`, `ui.palette.open`, `ui.palette.close`, `ui.entity.startRename`, `ui.mode.set`, `ui.setFocus`, `window.new`.

Files:
- `builtin/plugins/ui-commands/index.ts` — `load()` calls `ensureServices(this, ["commands", "ui_state", "window", "focus"])` then `registerCommands(this, [...])`.

Backend routing:
- inspector/palette/mode/startRename/inspect → **ui_state** (Inspect/InspectorClose/…/SetKeymapMode/StartRename)
- `ui.setFocus` → **focus** server (spatial-nav project — `SpatialRegistry`/`SpatialState`)
- `window.new` → **window** `OpenNewWindow`

Memory `no-client-side-inspect`: `ui.inspect` dispatches through the backend (ui_state) like any other command — no React-side shortcut. Memory `useDispatchCommand-signature`: hook takes a command name string; frontend wiring stays `useDispatchCommand("ui.palette.open")`, only the backend changes.

## Acceptance Criteria
- [ ] `builtin/plugins/ui-commands/` discoverable
- [ ] All 10 commands registered with original metadata
- [ ] inspector/palette/mode/rename route to `ui_state`; `ui.setFocus` to `focus`; `window.new` to `window`
- [ ] `ui.inspect` routes via the Command service (not a client shortcut)
- [ ] `load()` calls `ensureServices(this, ["commands","ui_state","window","focus"])` before `registerCommands`
- [ ] Metadata fidelity per YAML baseline

## Tests
- [ ] `crates/swissarmyhammer-command-service/tests/integration/builtin_ui_commands_e2e.rs` — load; assert 10 registered; execute each and observe the side effect (inspector mounts, palette flag, mode switch, focus change, new window)
- [ ] regression: `ui.inspect` via Command service, not a React shortcut
- [ ] Metadata fidelity table-test
- [ ] `cargo test -p swissarmyhammer-command-service --test integration builtin_ui_commands_e2e` passes

## Workflow
- Use `/tdd`

Depends on ui_state + window servers and the `focus` server (spatial-nav project).