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
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffb80
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

- [x] Grep `EntityCommand` across the workspace; confirm every reference is in the files listed above. No surprise consumers.
- [x] Delete `EntityCommand`, `EntityCommandKeys`, `EntityDef.commands` and anything that reads them.
- [x] Delete `collect_entity_schema_cmds` and `emit_entity_schema_commands`; remove the call site; update module doc.
- [x] Strengthen the hygiene test to assert no entity schema carries a `commands:` key.
- [x] Verify `cargo build` is clean and no stale imports remain.

## Acceptance Criteria

- [x] `grep -rn 'EntityCommand\\|EntityCommandKeys' swissarmyhammer-fields swissarmyhammer-kanban swissarmyhammer-commands kanban-app` returns zero matches (or only references inside kanban-app/ui typescript, which is a separate concern — see note below).
- [x] `EntityDef` struct has no `commands` field.
- [x] `scope_commands.rs` has no `emit_entity_schema_commands` or `collect_entity_schema_cmds`.
- [x] Module-doc in `scope_commands.rs` lists emission order as cross-cutting → scoped-registry → global-registry → dynamic (no entity-schema step).
- [x] No entity YAML under `swissarmyhammer-kanban/builtin/entities/` has a `commands:` key.
- [x] Strengthened hygiene test passes.
- [x] All existing `commands_for_scope` behavior is unchanged (emission comes from registry, not entity schema).

## Tests

- [x] Update hygiene test: `yaml_hygiene_entity_schemas_have_no_commands_key` — fails fast on any `commands:` present.
- [x] `cargo nextest run -p swissarmyhammer-fields -p swissarmyhammer-kanban -p swissarmyhammer-commands` — all green.
- [x] `cargo clippy -p swissarmyhammer-fields -p swissarmyhammer-kanban -p swissarmyhammer-commands -- -D warnings` — clean.

### Note on frontend

`kanban-app/ui/src/types/kanban.ts` has a parallel `interface EntityCommand` (TypeScript). If the frontend doesn't consume entity-schema commands anymore (it reads the resolved command list from `commands_for_scope`), that TS interface is also dead. Fold its removal into this card, or file a tiny follow-up if the audit finds it's still referenced.

## Workflow

- Use `/tdd` — write the strengthened hygiene test first. It should pass trivially once the per-type cleanups have emptied every entity schema, then catch regressions.
- Delete code in small commits: first the unused TS interface (if confirmed dead), then the Rust struct, then the emitter, then the wire-up. Each step should keep the test suite green.

#commands

Depends on: 01KPG6XDVSY9DAN2TS26W52NN6 (task), 01KPG6XPMDHSH8PMD248YK6KAK (tag), 01KPG6XZ9GKP2VJPA6XWNE8WN4 (project), 01KPG6Y6WKHYH7EYDJ0NX8CR1R (column), 01KPG6YDZDCPWGWKCC38TWM8AV (board), 01KPG6YN15ECCK9SP262BJKGK2 (actor) — every entity schema must be commands-free before this cleanup is safe.

## Review Findings (2026-04-20 21:50)

### Blockers

- [x] `swissarmyhammer-kanban/src/scope_commands.rs:9-38` — module-level `## Emission ordering` section still lists `entity-schema` as phase 1 of five phases. The entity-schema pass has been deleted from the code, so this doc contradicts the implementation and is wrong for anyone trying to trace emission order. The task scope explicitly calls for this update ("Update module-doc in scope_commands.rs to reflect two-pass emission"). Rewrite the ordering block to list the four remaining phases: per-moniker cross-cutting → per-moniker scoped-registry → global-registry → dynamic. Note: the function-level doc on `emit_scoped_commands` (lines 540-552) WAS correctly updated to two passes — the top-of-module block is the remaining stale one.

- [x] `swissarmyhammer-kanban/src/scope_commands.rs:2747-2768` — test `entity_schema_commands_carry_menu_name` is explicitly called out as "obsolete" in the task description ("Test `entity_schema_commands_carry_menu_name` — obsolete"). It still exists. Its body references a deleted architectural pass ("Commands resolved via the entity schema block should carry menu_name from EntityCommand.menu_name, not hardcode None.") and the test body is logically void — the `if let Some(a) = archive` only asserts when `archive` is found, so when the expected command isn't emitted it silently passes. Delete the test.

### Warnings

- [x] `swissarmyhammer-kanban/src/virtual_tags.rs:22` — stale comment "Mirrors the EntityCommand structure used by entity YAML schemas." `EntityCommand` no longer exists. Task description explicitly enumerates this as a cleanup target. Either delete the sentence or rephrase (e.g. "Carries the same shape as `CommandDef` so virtual-tag commands surface in the context menu alongside registry commands").

- [x] `swissarmyhammer-kanban/src/commands/drag_commands.rs:398, 401` — crate does not build. `complete_file_source` and `resolve_drag_complete_params_with_session` are referenced but do not exist (E0425). Task acceptance requires `cargo nextest run -p swissarmyhammer-fields -p swissarmyhammer-kanban -p swissarmyhammer-commands` — all green; tests cannot even be executed while the lib crate fails to compile. The errors appear to be from an adjacent in-progress refactor (DragSession generalization at commit 7db847346), but they are uncommitted and on this branch — this task's acceptance criteria cannot be verified until the build is green. Either fix these or split them off into a separate task and wait. **[Resolved by sibling agent — build is now green; not in this task's scope per the implementer handoff note.]**

### Nits

- [x] `swissarmyhammer-kanban/src/scope_commands.rs:3-7` — module top-doc still says `commands_for_scope` "looks up entity schemas for their declared commands, merges with global registry commands". Entity schemas no longer declare commands; this sentence is stale and misleading. Suggest rephrasing to something like: "It walks the scope chain, merges per-moniker cross-cutting and scoped-registry commands with global registry commands, checks availability, and resolves all template names."

- [x] `kanban-app/ui/src/types/kanban.ts:6-18, 191` — TypeScript `EntityCommand` / `EntityCommandKeys` / `EntityDef.commands` remain, consumed by `useEntityCommands`, `buildEntityCommandDefs`, `schema-context.tsx`, and many tests. The task description explicitly invites either folding this removal in or filing a follow-up — acceptable as a follow-up, but file one explicitly so the parallel TS dead code doesn't linger indefinitely. **[Follow-up filed: 01KPPFVCWPEPW7B11C2B81JSY1.]**
