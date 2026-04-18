---
assignees:
- claude-code
depends_on:
- 01KPG6W9GCCRNZC81C4Z92QTNA
- 01KPEMYJV7BMTJB6GZ8MGTD04J
- 01KPG6HF1ZHWZ981PS3BEPP1HE
position_column: todo
position_ordinal: eb80
title: 'Commands: actor.yaml cleanup — purge cross-cutting opt-ins'
---
## What

Clean up `swissarmyhammer-kanban/builtin/entities/actor.yaml`: no type-specific commands; just purge the one cross-cutting opt-in.

### Moves IN

None.

### Moves OUT

- `ui.inspect`

### Files to touch

- `swissarmyhammer-kanban/builtin/entities/actor.yaml` — remove `commands:` key entirely (the list becomes empty after purge).

### Subtasks

- [ ] Delete the `ui.inspect` entry.
- [ ] Remove the now-empty `commands:` key.
- [ ] Hygiene test green for actor.yaml.

## Acceptance Criteria

- [ ] `actor.yaml` has no `commands:` key.
- [ ] Right-click on an actor shows Inspect Actor (via auto-emit) and Paste onto Actor (via auto-emit entity.paste when applicable; likely unavailable since actor→task only works the other direction).
- [ ] Hygiene test green for actor.yaml.

## Tests

- [ ] Add `ui_inspect_auto_emits_on_actor_without_opt_in` — after the YAML entry is removed, scope `["actor:alice"]` still produces `ui.inspect` via auto-emit.
- [ ] Run command: `cargo nextest run -p swissarmyhammer-kanban scope_commands actor` — all green.

## Workflow

- Use `/tdd`.

#commands

Depends on: 01KPG6W9GCCRNZC81C4Z92QTNA, 01KPEMYJV7BMTJB6GZ8MGTD04J, 01KPG6HF1ZHWZ981PS3BEPP1HE (actor→task paste handler)