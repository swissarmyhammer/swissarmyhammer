---
assignees:
- claude-code
depends_on:
- 01KS36QGEVVP064EKW0JDGD94B
- 01KS3BK37R2P0RYSYSXGZTS0Z3
position_column: todo
position_ordinal: 8a80
project: command-service
title: 'Builtin plugin: perspective commands (port perspective.yaml, 17 commands)'
---
## What

Port `crates/swissarmyhammer-kanban/builtin/commands/perspective.yaml` (17 commands) to a builtin TypeScript plugin. Largest single port — separate task because it dwarfs the others.

Commands to port: `perspective.load`, `perspective.save`, `perspective.delete`, `perspective.rename`, `perspective.filter.focus`, `perspective.filter`, `perspective.clearFilter`, `perspective.group`, `perspective.clearGroup`, `perspective.sort.set`, `perspective.sort.clear`, `perspective.sort.toggle`, `perspective.next`, `perspective.prev`, `perspective.goto`, `perspective.list`, `perspective.switch`.

Files:
- `builtin/plugins/perspective-commands/index.ts` — entry; `load()` calls `ensureServices(this, ["commands"])` then `registerCommands(this, [...])`. Given 17 commands, split the file into one helper module per logical sub-domain to keep `index.ts` readable:
  - `index.ts` — registers from arrays
  - `commands/filter.ts` — `filter`, `filter.focus`, `clearFilter`
  - `commands/group.ts` — `group`, `clearGroup`
  - `commands/sort.ts` — `sort.set`, `sort.clear`, `sort.toggle`
  - `commands/nav.ts` — `next`, `prev`, `goto`, `switch`
  - `commands/lifecycle.ts` — `load`, `save`, `delete`, `rename`, `list`

Each registration carries the full YAML metadata 1:1. Callbacks call into whatever MCP server hosts perspective state today (likely the `views`/`kanban` server).

Several commands take complex params (filter expressions, sort entries, perspective ids). Keep the param shape identical to the YAML — `from`/`entity_type`/`default` semantics carry across.

## Acceptance Criteria
- [ ] `builtin/plugins/perspective-commands/` discoverable
- [ ] All 17 commands appear in `list command` with metadata matching the YAML baseline
- [ ] `execute` for each command produces the same effect as today's YAML-driven version (per-command e2e)
- [ ] Sub-files keep each helper under 200 lines
- [ ] `load()` calls `ensureServices` before `registerCommands` (the convention)

## Tests
- [ ] `crates/swissarmyhammer-command-service/tests/integration/builtin_perspective_commands_e2e.rs` — load plugin; assert all 17 registered with metadata fidelity; execute representative commands from each sub-domain (filter, group, sort, nav, lifecycle) and observe perspective state changes
- [ ] Metadata-fidelity table-test asserting every YAML field on every command survives the port
- [ ] `cargo test -p swissarmyhammer-command-service --test integration builtin_perspective_commands_e2e` passes

## Workflow
- Use `/tdd`