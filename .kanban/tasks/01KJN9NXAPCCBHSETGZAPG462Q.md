---
title: Fix test_tag_counts and InitBoard column ID response
position:
  column: done
  ordinal: c0
---
**Part 1 of the YAML/MD storage conversion plan.**

Fix two related issues in the board/get test and InitBoard:

1. **test_tag_counts** (`board/get.rs` ~line 424): Capture actual tag ULIDs from `AddTag` results instead of searching by name "bug". Use `bug_result["id"].as_str()` to get the ULID and find tags by ID in assertions.

2. **InitBoard::execute**: Currently serializes `Board::default_columns()` via `serde_json::to_value` which omits `id` due to `#[serde(skip)]`. Inject column IDs into the init response the same way other operations do.

**Files:**
- `swissarmyhammer-kanban/src/board/get.rs` (test fix)
- `swissarmyhammer-kanban/src/board/init.rs` (InitBoard response fix)

- [ ] Capture tag ULIDs from AddTag results in test_tag_counts
- [ ] Use ULID-based lookups in tag assertions
- [ ] Fix InitBoard to inject column IDs into response
- [ ] Run `cargo nextest run -p swissarmyhammer-kanban`