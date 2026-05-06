---
assignees:
- claude-code
depends_on:
- 01KNFFCTKZ276HWZ3Z9HWNB70C
position_column: done
position_ordinal: ffffffffffffffffffffffffffc880
title: Add commands field to FieldDef for field-specific commands (e.g. attachment.open)
---
## What

`EntityDef` has a `commands: Vec<EntityCommand>` field for entity-level commands, but `FieldDef` has no equivalent. Field-specific commands like `attachment.open` and `attachment.reveal` naturally belong on the `attachments` field definition rather than in a separate command YAML file in the commands crate.

Add a `commands: Vec<EntityCommand>` field to `FieldDef` (reusing the same `EntityCommand` type from card 01KNFFCTKZ276HWZ3Z9HWNB70C) and populate it for the `attachments` field.

### Files to modify

1. **`swissarmyhammer-fields/src/types.rs`** — add `commands: Vec<EntityCommand>` to `FieldDef` (line 111-139), with `#[serde(default, skip_serializing_if = \"Vec::is_empty\")]`

2. **`kanban-app/ui/src/types/kanban.ts`** — add `commands?: readonly EntityCommand[]` to `FieldDef` interface (line 112-130)

3. **`swissarmyhammer-kanban/builtin/fields/definitions/attachments.yaml`** — add commands section:
   ```yaml
   commands:
     - id: attachment.open
       name: Open
       scope: attachment
       context_menu: true
     - id: attachment.reveal
       name: Show in Finder
       scope: attachment
       context_menu: true
   ```

### Scope commands integration (future)

`commands_for_scope()` in `swissarmyhammer-kanban/src/scope_commands.rs` does NOT need to change in this card — it already picks up `attachment.open`/`attachment.reveal` from the command registry. Once the entity-command consolidation cards are complete, the field-level commands will be read from field definitions instead. This card just gets the data into the right place.

## Acceptance Criteria

- [ ] `FieldDef` has a `commands` field of type `Vec<EntityCommand>` with serde default
- [ ] TypeScript `FieldDef` interface has optional `commands` field
- [ ] `attachments.yaml` declares `attachment.open` and `attachment.reveal` commands
- [ ] Existing field YAML files without commands still parse (serde default)
- [ ] `cargo test -p swissarmyhammer-fields -p swissarmyhammer-kanban` passes

## Tests

- [ ] `swissarmyhammer-fields/src/types.rs` — add test: `FieldDef` with commands round-trips through YAML
- [ ] `swissarmyhammer-fields/src/types.rs` — add test: `FieldDef` without commands still deserializes (backward compat)
- [ ] `swissarmyhammer-kanban/src/defaults.rs` — `builtin_fields_parse_as_field_def` still passes with enriched `attachments.yaml`
- [ ] Run `cargo test -p swissarmyhammer-fields -p swissarmyhammer-kanban` — all pass

## Workflow

- Use `/tdd` — write failing tests first, then implement to make them pass.