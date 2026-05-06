---
assignees:
- claude-code
depends_on:
- 01KNFFCTKZ276HWZ3Z9HWNB70C
position_column: done
position_ordinal: ffffffffffffffffffffffffffc780
title: Populate entity YAML files with all entity-scoped commands from command YAML
---
## What

With `EntityCommand` now carrying full metadata (card 01KNFFCTKZ276HWZ3Z9HWNB70C), move every entity-scoped command from command YAML into the entity YAML that owns it. Commands stay in BOTH places for now (dual-source) — removal from command YAML is a separate card.

**Note:** `attachment.open` and `attachment.reveal` are NOT added here — they are field-level commands that belong on the `attachments` field definition, handled in card 01KNFFP5VSJHZRJVM5CPK87HDX.

**Note:** `entity.update_field` and `entity.delete` are NOT moved — they are generic commands that apply to all entity types. The command registry handles this via unscoped matching. Duplicating them across every entity YAML would be worse than the current approach.

### Commands to add to entity YAML files

**`swissarmyhammer-kanban/builtin/fields/entities/task.yaml`** — add:
- `task.add` (scope: entity:column, undoable, keys, params: column from scope_chain)
- `task.move` (scope: entity:task, undoable, params: task/column/drop_index)
- `task.delete` (scope: entity:task, undoable, context_menu, keys, params: task from scope_chain)
- `task.untag` (scope: entity:tag,entity:task, undoable, context_menu, keys, params: tag+task from scope_chain)
- `task.doThisNext` (scope: entity:task, undoable, context_menu, params: task from scope_chain)
- `attachment.delete` (scope: entity:task, undoable, visible:false, params: task_id+id from args)
- `entity.unarchive` (scope: entity:task, undoable, context_menu, params: moniker from target)

Also update existing inline commands (`entity.copy`, `entity.cut`, `entity.paste`, `entity.archive`) to include the full metadata (undoable, params, menu, scope) that currently only exists in `entity.yaml`.

**`swissarmyhammer-kanban/builtin/fields/entities/tag.yaml`** — add:
- `tag.update` (scope: entity:tag, undoable, visible:false, params: id from scope_chain)

**`swissarmyhammer-kanban/builtin/fields/entities/column.yaml`** — add:
- `column.reorder` (undoable, visible:false, params: id+target_index from args)

## Acceptance Criteria

- [ ] Every entity-specific command from `swissarmyhammer-commands/builtin/commands/entity.yaml` (except `entity.update_field` and `entity.delete`) has a corresponding entry in an entity YAML file
- [ ] `attachment.open`/`attachment.reveal` are NOT in entity YAML (they go on the field def)
- [ ] `cargo test -p swissarmyhammer-kanban` passes — `builtin_entities_parse_as_entity_def` confirms all YAML parses
- [ ] No behavior change — command YAML files still loaded, dual-source is fine for now

## Tests

- [ ] `swissarmyhammer-kanban/src/defaults.rs` — `builtin_entities_parse_as_entity_def` passes (validates all enriched YAML parses correctly)
- [ ] `swissarmyhammer-kanban/src/defaults.rs` — add test asserting task entity has >= 12 commands (current 5 + 7 new)
- [ ] Run `cargo test -p swissarmyhammer-kanban` — all pass

## Workflow

- Use `/tdd` — write failing tests first, then implement to make them pass.