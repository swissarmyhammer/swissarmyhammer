---
assignees:
- claude-code
position_column: todo
position_ordinal: '7e80'
title: Wire project field into task entity definition
---
## What

The `project` field definition exists (`builtin/definitions/project.yaml`) but was never added to the task entity's `fields` list in `builtin/entities/task.yaml`. This was missed during the swimlane-to-project migration (`d47ba21ac`). The field def is a `reference` to the `project` entity type with `multiple: false`, `editor: multi-select`, `display: badge-list`, `groupable: true`.

Add `project` to the task entity's fields list in `task.yaml`. Place it after `assignees` (or wherever makes sense in the field ordering — it's a header-section field like assignees and tags).

**Files to modify:**
- `swissarmyhammer-kanban/builtin/entities/task.yaml` — add `- project` to the fields list

**Files to update (test counts):**
- `swissarmyhammer-kanban/src/defaults.rs` — update `fields_for_entity(\"task\")` count assertion (11 → 12)
- Any other tests that assert on the task field count

**Downstream effects:**
- Tasks will gain a `project` field visible in inspector and grid views
- MCP `add task` / `update task` should accept a `project` parameter (check if task_commands.rs needs updating)
- Existing tasks with no project field will show empty/null — no migration needed

## Acceptance Criteria
- [ ] `project` field appears in task entity's fields list
- [ ] `cargo test -p swissarmyhammer-fields` passes
- [ ] `cargo test -p swissarmyhammer-kanban` passes (count assertions updated)
- [ ] Reading a task entity includes the `project` field (null when unset)
- [ ] `builtin_entity_fields_reference_existing_field_defs` test passes (project def already exists)

## Tests
- [ ] Update field count assertion in `defaults.rs` `from_yaml_sources_builds_valid_context` (11 → 12 task fields)
- [ ] Add assertion in `builtin_task_entity_has_expected_fields` for `project` field
- [ ] Integration test: create task with project set → read task → verify project field returned
- [ ] Integration test: update task to set/clear project
- [ ] `cargo test -p swissarmyhammer-kanban` passes

## Workflow
- Use `/tdd` — write failing tests first, then implement.

#task-dates