---
assignees:
- claude-code
depends_on:
- 01KNM9JJVA01Y8ZDWJSR4F799N
position_column: done
position_ordinal: ffffffffffffffffffffffba80
title: Define date field YAML definitions and register on task entity
---
## What

Create 6 new YAML field definitions in `swissarmyhammer-kanban/builtin/definitions/` and add them to the task entity's field list in `swissarmyhammer-kanban/builtin/entities/task.yaml`.

**User-set dates** (editable via date picker, stored as regular EAV fields):
- `due.yaml` — hard deadline, `type: { kind: date }`, `editor: date`, `display: date`, `sort: datetime`
- `scheduled.yaml` — earliest start date, same config

**System-derived dates** (computed from JSONL changelog, read-only):
- `created.yaml` — `type: { kind: computed, derive: derive-created, depends_on: [_changelog] }`, `editor: none`, `display: date`, `sort: datetime`
- `updated.yaml` — `type: { kind: computed, derive: derive-updated, depends_on: [_changelog] }`, same editor/display/sort
- `started.yaml` — `type: { kind: computed, derive: derive-started, depends_on: [_changelog] }`, same
- `completed.yaml` — `type: { kind: computed, derive: derive-completed, depends_on: [_changelog] }`, same

Add all 6 fields to `task.yaml` fields list. Place user-set dates in section `dates`, system-derived in section `system`.

Each YAML file needs a stable ID (use zero-padded sentinel format like existing builtins — e.g. `0000000000000000000000000U` for due, etc.).

## Acceptance Criteria
- [x] 6 new YAML files exist in `builtin/definitions/`
- [x] All 6 fields listed in `task.yaml` fields array
- [x] User-set dates use `kind: date`, `editor: date`
- [x] System dates use `kind: computed` with `derive` and `depends_on: [_changelog]`
- [x] `cargo test -p swissarmyhammer-fields` passes
- [x] Existing builtin tests in `defaults.rs` pass (counts updated: 29 fields, 18 task fields)

## Tests
- [x] Update `builtin_field_definitions_load` count assertion in `defaults.rs` (23 → 29)
- [x] Update `fields_for_entity` count assertion (12 → 18)
- [x] Add assertion in `builtin_task_entity_has_expected_fields` for new date fields
- [x] Verify all new fields parse correctly
- [x] `cargo test -p swissarmyhammer-kanban` passes (886 unit + integration tests green)

## Workflow
- Use `/tdd` — update test assertions first, then create YAML files to make them pass.

#task-dates