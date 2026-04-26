---
assignees:
- claude-code
depends_on:
- 01KPEMA771EPB8V51SPKAE0PBB
position_column: todo
position_ordinal: d480
title: 'Commands: add entity.delete to tag.yaml schema; collapse tag archive/copy/cut entries to overlays'
---
## What

Two user-visible symptoms to fix on the tag entity:

1. **No delete tag context-menu command exists today.** `swissarmyhammer-kanban/builtin/entities/tag.yaml` does not list `entity.delete`. The generalized `entity.delete` in `swissarmyhammer-kanban/src/commands/entity_commands.rs::DeleteEntityCmd` already routes `tag:<id>` targets to `tag::DeleteTag` — it just never gets surfaced because the tag schema omits it.
2. **`entity.archive`, `entity.copy`, `entity.cut` are redeclared in tag.yaml.** Same pattern as task.yaml — these should be overlay-only references now that `entity.yaml` is the authoritative declaration.

### Target shape for tag.yaml `commands:` after this card

```yaml
commands:
  - id: ui.inspect
    name: "Inspect {{entity.type}}"
    context_menu: true
  - id: entity.delete
    context_menu: true
    keys:
      vim: dd
      cua: Mod+Backspace
  - id: entity.archive
    context_menu: true
  - id: entity.copy
    context_menu: true
  - id: entity.cut
    context_menu: true
  - id: tag.update
    visible: false
```

No `name:` overrides unless the default template isn't sufficient; no `scope:`, `undoable:`, or `params:` fields.

### Files to touch

- `swissarmyhammer-kanban/builtin/entities/tag.yaml` — add `entity.delete`, collapse other entity.* entries.

### Subtasks

- [ ] Add `entity.delete` entry with `context_menu: true` and keybindings.
- [ ] Strip `name`, `scope`, `undoable`, `params` from other `entity.*` entries.
- [ ] Confirm `tag.update` reference still works (its declaration moves to `tag.yaml` command file in card 01KPEM93Z47JSME10BY1JBGTFM).

## Acceptance Criteria

- [ ] Right-click on a tag shows **Delete Tag** — clicking it deletes the tag.
- [ ] Right-click on a tag shows **Archive Tag** — clicking it archives the tag.
- [ ] Tag YAML entries for `entity.*` commands have no `scope:`, `undoable:`, or `params:` fields.
- [ ] `yaml_hygiene_entity_schemas_overlay_only` passes for `tag.yaml`.

## Tests

- [ ] Add `delete_entity_surfaces_on_tag_context_menu` in `swissarmyhammer-kanban/src/scope_commands.rs` tests: with scope `["tag:01X"]` and target `"tag:01X"`, `commands_for_scope(..., context_menu_only = true, ...)` result contains `entity.delete` with `available: true`.
- [ ] Add `archive_entity_surfaces_on_tag_context_menu` with the same pattern for `entity.archive`.
- [ ] Extend `entity_commands.rs::delete_entity_deletes_tag` (already exists) — no new test needed, but verify it still passes after the YAML change.
- [ ] Run command: `cargo nextest run -p swissarmyhammer-kanban` — all tests green.

## Workflow

- Use `/tdd` — both surface tests fail on this branch; commit them failing, then edit `tag.yaml` to make them pass.

#commands

Depends on: 01KPEMA771EPB8V51SPKAE0PBB (un-pinning the cross-cutting commands is a prerequisite)