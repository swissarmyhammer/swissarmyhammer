---
assignees:
- claude-code
depends_on:
- 01KPG6W9GCCRNZC81C4Z92QTNA
- 01KPEMYJV7BMTJB6GZ8MGTD04J
- 01KPG5XK61ND4JKXW3FCM3CC97
- 01KPG5YB7GTQ6Q3CEQAMXPJ58F
- 01KPG6GN9JQSCZKFER5ZJ5JC62
position_column: todo
position_ordinal: e880
title: 'Commands: project.yaml cleanup — purge cross-cutting opt-ins'
---
## What

Clean up `swissarmyhammer-kanban/builtin/entities/project.yaml`: no type-specific commands to migrate (project.delete retires in F via auto-emit entity.delete); just purge cross-cutting opt-ins.

### Moves IN

None. Project has no type-specific commands after `project.delete` retires.

### Moves OUT

Delete these:

- `ui.inspect`
- `entity.archive`
- `project.delete` (this command itself retires — F completes the Rust cleanup)

### Files to touch

- `swissarmyhammer-kanban/builtin/entities/project.yaml` — empty the `commands:` list (or remove the key if the schema loader tolerates absence).

### Subtasks

- [ ] Delete cross-cutting opt-ins.
- [ ] Delete `project.delete` entry (Rust retirement is F's job).
- [ ] Hygiene test green for project.yaml.

## Acceptance Criteria

- [ ] `project.yaml`'s `commands:` is empty or absent.
- [ ] Right-click on a project shows Inspect Project (via auto-emit ui.inspect), Delete Project (via auto-emit entity.delete), Archive Project, Unarchive Project.
- [ ] Hygiene test green for project.yaml.

## Tests

- [ ] Add `entity_delete_surfaces_on_project_via_autoemit` — scope `["project:backend"]`, target `"project:backend"`; context_menu emission contains `entity.delete` with `available: true`.
- [ ] Run command: `cargo nextest run -p swissarmyhammer-kanban scope_commands project` — all green.

## Workflow

- Use `/tdd`.

#commands

Depends on: 01KPG6W9GCCRNZC81C4Z92QTNA, 01KPEMYJV7BMTJB6GZ8MGTD04J, 01KPG5XK61ND4JKXW3FCM3CC97, 01KPG5YB7GTQ6Q3CEQAMXPJ58F, 01KPG6GN9JQSCZKFER5ZJ5JC62 (task→project paste handler)