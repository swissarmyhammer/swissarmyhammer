---
assignees:
- claude-code
depends_on:
- 01KS36QGEVVP064EKW0JDGD94B
- 01KS3BK37R2P0RYSYSXGZTS0Z3
position_column: todo
position_ordinal: 8b80
project: command-service
title: 'Builtin plugin: entity + clipboard commands (port entity.yaml)'
---
## What

Port `crates/swissarmyhammer-commands/builtin/commands/entity.yaml` to a builtin TypeScript plugin. This is the cross-cutting entity CRUD layer plus clipboard ops.

Commands (8): `entity.add`, `entity.update_field`, `entity.delete`, `entity.archive`, `entity.unarchive`, `entity.cut`, `entity.copy`, `entity.paste` (plus the dynamic `entity.add:type` variants — those are runtime-expanded; figure out if they belong here or stay dynamic, see notes).

Files:
- `builtin/plugins/entity-commands/index.ts` — entry; `load()` calls `ensureServices(this, ["commands"])` then `registerCommands(this, [...])` with all 8 cross-cutting entity commands

These commands use `from: target` rather than `from: scope_chain` — they operate on whatever entity the user is targeting. Preserve this exactly in the `params` field of the registration. Frontend dispatch resolves `target` before invocation.

Dynamic `entity.add:type` commands: today the dispatcher generates these at runtime from registered entity types. Two options to evaluate during implementation:
1. The plugin registers a static `entity.add` and the frontend's palette UI synthesizes the per-type variants by reading the entity-type registry. (Simpler — keeps the registry small.)
2. The plugin enumerates entity types at load and registers one per type. (Discoverable via `list command` — preferable if other code wants to ask "what entity types can I create?")

Pick option 1 unless an integration test reveals a consumer that needs the static enumeration.

## Acceptance Criteria
- [ ] `builtin/plugins/entity-commands/` discoverable
- [ ] All 8 cross-cutting entity commands registered with original metadata
- [ ] `entity.paste`, `entity.cut`, `entity.copy` work end-to-end against the entity store
- [ ] Drag-vs-paste distinction (see memory: drag-vs-paste) preserved — internal drag is property mutation, external paste creates via PasteMatrix
- [ ] Metadata fidelity per YAML baseline
- [ ] `load()` calls `ensureServices` before `registerCommands` (the convention)

## Tests
- [ ] `crates/swissarmyhammer-command-service/tests/integration/builtin_entity_commands_e2e.rs` — load plugin; assert all 8 commands registered; exercise the CRUD round-trip (add → update_field → delete → unarchive)
- [ ] Clipboard round-trip test: copy a task; paste; assert duplicate exists in store
- [ ] Metadata fidelity table-test
- [ ] `cargo test -p swissarmyhammer-command-service --test integration builtin_entity_commands_e2e` passes

## Workflow
- Use `/tdd`