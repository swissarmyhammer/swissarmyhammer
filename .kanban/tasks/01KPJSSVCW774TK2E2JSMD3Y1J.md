---
assignees:
- claude-code
depends_on:
- 01KPG6XDVSY9DAN2TS26W52NN6
- 01KPG6XPMDHSH8PMD248YK6KAK
- 01KPG6XZ9GKP2VJPA6XWNE8WN4
- 01KPG6Y6WKHYH7EYDJ0NX8CR1R
- 01KPG6YDZDCPWGWKCC38TWM8AV
- 01KPG6YN15ECCK9SP262BJKGK2
position_column: todo
position_ordinal: f480
title: 'Commands: retire EntityCommand / EntityCommandKeys — drop EntityDef.commands, delete dead emitter'
---
## What

Once every entity schema has lost its `commands:` list (per the 6 per-type cleanup cards), `EntityCommand` + `EntityCommandKeys` + `EntityDef.commands` + the code that reads them is dead weight. The type is a near-duplicate of `CommandDef` (same 7 fields, minus `params` and `menu`), and `EntityCommandKeys` is a byte-for-byte duplicate of `KeysDef`. Their in-code doc even admits the duplication: *"Carries the same metadata as `CommandDef` so entity YAML files can be the single source of truth for entity-scoped commands."* That source-of-truth argument dies with the new architecture.

### Definitions to delete

- `swissarmyhammer-fields/src/types.rs`:
  - `pub struct EntityCommand`
  - `pub struct EntityCommandKeys`
  - The `pub commands: Vec<EntityCommand>` field on `EntityDef` (plus its `#[serde(default)]` wiring)
  - Any doc comments or tests in that file that reference `EntityCommand` (e.g. `commands:` fixtures inside `entity_def_yaml_round_trip`)

### Code to delete

- `swissarmyhammer-kanban/src/scope_commands.rs`:
  - `fn collect_entity_schema_cmds` — reads `EntityDef.commands`
  - `fn emit_entity_schema_commands` — the per-entity emission pass that used the list
  - The call site of `emit_entity_schema_commands` inside `emit_scoped_commands` (or wherever it's wired in)
  - Update the module-doc emission ordering to remove the entity-schema step. New order: cross-cutting → scoped-registry → global-registry → dynamic.
  - Test `entity_schema_commands_carry_menu_name` — obsolete
  - Any other test that seeds a `commands:` list into a fixture `EntityDef` — update or delete
- `swissarmyhammer-kanban/src/virtual_tags.rs` — the comment *"Mirrors the EntityCommand structure used by entity YAML schemas"* is now stale; either delete the line or rephrase to reference `CommandDef` if the structural similarity still matters.

### YAML cleanup

- `swissarmyhammer-kanban/builtin/entities/attachment.yaml` has `commands: []` — remove the key entirely.
- Any other entity YAML that still has `commands:` present (empty or otherwise) — remove the key. Should be zero by the time this card lands, but grep to confirm.

### Hygiene test update

The hygiene test from card A (`yaml_hygiene_no_cross_cutting_in_entity_schemas`) scans `commands:` entries for cross-cutting ids. After this card lands, no entity schema has a `commands:` key at all. Rewrite the test to assert exactly that: *every entity YAML has no `commands:` key* — stronger than the current assertion and catches regressions where someone re-adds the field.

### Files to touch

- MODIFY `swissarmyhammer-fields/src/types.rs` — delete the two structs + `EntityDef.commands` field + related tests
- MODIFY `swissarmyhammer-kanban/src/scope_commands.rs` — delete the two functions, their call site, module-doc update, obsolete tests
- MODIFY `swissarmyhammer-kanban/src/virtual_tags.rs` — stale comment
- MODIFY `swissarmyhammer-kanban/builtin/entities/attachment.yaml` — drop `commands: []`
- MODIFY `swissarmyhammer-kanban/src/scope_commands.rs` tests — strengthen hygiene test

### Subtasks

- [ ] Grep `EntityCommand` across the workspace; confirm every reference is in the files listed above. No surprise consumers.
- [ ] Delete `EntityCommand`, `EntityCommandKeys`, `EntityDef.commands` and anything that reads them.
- [ ] Delete `collect_entity_schema_cmds` and `emit_entity_schema_commands`; remove the call site; update module doc.
- [ ] Strengthen the hygiene test to assert no entity schema carries a `commands:` key.
- [ ] Verify `cargo build` is clean and no stale imports remain.

## Acceptance Criteria

- [ ] `grep -rn 'EntityCommand\\|EntityCommandKeys' swissarmyhammer-fields swissarmyhammer-kanban swissarmyhammer-commands kanban-app` returns zero matches (or only references inside kanban-app/ui typescript, which is a separate concern — see note below).
- [ ] `EntityDef` struct has no `commands` field.
- [ ] `scope_commands.rs` has no `emit_entity_schema_commands` or `collect_entity_schema_cmds`.
- [ ] Module-doc in `scope_commands.rs` lists emission order as cross-cutting → scoped-registry → global-registry → dynamic (no entity-schema step).
- [ ] No entity YAML under `swissarmyhammer-kanban/builtin/entities/` has a `commands:` key.
- [ ] Strengthened hygiene test passes.
- [ ] All existing `commands_for_scope` behavior is unchanged (emission comes from registry, not entity schema).

## Tests

- [ ] Update hygiene test: `yaml_hygiene_entity_schemas_have_no_commands_key` — fails fast on any `commands:` present.
- [ ] `cargo nextest run -p swissarmyhammer-fields -p swissarmyhammer-kanban -p swissarmyhammer-commands` — all green.
- [ ] `cargo clippy -p swissarmyhammer-fields -p swissarmyhammer-kanban -p swissarmyhammer-commands -- -D warnings` — clean.

### Note on frontend

`kanban-app/ui/src/types/kanban.ts` has a parallel `interface EntityCommand` (TypeScript). If the frontend doesn't consume entity-schema commands anymore (it reads the resolved command list from `commands_for_scope`), that TS interface is also dead. Fold its removal into this card, or file a tiny follow-up if the audit finds it's still referenced.

## Workflow

- Use `/tdd` — write the strengthened hygiene test first. It should pass trivially once the per-type cleanups have emptied every entity schema, then catch regressions.
- Delete code in small commits: first the unused TS interface (if confirmed dead), then the Rust struct, then the emitter, then the wire-up. Each step should keep the test suite green.

#commands

Depends on: 01KPG6XDVSY9DAN2TS26W52NN6 (task), 01KPG6XPMDHSH8PMD248YK6KAK (tag), 01KPG6XZ9GKP2VJPA6XWNE8WN4 (project), 01KPG6Y6WKHYH7EYDJ0NX8CR1R (column), 01KPG6YDZDCPWGWKCC38TWM8AV (board), 01KPG6YN15ECCK9SP262BJKGK2 (actor) — every entity schema must be commands-free before this cleanup is safe.