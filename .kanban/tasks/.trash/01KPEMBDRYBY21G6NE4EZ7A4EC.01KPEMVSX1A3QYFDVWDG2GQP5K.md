---
assignees:
- claude-code
depends_on:
- 01KPEMA771EPB8V51SPKAE0PBB
position_column: todo
position_ordinal: d380
title: 'Commands: collapse task.yaml entity schema to overlay-only references'
---
## What

`swissarmyhammer-kanban/builtin/entities/task.yaml` re-declares cross-cutting commands (`entity.copy`, `entity.cut`, `entity.paste`, `entity.archive`, `entity.unarchive`) with full `name`, `scope`, `undoable` fields — duplicating the authoritative declarations in `entity.yaml`. After this card, each entry is overlay-only: `id` plus the task-specific metadata (`context_menu: true`, `keys:`, optional templated `name`). No `scope` field, no `undoable` field, no re-declared `params`.

### Target shape for each entry

Example — `entity.copy` in `task.yaml` becomes:

```yaml
- id: entity.copy
  context_menu: true
  keys:
    cua: Mod+C
    vim: "y"
```

The `name: "Copy {{entity.type}}"` template resolves from `entity.yaml`'s declaration. If task needs a different display name, keep `name:` as overlay — but the default should come from the declaration.

Task-specific commands (`task.move`, `task.delete`, `task.untag`, `task.doThisNext`, `attachment.delete`) stay declared in full in the `task.yaml` *command* file (created in the prior card), and the entity schema just references them by `id` with overlay fields.

### Files to touch

- `swissarmyhammer-kanban/builtin/entities/task.yaml` — rewrite the `commands:` list so every entry is overlay-only.
- `swissarmyhammer-kanban/src/scope_commands.rs` — verify `emit_entity_schema_commands` correctly reads an entry that has only `id` + `context_menu` + `keys`. The existing `EntityCommand` struct in `swissarmyhammer-fields` already has all fields optional, so this should work; add a test that asserts merge-with-registry semantics if one doesn't exist.

### Subtasks

- [ ] Rewrite `task.yaml` `commands:` — strip redundant `name`, `scope`, `undoable` from entity.* entries.
- [ ] Keep `ui.inspect` entry (it's the per-entity overlay with `context_menu: true`).
- [ ] Keep task-specific references (`task.move`, `task.delete`, `task.untag`, `task.doThisNext`, `attachment.delete`) as overlay-only; their declarations live in `task.yaml` / `attachment.yaml` command files.

## Acceptance Criteria

- [ ] `task.yaml` entity schema entries for `entity.*` commands have no `scope:`, `undoable:`, or `params:` fields.
- [ ] Task context menu still shows Copy / Cut / Paste / Archive / Unarchive / Delete / Move / Do This Next / Remove Tag.
- [ ] Keybindings `Mod+C`, `Mod+X`, `Mod+V`, `dd`, `Mod+Backspace` still work on tasks.
- [ ] `yaml_hygiene_entity_schemas_overlay_only` (from card 01KPEM811W5XE6WVHDQVRCZ4B0) passes for `task.yaml` entries.

## Tests

- [ ] Existing `scope_commands.rs` tests that drive task-scope commands still pass — they're the regression safety net.
- [ ] Add `task_schema_overlay_inherits_declaration_name` in `swissarmyhammer-kanban/src/scope_commands.rs` tests: when task.yaml schema entry has only `id: entity.copy` + `context_menu: true`, the resolved `name` comes from the `entity.yaml` declaration (`"Copy {{entity.type}}"` → `"Copy Task"`).
- [ ] Run command: `cargo nextest run -p swissarmyhammer-kanban scope_commands entity_commands` — all tests green.

## Workflow

- Use `/tdd` — write the merge-semantics test first, then rewrite the YAML.

#commands

Depends on: 01KPEMA771EPB8V51SPKAE0PBB (scope pins gone first so task schema doesn't need to compensate)