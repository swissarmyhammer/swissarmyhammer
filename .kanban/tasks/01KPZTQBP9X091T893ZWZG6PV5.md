---
assignees:
- claude-code
position_column: todo
position_ordinal: '9e80'
title: Collapse duplicate board_display_name between kanban-app and swissarmyhammer-kanban
---
## What

Follow-up to PR #40 review on task 01KPZMYFYQ0ZFS00GG0DY7JGQJ (Move build_dynamic_sources out of kanban-app). After that refactor, an identical 6-line `board_display_name` implementation exists in two places:

- `kanban-app/src/commands.rs::board_display_name(&BoardHandle)` — takes a `BoardHandle` from the Tauri side, used by ~4 remaining call sites in the GUI crate.
- `swissarmyhammer-kanban/src/dynamic_sources.rs::board_display_name(&KanbanContext)` — takes a plain context, used only by the headless `gather_boards` helper.

The GUI version could call a public `swissarmyhammer_kanban::board_display_name(&handle.ctx)` and the module-level helper could collapse to a single definition in the kanban crate.

## Acceptance Criteria

- [ ] `swissarmyhammer_kanban::board_display_name` is `pub` (promote from private helper in `dynamic_sources.rs`, or move to a more neutral location).
- [ ] `kanban-app/src/commands.rs::board_display_name` is deleted; its 4 call sites pass `&handle.ctx` to the kanban-crate version.
- [ ] `cargo test -p kanban-app` still passes.
- [ ] `cargo test -p swissarmyhammer-kanban` still passes.
- [ ] No behavior change — the helper reads the board entity's `name` field and nothing else.

## Context

Explicitly deferred from task 01KPZMYFYQ0ZFS00GG0DY7JGQJ because that task's scope was to move the assembly logic, not to rationalise helpers. Now that a non-GUI `board_display_name` exists, this duplication can be eliminated.

#refactor #commands #cleanup
