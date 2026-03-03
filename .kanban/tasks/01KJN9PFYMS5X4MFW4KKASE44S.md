---
title: Convert non-task entity storage from JSON to YAML
position:
  column: done
  ordinal: c2
---
**Part 2b of the YAML/MD storage conversion plan.**

Convert all non-task entity files from `.json` to `.yaml` using `serde_yaml`.

**Path method changes** in `context.rs`:
- `board_path()` → `board.yaml` (was `board.json`)
- `actor_path()` → `actors/{id}.yaml`
- `tag_path()` → `tags/{id}.yaml`
- `column_path()` → `columns/{id}.yaml`
- `swimlane_path()` → `swimlanes/{id}.yaml`

**Read/write method changes:**
- Replace `serde_json::from_str` → `serde_yaml::from_str` in: read_board, read_actor, read_tag, read_column, read_swimlane
- Replace `serde_json::to_string_pretty` → `serde_yaml::to_string` in: write_board, write_actor, write_tag, write_column, write_swimlane

**List method changes:** Update `list_actor_ids`, `list_tag_ids`, `list_column_ids`, `list_swimlane_ids` to accept both `.yaml` and `.json` extensions.

**Initialization:** `is_initialized()` must check for `board.yaml` OR `board.json`. `board/init.rs` writes `board.yaml`.

**Backward compat:** Each `read_*` method falls back to `.json` if `.yaml` doesn't exist. `serde_yaml` can parse JSON, so the same deserializer works.

**Test updates:**
- `tests/integration_tag_storage.rs`: `format!("{}.json", tag_id)` → `format!("{}.yaml", tag_id)`
- `context.rs` tests: Update path assertions

**Files:**
- `swissarmyhammer-kanban/src/context.rs` (all path/read/write/list methods)
- `swissarmyhammer-kanban/src/board/init.rs` (write board.yaml)
- `swissarmyhammer-kanban/tests/integration_tag_storage.rs`

- [ ] Update all 5 entity path methods to .yaml
- [ ] Convert read_board/write_board to serde_yaml
- [ ] Convert read/write for actor, tag, column, swimlane
- [ ] Update all list_*_ids methods for dual extension
- [ ] Update is_initialized() for board.yaml/board.json
- [ ] Update board/init.rs to write board.yaml
- [ ] Fix integration_tag_storage.rs assertions
- [ ] Fix context.rs test assertions
- [ ] Run `cargo nextest run -p swissarmyhammer-kanban`