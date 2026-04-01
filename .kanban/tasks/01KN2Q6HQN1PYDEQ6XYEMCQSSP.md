---
assignees:
- claude-code
depends_on:
- 01KN2Q67HEFFEM63KJ5V006ASD
position_column: done
position_ordinal: ffffffffffffffffab80
title: 'PERSP-5: Dispatch integration and Noun/Verb wiring'
---
## What

Wire perspective operations into the existing dispatch system.

**`swissarmyhammer-kanban/src/types/operation.rs`:**
- Add `Perspective` and `Perspectives` to `Noun` enum
- Add `"perspective"` and `"perspectives"` to `as_str()` and `parse()`
- Add valid `(Verb, Noun)` combos to `is_valid_operation()`: Add/Get/Update/Delete Perspective, List Perspectives

**`swissarmyhammer-kanban/src/dispatch.rs`:**
- Import all 5 perspective operation structs
- Add match arms for `(Verb::Add, Noun::Perspective)`, `(Verb::Get, Noun::Perspective)`, `(Verb::Update, Noun::Perspective)`, `(Verb::Delete, Noun::Perspective)`, `(Verb::List, Noun::Perspectives)`

**`swissarmyhammer-kanban/src/schema.rs`:**
- Register perspective operations in `KANBAN_OPERATIONS`
- Add perspective examples to schema generation

**`swissarmyhammer-kanban/src/lib.rs`:**
- Add `pub mod perspective;` and re-exports

## Acceptance Criteria
- [x] `Noun::parse("perspective")` and `Noun::parse("perspectives")` work
- [x] All 5 (Verb, Noun) combos are valid
- [x] `execute_operation()` dispatches all 5 perspective ops
- [x] Schema `op` enum includes perspective operations
- [x] End-to-end: add → get → update → list → delete through dispatch

## Tests
- [x] `test_noun_parsing_perspective` — singular and plural
- [x] `test_valid_perspective_operations` — all 5 combos
- [x] `dispatch_add_perspective` — through execute_operation
- [x] `dispatch_get_perspective` — by ID
- [x] `dispatch_list_perspectives`
- [x] `dispatch_update_perspective`
- [x] `dispatch_delete_perspective`
- [x] `dispatch_perspective_full_lifecycle` — add → get → update → list → delete
- [x] Run: `cargo test -p swissarmyhammer-kanban`