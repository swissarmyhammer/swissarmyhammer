---
assignees:
- claude-code
depends_on:
- 01KNFFH9SCAX7CDKARJVC8G688
position_column: done
position_ordinal: ffffffffffffffffffffb380
title: Remove entity-scoped commands from command YAML files (entity.yaml, attachment.yaml)
---
## What

After cards 1-3, entity definitions are the primary source for entity-scoped commands and `scope_commands.rs` reads from them. The command YAML files still contain duplicate entries that are now dead weight. Remove the duplicates.

### Commands to remove from `swissarmyhammer-commands/builtin/commands/entity.yaml`

Remove entity-specific entries that are now in entity YAML:
- `task.add`, `task.move`, `task.delete`, `task.untag`, `task.doThisNext`
- `entity.archive`, `entity.unarchive`
- `entity.cut`, `entity.copy`, `entity.paste`
- `tag.update`
- `column.reorder`
- `attachment.delete`

**Keep in `entity.yaml`** (generic, apply to all entity types):
- `entity.update_field` — unscoped, applies to any entity
- `entity.delete` — unscoped, applies to any entity

### Commands to remove from `swissarmyhammer-commands/builtin/commands/attachment.yaml`

`attachment.open` and `attachment.reveal` move to the `attachments` **field definition** (card 01KNFFP5VSJHZRJVM5CPK87HDX), not to an entity def. Once that card is complete and `scope_commands.rs` reads field-level commands, remove them from `attachment.yaml` and delete the file.

**This card should depend on the FieldDef commands card being wired into scope_commands.** If the FieldDef card (01KNFFP5VSJHZRJVM5CPK87HDX) only adds data without wiring, then `attachment.yaml` must stay until a follow-up card wires field-level commands into `scope_commands.rs`.

### Files to modify

- `swissarmyhammer-commands/builtin/commands/entity.yaml` — remove migrated entries, keep `entity.update_field` and `entity.delete`
- `swissarmyhammer-commands/builtin/commands/attachment.yaml` — delete file (only once field-level commands are wired)
- `swissarmyhammer-commands/src/registry.rs` — remove `attachment` from `builtin_yaml_sources()` if file deleted

## Acceptance Criteria

- [ ] Migrated entity-specific commands removed from `entity.yaml`
- [ ] `entity.update_field` and `entity.delete` remain in `entity.yaml`
- [ ] `attachment.yaml` deleted (or kept if field-level wiring isn't done yet — add a TODO)
- [ ] `builtin_yaml_sources()` updated if files deleted
- [ ] `cargo test -p swissarmyhammer-commands -p swissarmyhammer-kanban` passes
- [ ] Full `cargo test` passes

## Tests

- [ ] `swissarmyhammer-commands/src/registry.rs` — existing `load_builtin_yaml_files` test updated
- [ ] `swissarmyhammer-kanban/src/scope_commands.rs` — all existing tests still pass
- [ ] Run `cargo test` (workspace-wide) — all pass

## Workflow

- Use `/tdd` — write failing tests first, then implement to make them pass.
