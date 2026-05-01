---
assignees:
- claude-code
depends_on:
- 01KPEM811W5XE6WVHDQVRCZ4B0
position_column: todo
position_ordinal: d180
title: 'Commands: move task/tag/column/attachment declarations out of entity.yaml into per-noun files'
---
## What

`swissarmyhammer-commands/builtin/commands/entity.yaml` today bundles type-specific command declarations that should live with their noun. After this card, `entity.yaml` contains ONLY cross-cutting commands.

### Commands that must move OUT of `entity.yaml`

- `task.move`, `task.delete`, `task.untag`, `task.doThisNext` → new file `swissarmyhammer-commands/builtin/commands/task.yaml`
- `tag.update` → new file `swissarmyhammer-commands/builtin/commands/tag.yaml`
- `column.reorder` → new file `swissarmyhammer-commands/builtin/commands/column.yaml`
- `attachment.delete` → existing file `swissarmyhammer-commands/builtin/commands/attachment.yaml` (which already holds `attachment.open`, `attachment.reveal`)

### What stays in `entity.yaml`

`entity.add`, `entity.delete`, `entity.archive`, `entity.unarchive`, `entity.cut`, `entity.copy`, `entity.paste`, `entity.update_field`. (Scope pins are removed in a later card — do not touch them here.)

### Files to touch

- CREATE `swissarmyhammer-commands/builtin/commands/task.yaml`
- CREATE `swissarmyhammer-commands/builtin/commands/tag.yaml`
- CREATE `swissarmyhammer-commands/builtin/commands/column.yaml`
- EDIT `swissarmyhammer-commands/builtin/commands/attachment.yaml` (add `attachment.delete`)
- EDIT `swissarmyhammer-commands/builtin/commands/entity.yaml` (remove the moved commands)
- VERIFY `swissarmyhammer-commands/src/registry.rs::builtin_yaml_sources()` picks up the new YAML files automatically (it uses `include_dir!`, so new files should be picked up — confirm with a test).

### Subtasks

- [ ] Create `task.yaml` with `task.move`, `task.delete`, `task.untag`, `task.doThisNext` copied verbatim from `entity.yaml`.
- [ ] Create `tag.yaml` with `tag.update` copied verbatim.
- [ ] Create `column.yaml` with `column.reorder` copied verbatim.
- [ ] Append `attachment.delete` to the existing `attachment.yaml`.
- [ ] Delete the moved entries from `entity.yaml`.

## Acceptance Criteria

- [ ] `grep -n 'id: task\\.' swissarmyhammer-commands/builtin/commands/entity.yaml` returns nothing.
- [ ] `grep -n 'id: tag\\.update' swissarmyhammer-commands/builtin/commands/entity.yaml` returns nothing.
- [ ] `grep -n 'id: column\\.' swissarmyhammer-commands/builtin/commands/entity.yaml` returns nothing.
- [ ] `grep -n 'id: attachment\\.' swissarmyhammer-commands/builtin/commands/entity.yaml` returns nothing.
- [ ] `register_commands_returns_expected_count` test in `swissarmyhammer-kanban/src/commands/mod.rs` still passes — total command count unchanged (this card only relocates declarations, does not add/remove commands).
- [ ] `test_all_yaml_commands_have_rust_implementations` still passes.

## Tests

- [ ] Add `builtin_yaml_sources_includes_new_files` in `swissarmyhammer-commands/src/registry.rs` tests: assert the loader returns entries for `task.yaml`, `tag.yaml`, `column.yaml`.
- [ ] Run command: `cargo nextest run -p swissarmyhammer-commands -p swissarmyhammer-kanban` — all tests green.

## Workflow

- Use `/tdd` — write the new `builtin_yaml_sources_includes_new_files` test first (fails because files don't exist), then move the YAML blocks, then confirm green.

#commands

Depends on: 01KPEM811W5XE6WVHDQVRCZ4B0 (rule must be documented before mechanical moves)