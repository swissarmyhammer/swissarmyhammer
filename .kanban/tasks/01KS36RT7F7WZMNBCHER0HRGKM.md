---
assignees:
- claude-code
depends_on:
- 01KS36QGEVVP064EKW0JDGD94B
- 01KS3BK37R2P0RYSYSXGZTS0Z3
position_column: todo
position_ordinal: '8880'
project: command-service
title: 'Builtin plugin: column/attachment/tag/view commands (port 4 small YAMLs)'
---
## What

Port the four small kanban-domain YAML files to a single builtin TypeScript plugin (they're small individually — 1-2 commands each — and tightly coupled to the same kanban server, so bundling reduces plugin overhead).

Source YAMLs (5 commands total):
- `crates/swissarmyhammer-kanban/builtin/commands/column.yaml` — `column.reorder`
- `crates/swissarmyhammer-kanban/builtin/commands/attachment.yaml` — `attachment.open`, `attachment.reveal`
- `crates/swissarmyhammer-kanban/builtin/commands/tag.yaml` — `tag.update`
- `crates/swissarmyhammer-kanban/builtin/commands/view.yaml` — `view.set`

Files:
- `builtin/plugins/kanban-misc-commands/index.ts` — entry; `load()` calls `ensureServices(this, ["commands"])` then `registerCommands(this, [...])` with all 5 commands grouped by their domain (column, attachment, tag, view)

Each registration must preserve the original YAML's metadata 1:1 (keys, scope, params, undoable, context_menu, tab_button). The implementation pattern matches `task-commands`: callbacks call into the kanban MCP server.

`attachment.open` and `attachment.reveal` may need to call out to a `files` or `shell` MCP server (whichever the host exposes for opening files in the OS default app and revealing in Finder). Reuse whatever the current YAML-driven implementation calls.

## Acceptance Criteria
- [ ] `builtin/plugins/kanban-misc-commands/` discoverable by the builtin layer
- [ ] After load, all 5 commands appear in `list command` with metadata matching the YAML baselines
- [ ] `execute command` for each of the 5 ids produces the same observable effect as today's YAML-driven version
- [ ] `available` reflects each command's original preconditions
- [ ] `load()` calls `ensureServices` before `registerCommands` (the convention)

## Tests
- [ ] `crates/swissarmyhammer-command-service/tests/integration/builtin_kanban_misc_e2e.rs` — load `kanban-misc-commands`; assert all 5 commands registered with correct metadata; execute each and observe effect
- [ ] Metadata-fidelity tests for each command (one assertion per YAML field)
- [ ] `cargo test -p swissarmyhammer-command-service --test integration builtin_kanban_misc_e2e` passes

## Workflow
- Use `/tdd` — metadata fidelity tests first; then implementation.