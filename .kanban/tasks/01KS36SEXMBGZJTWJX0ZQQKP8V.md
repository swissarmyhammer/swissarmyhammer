---
assignees:
- claude-code
depends_on:
- 01KS36QGEVVP064EKW0JDGD94B
- 01KS3BK37R2P0RYSYSXGZTS0Z3
- 01KS36VTN9K8C41P20SJ2WQA6X
position_column: todo
position_ordinal: '8980'
project: builtin-commands
title: 'Builtin plugin: file/board commands (port file.yaml)'
---
## What

Port `crates/swissarmyhammer-kanban/builtin/commands/file.yaml` (4 commands: `file.switchBoard`, `file.closeBoard`, `file.newBoard`, `file.openBoard`) to a builtin TypeScript plugin.

(NOTE: `window.new` lives in `ui.yaml`, not `file.yaml` — it is handled by the ui-commands plugin task.)

Files:
- `builtin/plugins/file-commands/index.ts` — entry; `load()` calls `ensureServices(this, ["commands", "window"])` then `registerCommands(this, [...])` with all 4 commands. The `window` service is needed for `file.openBoard` (OS file dialog).

These commands cross into the host shell (board file management), so several will route through the `window` MCP server (defined in a later task) for OS file dialogs, plus the kanban server for new-board creation. That's fine — at registration time the plugin just declares the command; the callback target is decided at call time.

Sequencing note: this plugin can register the commands before the `window` MCP server exists; the `execute` callbacks will fail at runtime if `window` isn't registered yet, but that's caught by the frontend e2e tests, not by the plugin-load tests. Ordering with the `window` server task is loose — the cut-over task gates the final user-visible behavior.

## Acceptance Criteria
- [ ] `builtin/plugins/file-commands/` discoverable
- [ ] All 4 commands appear in `list command` with original metadata
- [ ] `execute` for each, when its downstream MCP server is registered, produces the same effect as today's YAML version
- [ ] Metadata (keys, scope, params, menu placement) matches the YAML baseline exactly
- [ ] `load()` calls `ensureServices` before `registerCommands` (the convention)

## Tests
- [ ] `crates/swissarmyhammer-command-service/tests/integration/builtin_file_commands_e2e.rs` — load plugin; assert all 4 registered with metadata fidelity; execute `file.newBoard` (which only touches the store) and assert the new board file appears
- [ ] Metadata-fidelity tests for each command
- [ ] `cargo test -p swissarmyhammer-command-service --test integration builtin_file_commands_e2e` passes

## Workflow
- Use `/tdd`