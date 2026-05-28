---
assignees:
- claude-code
depends_on:
- 01KS36QGEVVP064EKW0JDGD94B
- 01KS3BK37R2P0RYSYSXGZTS0Z3
- 01KS5EAD57PCBFJGMVB74FF4MK
- 01KS614S1YAVEWVR1RHP62SQF0
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffca80
project: builtin-commands
title: 'Builtin plugin: entity + clipboard commands (port entity.yaml)'
---
## What

Port `crates/swissarmyhammer-commands/builtin/commands/entity.yaml` to a builtin TypeScript plugin. Cross-cutting entity CRUD + clipboard — all routing to the new generic **`entity` MCP server** (see the entity-service plan), not `kanban`.

Commands (8): `entity.add`, `entity.update_field`, `entity.delete`, `entity.archive`, `entity.unarchive`, `entity.cut`, `entity.copy`, `entity.paste`.

Files:
- `builtin/plugins/entity-commands/index.ts` — `load()` calls `ensureServices(this, ["commands", "entity"])` then `registerCommands(this, [...])`.

Backend routing (all → `entity` server):
- `entity.add` → `entity` `AddEntity { type, fields }`
- `entity.update_field` → `entity` `UpdateField { type, id, field, value }`
- `entity.delete` → `entity` `DeleteEntity`
- `entity.archive` / `entity.unarchive` → `entity` `ArchiveEntity` / `UnarchiveEntity`
- `entity.cut` / `entity.copy` / `entity.paste` → `entity` `Cut` / `Copy` / `Paste`

These commands use `from: target` (operate on whatever entity the user targets); preserve that in `params`. Dynamic `entity.add:type` variants are synthesized client-side by the palette from the entity-type registry (option 1) unless a test shows a consumer needs the static enumeration.

Drag-vs-paste distinction (memory: drag-vs-paste) preserved — external paste creates via PasteMatrix (the `entity` server's `Paste`), not the internal-drag property mutation.

## Acceptance Criteria
- [ ] `builtin/plugins/entity-commands/` discoverable
- [ ] All 8 commands registered with original metadata
- [ ] Each routes to the `entity` server; CRUD + archive + clipboard work end-to-end and are undoable via the shared stack
- [ ] `load()` calls `ensureServices(this, ["commands", "entity"])` before `registerCommands`
- [ ] Metadata fidelity per YAML baseline

## Tests
- [ ] `crates/swissarmyhammer-command-service/tests/integration/builtin_entity_commands_e2e.rs` — load plugin; assert all 8 registered; CRUD round-trip (add → update_field → delete → unarchive) via `entity`; clipboard copy→paste creates a duplicate
- [ ] Metadata fidelity table-test
- [ ] `cargo test -p swissarmyhammer-command-service --test integration builtin_entity_commands_e2e` passes

## Workflow
- Use `/tdd`