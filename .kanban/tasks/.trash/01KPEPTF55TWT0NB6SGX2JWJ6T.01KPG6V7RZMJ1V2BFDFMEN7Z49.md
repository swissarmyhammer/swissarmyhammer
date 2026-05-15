---
assignees:
- claude-code
depends_on:
- 01KPEM811W5XE6WVHDQVRCZ4B0
position_column: todo
position_ordinal: da80
title: Reorganize entity.yaml — move per-type commands back to their entity YAMLs, keep only generic entity.* commands in entity.yaml
---
## What

`swissarmyhammer-commands/builtin/commands/entity.yaml` currently mixes two unrelated concerns:

1. **Generic cross-entity commands** (`entity.add`, `entity.update_field`, `entity.delete`, `entity.archive`, `entity.unarchive`, `entity.cut`, `entity.copy`, `entity.paste`) — these belong in `entity.yaml`.
2. **Per-type commands dressed up as "entity"** (`task.move`, `task.delete`, `task.untag`, `task.doThisNext`, `tag.update`, `column.reorder`, `attachment.delete`) — these are task/tag/column/attachment specific, not generic. They should live in their respective entity YAMLs.

Worse: several generic-looking commands in entity.yaml are actually task-only — `entity.archive` and `entity.unarchive` are `scope: "entity:task"`; `entity.cut` and `entity.copy` are `scope: "entity:task"`. They've been given the `entity.*` name but only work for tasks.

The user's words: "chock full of 'specific types'" / "you've used dice to organize this" / "let's randomly put specific entity commands in entity instead of their actual specific definition". This is the cleanup.

## Approach

### 1. Move per-type commands out of `swissarmyhammer-commands/builtin/commands/entity.yaml` into their entity-schema YAMLs at `swissarmyhammer-kanban/builtin/entities/<type>.yaml`

Entity-schema YAMLs already declare `commands:` arrays — these are the right home for per-type commands. Moves:
- `task.move`, `task.delete`, `task.untag`, `task.doThisNext` → `builtin/entities/task.yaml:commands:`. They already appear there as entity-schema commands pointing at the same ids — the entity.yaml registry entries are the dispatch-side declarations (params, undoable, scope). Both can co-exist, but the DECLARATION (name, keys, context_menu) should live with the entity. Audit to confirm no duplication ambiguity.
- `tag.update` → `builtin/entities/tag.yaml:commands:` (already exists as entity-schema command `tag.update`).
- `column.reorder` → `builtin/entities/column.yaml:commands:`.
- `attachment.delete` → `builtin/entities/attachment.yaml:commands:`.

### 2. Demote or rename task-specific `entity.*` commands

- `entity.archive` with `scope: "entity:task"` — either (a) broaden to all entity types that have an `archive` affordance, or (b) rename to `task.archive` and move to `task.yaml`. The schema says it's task-only; the name says it's generic. Pick one and commit.
- Same for `entity.unarchive`.
- `entity.cut` / `entity.copy` / `entity.paste` — currently `scope: "entity:task"`. Either generalise (so cut/copy/paste works for tags, projects, attachments too) or rename to `task.cut` etc. The clipboard impl (`CopyTaskCmd`, `CutTaskCmd`, `PasteTaskCmd`) already has "Task" in the name — these are task-only in Rust too.

### 3. Leave `entity.yaml` with ONLY truly generic commands

After the move, `entity.yaml` should contain exactly:
- `entity.add` (dispatch-side; dynamic palette item via `emit_entity_add`)
- `entity.update_field`
- `entity.delete`

Plus a header comment documenting what belongs in this file vs per-type entity YAMLs. User's quote: "make sure you leave comments in the yaml definitions with proper guidance of what belongs together".

### 4. Add a schema test that catches future drift

`swissarmyhammer-commands/src/registry.rs` tests — add a `no_per_type_commands_in_entity_yaml` test that scans the loaded commands with id prefix in {task., tag., column., project., attachment., board.} and asserts they were loaded from their respective entity YAML files, not from the generic `entity.yaml`. This codifies the rule so it can't erode again.

## Files to modify

- `swissarmyhammer-commands/builtin/commands/entity.yaml` — remove per-type command entries.
- `swissarmyhammer-kanban/builtin/entities/task.yaml` — add migrated command entries with full declarations.
- `swissarmyhammer-kanban/builtin/entities/tag.yaml` — ensure `tag.update` is the authoritative declaration.
- `swissarmyhammer-kanban/builtin/entities/column.yaml` — add `column.reorder`.
- `swissarmyhammer-kanban/builtin/entities/attachment.yaml` — add `attachment.delete`.
- `swissarmyhammer-kanban/builtin/entities/project.yaml` — if any project-only commands migrated here.
- `swissarmyhammer-commands/src/registry.rs` — add schema test.

## Acceptance Criteria

- [ ] `entity.yaml` contains ONLY `entity.*` prefixed commands.
- [ ] Every `task.*`, `tag.*`, `column.*`, `project.*`, `attachment.*`, `board.*` command is declared in its respective entity YAML.
- [ ] No command is declared in both `entity.yaml` AND an entity YAML (no duplication).
- [ ] Task-specific `entity.archive`/`entity.unarchive`/`entity.cut`/`entity.copy` are either renamed to `task.*` or genuinely generalised to all entity types.
- [ ] A header comment at the top of each YAML documents what belongs there (per the user: "proper guidance of what belongs together").
- [ ] New test `no_per_type_commands_in_entity_yaml` passes and would fail if someone re-introduces a per-type command in `entity.yaml`.
- [ ] All existing tests still pass.
- [ ] Palette + context menu behavior unchanged — no command id disappears, no command is resurfaced with a different target.

## Tests

- [ ] Schema test: iterate loaded commands, partition by id prefix, verify prefix → source file mapping.
- [ ] Existing `builtin_yaml_files_parse` test updated to reflect new counts.
- [ ] `cargo nextest run -p swissarmyhammer-kanban -p swissarmyhammer-commands` all pass.

## Workflow

- Use `/tdd` — write the drift-detection test first, confirm it fails with current layout (per-type commands present in entity.yaml), then do the moves and confirm it passes.

#entity #organization #commands