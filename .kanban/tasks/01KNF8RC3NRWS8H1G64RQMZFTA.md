---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffff8780
title: Define Project entity and field YAMLs
---
## What

Create the Project entity definition and a `project` reference field on tasks. Do NOT delete swimlane YAML yet — that happens in Card 3 alongside the Rust code removal.

### Files to create:
- **Create** `swissarmyhammer-kanban/builtin/fields/entities/project.yaml` — new entity with fields: `name`, `description`, `color`, `order`
- **Create** `swissarmyhammer-kanban/builtin/fields/definitions/project.yaml` — single-select reference field (`type.kind: reference`, `entity: project`, `multiple: false`, `groupable: true`, `editor: select`, `display: badge`)

### Files to modify:
- **Modify** `swissarmyhammer-kanban/builtin/fields/entities/task.yaml` — ADD `project` to the fields list (keep `position_swimlane` for now — removed in Card 3)

### Entity design:
```yaml
# project.yaml entity
name: project
icon: folder
search_display_field: name
commands:
  - id: ui.inspect
    name: "Inspect {{entity.type}}"
    context_menu: true
fields:
  - name
  - description
  - color
  - order
```

### Field design:
```yaml
# project.yaml field definition
id: "00000000000000000000000010"
name: project
description: Project this task belongs to
type:
  kind: reference
  entity: project
  multiple: false
icon: folder
editor: select
display: badge
width: 150
section: header
groupable: true
```

### Notes:
- The `project` field is NOT a position component — it's a regular reference field like `assignees`
- `search_display_field: name` on the project entity enables group header display
- Do NOT delete swimlane YAML here — Card 3 handles that with the Rust code removal

## Acceptance Criteria
- [ ] `project.yaml` entity exists with name, description, color, order fields
- [ ] `project.yaml` field definition exists as single-select reference with `groupable: true`
- [ ] `task.yaml` includes both `project` and `position_swimlane` (transitional)
- [ ] Rust compilation succeeds
- [ ] `cargo test -p swissarmyhammer-fields` passes

## Tests
- [ ] Existing field validation tests still pass
- [ ] `cargo test -p swissarmyhammer-fields` passes

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.