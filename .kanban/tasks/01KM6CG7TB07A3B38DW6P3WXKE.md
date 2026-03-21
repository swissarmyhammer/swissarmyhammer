---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffce80
title: Add `commands` field to EntityDef (Rust + YAML)
---
## What

Add a `commands` field to the `EntityDef` struct in `swissarmyhammer-fields/src/types.rs`, mirroring how `ViewDef` already has a `commands: Vec<ViewCommand>` field. Reuse the existing `ViewCommand` type (or extract a shared `EntityCommand` type into `swissarmyhammer-fields` since that's where `EntityDef` lives).

Each command entry has:
- `id: String` — command identifier (e.g. `entity.inspect`, `entity.archive`)
- `name: String` — display name template using `{{entity.type}}` for the capitalized type name, `{{entity.<field>}}` for field values
- `context_menu: bool` — whether it appears in context menus (default false)
- `keys: Option<CommandKeys>` — optional keybindings per keymap mode

### Template convention
- `{{entity.type}}` — capitalized entity type name (\"task\" → \"Task\")
- `{{entity.<field>}}` — field value from the entity instance (e.g. `{{entity.title}}`, `{{entity.name}}`)

### Files to modify
- `swissarmyhammer-fields/src/types.rs` — add `EntityCommand` struct and `commands: Vec<EntityCommand>` to `EntityDef`
- `swissarmyhammer-kanban/builtin/fields/entities/task.yaml` — add commands
- `swissarmyhammer-kanban/builtin/fields/entities/tag.yaml` — add commands
- `swissarmyhammer-kanban/builtin/fields/entities/column.yaml` — add commands
- `swissarmyhammer-kanban/builtin/fields/entities/board.yaml` — add commands

### YAML for task.yaml example
```yaml
name: task
body_field: body
commands:
  - id: entity.inspect
    name: \"Inspect {{entity.type}}\"
    context_menu: true
  - id: entity.archive
    name: \"Archive {{entity.type}}\"
    context_menu: true
fields:
  - title
  - tags
  ...
```

## Acceptance Criteria
- [ ] `EntityDef` has a `commands` field that deserializes from YAML
- [ ] All 7 entity YAML files have appropriate commands declared (task, tag, column, board get inspect; task, tag get archive)
- [ ] Existing `get_entity_schema` IPC command returns commands in the JSON response
- [ ] Round-trip YAML serialization tests pass for EntityDef with commands

## Tests
- [ ] `swissarmyhammer-fields/src/types.rs` — add `entity_def_with_commands_yaml_round_trip` test
- [ ] `swissarmyhammer-fields/src/types.rs` — update `task_entity_def_from_yaml` test to include commands
- [ ] `cargo test -p swissarmyhammer-fields` passes