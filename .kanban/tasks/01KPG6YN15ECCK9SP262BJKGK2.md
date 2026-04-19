---
assignees:
- claude-code
depends_on:
- 01KPG6W9GCCRNZC81C4Z92QTNA
- 01KPEMYJV7BMTJB6GZ8MGTD04J
- 01KPG6HF1ZHWZ981PS3BEPP1HE
position_column: done
position_ordinal: fffffffffffffffffffffff780
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

- [x] Delete the `ui.inspect` entry. (already absent on disk — no `commands:` key present in actor.yaml)
- [x] Remove the now-empty `commands:` key. (already absent)
- [x] Hygiene test green for actor.yaml. (verified — `actor.yaml` no longer appears in `yaml_hygiene_no_cross_cutting_in_entity_schemas` violations list)

## Acceptance Criteria

- [x] `actor.yaml` has no `commands:` key.
- [x] Right-click on an actor shows Inspect Actor (via auto-emit) and Paste onto Actor (via auto-emit entity.paste when applicable; likely unavailable since actor→task only works the other direction).
- [x] Hygiene test green for actor.yaml.

## Tests

- [x] Add `ui_inspect_auto_emits_on_actor_without_opt_in` — after the YAML entry is removed, scope `["actor:alice"]` still produces `ui.inspect` via auto-emit.
- [x] Run command: `cargo nextest run -p swissarmyhammer-kanban scope_commands actor` — all green (both `actor_scope_has_inspect` and `ui_inspect_auto_emits_on_actor_without_opt_in` pass).

## Workflow

- Use `/tdd`.

#commands

Depends on: 01KPG6W9GCCRNZC81C4Z92QTNA, 01KPEMYJV7BMTJB6GZ8MGTD04J, 01KPG6HF1ZHWZ981PS3BEPP1HE (actor→task paste handler)

## Implementation Notes

- `actor.yaml` already had no `commands:` key on disk before this task started — likely cleaned up as part of dependency 01KPG6W9GCCRNZC81C4Z92QTNA. The hygiene-test "GREEN" milestone for actor.yaml was already de facto met; this task formalizes it with a dedicated test that asserts both:
  1. The YAML has no `commands:` opt-in (premise guard via `EntityDef::commands.is_empty()`).
  2. `commands_for_scope(["actor:alice"])` still emits `ui.inspect` with the correct target/context_menu/available flags via the cross-cutting auto-emit pass.
- New test added in `swissarmyhammer-kanban/src/scope_commands.rs` immediately after `actor_scope_has_inspect`, in the "Other entity types (actor, attachment)" section.