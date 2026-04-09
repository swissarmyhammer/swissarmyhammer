---
assignees:
- claude-code
depends_on:
- 01KNM9JJVA01Y8ZDWJSR4F799N
position_column: todo
position_ordinal: '9480'
title: Define date field YAML definitions and register on task entity
---
## What

Create 5 new YAML field definitions in `swissarmyhammer-kanban/builtin/definitions/` and add them to the task entity's field list in `swissarmyhammer-kanban/builtin/entities/task.yaml`.

**User-set dates** (editable via date picker, stored as regular EAV fields):
- `due.yaml` — hard deadline, `type: { kind: date }`, `editor: date`, `display: date`, `sort: datetime`
- `scheduled.yaml` — earliest start date, same config

**System-derived dates** (computed from JSONL changelog, read-only):
- `created.yaml` — `type: { kind: computed, derive: derive-created, depends_on: [_changelog] }`, `editor: none`, `display: date`, `sort: datetime`
- `updated.yaml` — `type: { kind: computed, derive: derive-updated, depends_on: [_changelog] }`, same editor/display/sort
- `started.yaml` — `type: { kind: computed, derive: derive-started, depends_on: [_changelog] }`, same
- `completed.yaml` — `type: { kind: computed, derive: derive-completed, depends_on: [_changelog] }`, same

Add all 5 fields to `task.yaml` fields list. Place user-set dates in section `dates`, system-derived in section `system`.

Each YAML file needs a stable ID (use zero-padded sentinel format like existing builtins — e.g. `0000000000000000000000000U` for due, etc.).

## Acceptance Criteria
- [ ] 5 new YAML files exist in `builtin/definitions/`
- [ ] All 5 fields listed in `task.yaml` fields array
- [ ] User-set dates use `kind: date`, `editor: date`
- [ ] System dates use `kind: computed` with `derive` and `depends_on: [_changelog]`
- [ ] `cargo test -p swissarmyhammer-fields` passes
- [ ] Existing builtin tests in `defaults.rs` pass (counts updated: 28 fields, 16 task fields)

## Tests
- [ ] Update `builtin_field_definitions_load` count assertion in `defaults.rs` (23 → 28)
- [ ] Update `fields_for_entity` count assertion if present
- [ ] Add assertion in `builtin_task_entity_has_expected_fields` for new date fields
- [ ] Verify all new fields parse correctly
- [ ] `cargo test -p swissarmyhammer-kanban` passes

## Workflow
- Use `/tdd` — update test assertions first, then create YAML files to make them pass.

#task-dates