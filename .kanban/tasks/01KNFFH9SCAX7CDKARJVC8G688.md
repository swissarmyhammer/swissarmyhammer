---
assignees:
- claude-code
depends_on:
- 01KNFFGHZJPP17RX608S1WZ2XX
position_column: done
position_ordinal: ffffffffffffffffffffb080
title: 'scope_commands: read entity-scoped commands from entity defs instead of registry'
---
## What

`commands_for_scope()` in `swissarmyhammer-kanban/src/scope_commands.rs` (line 175) currently reads commands from two sources:
1. **Entity schema commands** (lines 205-250) — reads `entity_def.commands` from `FieldsContext`, emits `ResolvedCommand` with `target` set
2. **Registry commands** (lines 253-306) — reads `CommandDef` from `CommandsRegistry`, matches by scope, emits `ResolvedCommand` without target

After cards 1+2, entity YAML files carry full metadata (params, undoable, menu, etc.). This card makes entity definitions the **primary** source for entity-scoped commands, replacing the registry lookup for those commands.

### Changes to `commands_for_scope()`

The entity schema commands block (lines 205-250) currently only maps `id`, `name`, `context_menu`, `keys`. It needs to also map:
- `menu_name` from `EntityCommand.menu_name`
- `keys` already handled
- The `available` check already works (uses `command_impls` map)

The registry commands block (lines 253-306) currently picks up entity-scoped commands that are NOT in entity YAML. After this card, those commands ARE in entity YAML, so the registry block will naturally skip them via the `seen` dedup set — no code change needed there.

The key change: the entity schema block must emit `ResolvedCommand` entries with `menu_name` populated (from `EntityCommand.menu_name`). Currently it hardcodes `menu_name: None`.

### Also update `ResolvedCommand` consumers

- `swissarmyhammer-kanban/src/scope_commands.rs` — the entity schema block
- `kanban-app/src/menu.rs` (line 296) — calls `commands_for_scope()`, uses `menu_name` for native menus. No change needed since it already reads `ResolvedCommand.menu_name`.

## Acceptance Criteria

- [ ] Entity schema commands block in `commands_for_scope()` maps `menu_name` from `EntityCommand`
- [ ] Commands that were previously only in the registry (e.g. `task.add`, `attachment.open`) now appear via the entity schema path
- [ ] Dedup via `seen` prevents double-emission (entity + registry)
- [ ] `cargo test -p swissarmyhammer-kanban` passes — all scope_commands tests green

## Tests

- [ ] `swissarmyhammer-kanban/src/scope_commands.rs` — add test: `task.add` appears in resolved commands when scope contains `column:todo` (previously only from registry, now from entity def)
- [ ] `swissarmyhammer-kanban/src/scope_commands.rs` — add test: `attachment.open` appears when scope contains `attachment:/path` (previously only from registry)
- [ ] `swissarmyhammer-kanban/src/scope_commands.rs` — existing test `entity_commands_have_correct_targets` still passes
- [ ] Run `cargo test -p swissarmyhammer-kanban` — all pass

## Workflow

- Use `/tdd` — write failing tests first, then implement to make them pass.
