---
assignees:
- claude-code
depends_on:
- 01KPEM811W5XE6WVHDQVRCZ4B0
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffe080
title: 'Commands: entity.yaml — establish as cross-cutting home (un-pin scope, header comment)'
---
## What

Make `swissarmyhammer-commands/builtin/commands/entity.yaml` the authoritative home for cross-cutting commands — and *only* cross-cutting commands. This card does three things:

1. **Un-pin scope** on `entity.archive`, `entity.unarchive`, `entity.cut`, `entity.copy`, `entity.paste`. They currently have `scope: "entity:task"` which is a lie (archive/unarchive are already generic in Rust; cut/copy/paste will be generalized in H and I).
2. **Add a header comment** listing the cross-cutting IDs and explaining the rule: declarations here apply to any entity via `from: target`; per-type commands belong in their entity schema.
3. **Do NOT remove the per-type declarations yet** — `task.move`, `task.delete`, `tag.update`, `column.reorder`, `attachment.delete` stay in `entity.yaml` for now. Each per-type cleanup card (01KPG6GD34…-style) moves its own commands INTO its entity schema and OUT of entity.yaml atomically.

After this card lands, `entity.yaml` has the right contents for cross-cutting commands (no scope lies, clear comment) but still contains the per-type stragglers. Those get migrated one entity type at a time in the 6 follow-up cards.

### Files to touch

- `swissarmyhammer-commands/builtin/commands/entity.yaml` — strip 5 `scope:` lines, add header comment.

### Subtasks

- [x] Strip `scope: "entity:task"` from `entity.archive`, `entity.unarchive`, `entity.cut`, `entity.copy` (verify `entity.paste` already has no scope).
- [x] Add a `# ` header block explaining the cross-cutting rule and listing the IDs that belong here.

## Acceptance Criteria

- [x] `grep -n 'scope: "entity:task"' swissarmyhammer-commands/builtin/commands/entity.yaml` returns nothing for the cross-cutting commands (per-type stragglers remain by design — see point 3 above).
- [x] File opens with a `# ` comment block stating the rule and cross-cutting ID list.
- [x] `register_commands_returns_expected_count` still passes unchanged.
- [x] All existing tests still pass (the yaml_hygiene failure is pre-existing and intentional; it tracks the per-entity follow-up cards).

## Tests

- [x] Add `entity_archive_surfaces_on_non_task_entity` in `swissarmyhammer-kanban/src/scope_commands.rs` tests — with scope `["tag:01X"]`, `entity.archive` appears with `available: true` before any entity schema is touched.
- [x] Run command: `cargo nextest run -p swissarmyhammer-kanban scope_commands` — 80/81 pass (only the known `yaml_hygiene_no_cross_cutting_in_entity_schemas` foundation-branch failure remains, by design).

## Workflow

- Use `/tdd` — the `entity_archive_surfaces_on_non_task_entity` test fails on the current branch because of the scope pin; stripping the pin makes it pass.

## Implementation Notes

- The header comment was already added by 01KPEM811W5XE6WVHDQVRCZ4B0; verified it is sufficient (lists all cross-cutting IDs, explains the rule, and references the hygiene test).
- Stripped `scope: "entity:task"` from `entity.archive`, `entity.unarchive`, `entity.cut`, `entity.copy`. `entity.paste` had no scope already.
- Per-type declarations (`task.move`, `task.delete`, `task.doThisNext`, `attachment.delete`) intentionally retain their scope pin — they will migrate to per-entity schemas in the 6 follow-up cards.
- Added `entity_archive_surfaces_on_non_task_entity` test in `swissarmyhammer-kanban/src/scope_commands.rs` near the related entity-schema tests. Test passes today via the entity-schema path (`tag.yaml` lists `entity.archive`); it will continue to pass via the registry path after the follow-up cards strip those duplicates.
- Verified: `register_commands_returns_expected_count` passes; full kanban+commands suite is 1239/1240 (only the documented foundation-branch failure remains).

#commands

Depends on: 01KPEM811W5XE6WVHDQVRCZ4B0 (rule documented first)