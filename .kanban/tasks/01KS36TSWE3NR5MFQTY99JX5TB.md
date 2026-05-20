---
assignees:
- claude-code
depends_on:
- 01KS36QGEVVP064EKW0JDGD94B
- 01KS3BK37R2P0RYSYSXGZTS0Z3
position_column: todo
position_ordinal: 8c80
project: command-service
title: 'Builtin plugin: ui commands (port ui.yaml — 10 commands incl. window.new)'
---
## What

Port `crates/swissarmyhammer-commands/builtin/commands/ui.yaml` to a builtin TypeScript plugin. UI-state commands: palette, inspector, focus, mode, rename, plus `window.new` which lives here despite the `window.` prefix.

Commands (10): `ui.inspect`, `ui.inspector.close`, `ui.inspector.close_all`, `ui.inspector.set_width`, `ui.palette.open`, `ui.palette.close`, `ui.entity.startRename`, `ui.mode.set`, `ui.setFocus`, `window.new`.

Files:
- `builtin/plugins/ui-commands/index.ts` — registers all 10 commands

Load convention (see SDK helpers task): `load()` calls `ensureServices(this, ["commands", "window"])` (window is needed for `window.new`) before `registerCommands`. Idempotency guarantees other plugins can call the same `ensureServices` without collision.

Memory `no-client-side-inspect`: `ui.inspect` dispatches through the backend like any other command — no React-side shortcut. Preserve that. The command's `execute` callback calls into whatever MCP server hosts UI state today (likely a `ui` server) — same dispatch path as any other command.

Memory `useDispatchCommand-signature`: the hook takes a command name string. Frontend wiring to these commands stays as `useDispatchCommand("ui.palette.open")` — only the dispatch backend changes (it goes through Command service rather than the Rust registry).

`window.new` routes through the `window` MCP server's `open_new_window` operation.

## Acceptance Criteria
- [ ] `builtin/plugins/ui-commands/` discoverable
- [ ] All 10 commands registered with original metadata (including `window.new`)
- [ ] `ui.inspect` callback chain routes via the Command service end-to-end (not via a client-side shortcut)
- [ ] `ui.mode.set` switches modes as today; `ui.setFocus` updates focus state; `window.new` opens a new app window
- [ ] Metadata fidelity per YAML baseline

## Tests
- [ ] `crates/swissarmyhammer-command-service/tests/integration/builtin_ui_commands_e2e.rs` — load plugin; assert all 10 registered; execute each and observe the UI state side effect
- [ ] Specific regression test for `ui.inspect`: dispatches through the Command service, NOT through a React context shortcut
- [ ] `window.new` integration test: execute the command, assert a new tauri window exists in the app
- [ ] Metadata fidelity table-test (every YAML field assertion)
- [ ] `cargo test -p swissarmyhammer-command-service --test integration builtin_ui_commands_e2e` passes

## Workflow
- Use `/tdd`