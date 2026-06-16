---
assignees: []
position_column: todo
position_ordinal: ff80
project: task-card-fields
title: 'Tech debt (review fallout): long functions, String errors, and parser/test-setup duplication in kanban + entity'
---
#tech-debt

## What

Pre-existing tech-debt findings surfaced by the review engine while reviewing the tags⇄body sync fix (`^8q2v2vf`). These target code that fix did **not** author (the fix added `tag_parser::body_tags_value` and ~10 lines in `cross_board`'s tag-strip block; the functions below predate it). Split out per the no-unrelated-refactors rule so the bug fix stays scoped.

## Findings to address
- [ ] `crates/swissarmyhammer-kanban/src/entity/update_field.rs:220` — `async fn setup() -> (TempDir, KanbanContext)` is verbatim-duplicated across ~7 test modules (`update_field.rs`, `attachment/{list,delete,add,update}.rs`, `entity/add.rs`, `task/add.rs`). Extract a shared test helper (e.g. `crates/swissarmyhammer-kanban/src/test_support.rs`) and reuse.
- [ ] `crates/swissarmyhammer-entity/src/cache.rs:398` — `write` (~85 lines) mixes pre-write capture, disk write, hash, change-detection, cache update, invalidation, event emission. Extract change-detection (429-445), cache-update (447-456), event-emission (471-481) helpers.
- [ ] `crates/swissarmyhammer-entity/src/cache.rs:764` — `get_or_load_compute_inputs` (~72 lines). Extract off-lock loading (796-815) and epoch-guarded memoization into helpers.
- [ ] `crates/swissarmyhammer-kanban/src/cross_board.rs:25` — `transfer_task` (~130 lines). Extract `compute_target_ordinal()`, `strip_nonexistent_tags()`, `copy_task_fields()`.
- [ ] `crates/swissarmyhammer-kanban/src/cross_board.rs:28` — `transfer_task` returns `Result<Value, String>`; define a `thiserror` enum (e.g. `TransferError`) so callers can match failure modes.
- [ ] `crates/swissarmyhammer-kanban/src/entity/update_field.rs:73` — `execute` (110+ lines) intertwines schema validation, derive routing, comment-log normalization, normal-field update. Extract `validate_field`, `handle_computed_field`, `handle_comment_log`, `handle_normal_field`.
- [ ] `crates/swissarmyhammer-kanban/src/tag_parser.rs` — `parse_tags`, `remove_tag`, `rename_tag` each re-implement the same byte-scan (line loop + backtick/code-skip + `#tag` boundary match), 5 levels deep. Consolidate into one `scan_markdown_bytes(text, visitor)` (or `find_tag_matches`) used by all three. Also hoist the fenced-code delimiters (` ``` `, `~~~`) into a named constant / `is_fence_marker` helper.

## Acceptance Criteria
- [ ] Each function above is under ~50 lines or justified; shared logic extracted (no behavior change).
- [ ] `transfer_task` returns a typed error.
- [ ] `tag_parser` byte-scan logic exists once; `parse_tags`/`remove_tag`/`rename_tag` delegate to it.
- [ ] Shared test `setup()` helper; per-module copies removed.

## Tests
- [ ] `cargo test -p swissarmyhammer-kanban -p swissarmyhammer-entity` → green (behavior-preserving refactor; existing tests are the guard).
- [ ] `cargo clippy -p swissarmyhammer-kanban -p swissarmyhammer-entity --all-targets -- -D warnings` → clean.

## Notes
- Surfaced during `^8q2v2vf`; independent of it. Behavior-preserving — lean on existing tests.