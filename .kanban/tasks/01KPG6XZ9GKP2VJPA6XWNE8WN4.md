---
assignees:
- claude-code
depends_on:
- 01KPG6W9GCCRNZC81C4Z92QTNA
- 01KPEMYJV7BMTJB6GZ8MGTD04J
- 01KPG5XK61ND4JKXW3FCM3CC97
- 01KPG5YB7GTQ6Q3CEQAMXPJ58F
- 01KPG6GN9JQSCZKFER5ZJ5JC62
position_column: done
position_ordinal: fffffffffffffffffffffff980
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

- [x] Delete cross-cutting opt-ins.
- [x] Delete `project.delete` entry (Rust retirement is F's job).
- [x] Hygiene test green for project.yaml.

## Acceptance Criteria

- [x] `project.yaml`'s `commands:` is empty or absent.
- [x] Right-click on a project shows Inspect Project (via auto-emit ui.inspect), Delete Project (via auto-emit entity.delete), Archive Project, Unarchive Project.
- [x] Hygiene test green for project.yaml.

## Tests

- [x] Add `entity_delete_surfaces_on_project_via_autoemit` — scope `["project:backend"]`, target `"project:backend"`; context_menu emission contains `entity.delete` with `available: true`.
- [x] Run command: `cargo nextest run -p swissarmyhammer-kanban scope_commands project` — all green.

## Workflow

- Use `/tdd`.

## Implementation note (added during /implement)

Per user-approved scope expansion (Q&A 20260418_204916): the test acceptance for `entity_delete_surfaces_on_project_via_autoemit` required `entity.delete` to actually auto-emit, which it could not because `swissarmyhammer-commands/builtin/commands/entity.yaml` declared it `visible: false` with no `context_menu` opt-in. Flipped to `name: "Delete {{entity.type}}"`, `context_menu: true`, default `visible: true`. This is the registry-side counterpart to retiring per-entity Delete opt-ins and unblocks the F card (01KPEME1897275TKE61EKN6EVX) on the Rust-impl removal step.

The crate-wide `yaml_hygiene_no_cross_cutting_in_entity_schemas` still fails on column.yaml, tag.yaml, task.yaml — those are the other per-entity cleanup cards' jobs. Project.yaml correctly drops from the violations list, satisfying this card's "Hygiene test green for project.yaml" deliverable.

#commands

Depends on: 01KPG6W9GCCRNZC81C4Z92QTNA, 01KPEMYJV7BMTJB6GZ8MGTD04J, 01KPG5XK61ND4JKXW3FCM3CC97, 01KPG5YB7GTQ6Q3CEQAMXPJ58F, 01KPG6GN9JQSCZKFER5ZJ5JC62 (task→project paste handler)