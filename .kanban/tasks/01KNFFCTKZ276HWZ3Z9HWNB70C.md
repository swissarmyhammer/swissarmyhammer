---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffa680
title: Extend EntityCommand struct to carry full CommandDef metadata
---
## What

`EntityCommand` in `swissarmyhammer-fields/src/types.rs` (lines 222-230) currently carries only `id`, `name`, `context_menu`, and `keys`. It needs to carry the same metadata as `CommandDef` in `swissarmyhammer-commands/src/types.rs` (lines 66-88) so entity YAML files can be the single source of truth for entity-scoped commands.

Add these optional fields to `EntityCommand`:
- `undoable: bool` (default false)
- `visible: Option<bool>` (default true, matching CommandDef)
- `params: Vec<ParamDef>` (reuse or mirror the type from swissarmyhammer-commands)
- `menu: Option<MenuPlacement>` (reuse or mirror)
- `menu_name: Option<String>`
- `scope: Option<String>` (for dual-scoped commands like `task.untag` which scopes to `entity:tag,entity:task`)

### Cross-crate dependency consideration

`EntityCommand` lives in `swissarmyhammer-fields`. `ParamDef` and `MenuPlacement` live in `swissarmyhammer-commands`. If fields already depends on commands, reuse the types directly. If not, either add the dependency or duplicate the struct definitions (with identical serde shapes) to avoid a circular dep. Check `swissarmyhammer-fields/Cargo.toml`.

### Frontend TypeScript mirror

Also update the TypeScript `EntityCommand` interface at `kanban-app/ui/src/types/kanban.ts` (lines 11-16) to match the new fields. The frontend receives serialized entity defs via IPC — the TS type must accept the new optional fields even if the frontend doesn't use them yet.

### Files to modify

- `swissarmyhammer-fields/src/types.rs` — extend `EntityCommand` with new optional fields
- `swissarmyhammer-fields/Cargo.toml` — possibly add `swissarmyhammer-commands` dep (or duplicate types)
- `kanban-app/ui/src/types/kanban.ts` — add optional fields to `EntityCommand` interface

## Acceptance Criteria

- [ ] `EntityCommand` has fields: `undoable`, `visible`, `params`, `menu`, `menu_name`, `scope`
- [ ] Existing entity YAML files (with only id/name/context_menu/keys) still parse without error
- [ ] New fields are `#[serde(default)]` so they're backward-compatible
- [ ] TypeScript `EntityCommand` interface mirrors the new optional fields
- [ ] `cargo test -p swissarmyhammer-fields` passes

## Tests

- [ ] `swissarmyhammer-fields/src/types.rs` — add YAML round-trip test for `EntityCommand` with all new fields populated
- [ ] `swissarmyhammer-fields/src/types.rs` — add test that existing minimal YAML (`{id: \"ui.inspect\", name: \"Inspect\"}`) still deserializes
- [ ] Run `cargo test -p swissarmyhammer-fields` — all pass

## Workflow

- Use `/tdd` — write failing tests first, then implement to make them pass.
