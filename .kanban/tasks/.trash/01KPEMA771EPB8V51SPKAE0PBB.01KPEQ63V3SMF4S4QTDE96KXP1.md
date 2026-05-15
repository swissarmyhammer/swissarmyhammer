---
assignees:
- claude-code
depends_on: []
position_column: todo
position_ordinal: d280
title: 'Commands: remove scope:"entity:task" pins from cross-cutting commands in entity.yaml'
---
## What

`swissarmyhammer-commands/builtin/commands/entity.yaml` declares cross-cutting commands with `scope: "entity:task"` — which limits the registry-level availability to tasks and forces every other entity YAML to re-declare the same commands just to surface them. Removing the scope pin lets a single declaration serve every entity that references it.

### Entries to un-pin

In `entity.yaml`, remove `scope: "entity:task"` from:

- `entity.archive`
- `entity.unarchive`
- `entity.cut`
- `entity.copy`
- `entity.paste` (note: `entity.paste` currently has no `scope` — verify)

Availability is already enforced at the Rust level:

- `ArchiveEntityCmd::available` — checks `target` is set and is NOT an archive moniker
- `UnarchiveEntityCmd::available` — checks target ends with `:archive`
- `CopyTaskCmd` / `CutTaskCmd` / `PasteTaskCmd` in `swissarmyhammer-kanban/src/commands/clipboard_commands.rs` — review these; they are named *Task* but should be generic now. If they only handle task today, split the concern: this card **only** changes YAML scope pins; a follow-up card handles the Rust impl generalization if needed.

### Files to touch

- `swissarmyhammer-commands/builtin/commands/entity.yaml` — remove the five `scope:` lines.
- `swissarmyhammer-kanban/src/commands/clipboard_commands.rs` — audit (no code changes in this card; open a follow-up if cut/copy/paste are task-only today).

### Subtasks

- [ ] Strip `scope: "entity:task"` from `entity.archive`, `entity.unarchive`, `entity.cut`, `entity.copy` in `entity.yaml`.
- [ ] Verify `entity.paste` does not have a scope pin (it should already be un-pinned).
- [ ] Audit `clipboard_commands.rs` — if any handler hard-codes task-only behavior, file a follow-up card noting which entities fail copy/cut/paste today.

## Acceptance Criteria

- [ ] `grep -n 'scope:' swissarmyhammer-commands/builtin/commands/entity.yaml` returns nothing.
- [ ] `register_commands_returns_expected_count` test still passes.
- [ ] All existing `archive_entity_*`, `unarchive_entity_*`, `cut_*`, `copy_*`, `paste_*` tests in `swissarmyhammer-kanban/src/commands/` still pass.

## Tests

- [ ] Add `entity_archive_surfaces_on_non_task_entity` in `swissarmyhammer-kanban/src/scope_commands.rs` tests: build a scope with `tag:01X` and assert `entity.archive` appears with `available: true` in `commands_for_scope` output — even before `tag.yaml` entity schema is cleaned up (the declaration alone should be enough once scope pin is gone and the tag schema references it).
- [ ] Run command: `cargo nextest run -p swissarmyhammer-kanban scope_commands` — all tests green.

## Workflow

- Use `/tdd` — the `entity_archive_surfaces_on_non_task_entity` test probably fails on the current branch because of the scope pin; fix the YAML to make it pass.

#commands

Depends on: 01KPEM93Z47JSME10BY1JBGTFM (entity.yaml must be cleaned of type-specific commands first to minimize diff conflicts)