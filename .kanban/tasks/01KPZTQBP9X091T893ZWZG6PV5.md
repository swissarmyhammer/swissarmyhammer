---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffe380
title: Collapse duplicate board_display_name between kanban-app and swissarmyhammer-kanban
---
## What

Follow-up to PR review on task 01KPZMYFYQ0ZFS00GG0DY7JGQJ (Move build_dynamic_sources out of kanban-app). After that refactor, an identical 6-line `board_display_name` implementation exists in two places:

- `kanban-app/src/commands.rs::board_display_name(&BoardHandle)` — takes a `BoardHandle` from the Tauri side, used by ~4 remaining call sites in the GUI crate.
- `swissarmyhammer-kanban/src/dynamic_sources.rs::board_display_name(&KanbanContext)` — takes a plain context, used only by the headless `gather_boards` helper.

The GUI version could call a public `swissarmyhammer_kanban::board_display_name(&handle.ctx)` and the module-level helper could collapse to a single definition in the kanban crate.

## Acceptance Criteria

- [x] `swissarmyhammer_kanban::board_display_name` is `pub` (promote from private helper in `dynamic_sources.rs`, or move to a more neutral location).
- [x] `kanban-app/src/commands.rs::board_display_name` is deleted; its 4 call sites pass `&handle.ctx` to the kanban-crate version.
- [x] `cargo test -p kanban-app` still passes.
- [x] `cargo test -p swissarmyhammer-kanban` still passes.
- [x] No behavior change — the helper reads the board entity's `name` field and nothing else.

## Resolution

- Promoted `swissarmyhammer_kanban::dynamic_sources::board_display_name` from `async fn` to `pub async fn` and expanded its doc comment.
- Added crate-root re-export `pub use dynamic_sources::board_display_name;` in `swissarmyhammer-kanban/src/lib.rs` so call sites can use the short path `swissarmyhammer_kanban::board_display_name(&handle.ctx)`.
- Deleted `kanban-app/src/commands.rs::board_display_name` (the 10-line helper and its doc comment).
- Updated all 4 call sites in `kanban-app/src/commands.rs` (`list_open_boards`, `apply_board_title`, the board-switch handler, and `refresh_board_window_titles`) to call `swissarmyhammer_kanban::board_display_name(&handle.ctx)`. The argument shape works because `BoardHandle::ctx: Arc<KanbanContext>` derefs to `&KanbanContext`.

Validation:
- `cargo test -p swissarmyhammer-kanban` — 1123 lib + 5 + 45 + 14 + 52 + ... + 14 (test bins) + 1 doctest, all pass.
- `cargo test -p kanban-app` — 111 passed; 0 failed.
- `cargo fmt -p swissarmyhammer-kanban -p kanban-app` — clean (formatter reordered the new `pub use` alphabetically).
- `cargo clippy -p swissarmyhammer-kanban -p kanban-app --all-targets -- -D warnings` — clean, zero warnings.

#refactor #commands #cleanup