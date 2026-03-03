---
title: Add per-entity JSONL logs for all entity types
position:
  column: done
  ordinal: c3
---
**Part 3 of the YAML/MD storage conversion plan.**

Currently only tasks have per-entity `.jsonl` logs. Extend per-entity logging to tags, actors, columns, swimlanes, and the board.

**context.rs — New path methods:**
- `tag_log_path(id)` → `tags/{id}.jsonl`
- `actor_log_path(id)` → `actors/{id}.jsonl`
- `board_log_path()` → `board.jsonl`
- (column_log_path and swimlane_log_path already exist)

**context.rs — New append methods:**
- `append_tag_log(id, entry)`
- `append_actor_log(id, entry)`
- `append_board_log(entry)`
- `append_column_log(id, entry)` (may already exist)
- `append_swimlane_log(id, entry)` (may already exist)

**processor.rs — Extended write_log:**
Parse the noun from the operation string and route to the appropriate per-entity log:
- "tag" → `append_tag_log`
- "column" → `append_column_log`
- "swimlane" → `append_swimlane_log`
- "actor" → `append_actor_log`
- "board" → `append_board_log`

**Delete cleanup:** Update `delete_tag_file` and `delete_actor_file` to also remove the corresponding `.jsonl` log file.

**Files:**
- `swissarmyhammer-kanban/src/context.rs` (path + append methods)
- `swissarmyhammer-kanban/src/processor.rs` (route logs by entity type)
- `swissarmyhammer-kanban/src/tag/delete.rs` (cleanup .jsonl)
- `swissarmyhammer-kanban/src/actor/delete.rs` or wherever actor delete lives

- [ ] Add tag_log_path, actor_log_path, board_log_path methods
- [ ] Add append_tag_log, append_actor_log, append_board_log methods
- [ ] Verify column/swimlane append methods exist or add them
- [ ] Extend processor.rs to route logs by entity noun
- [ ] Update tag delete to clean up .jsonl
- [ ] Update actor delete to clean up .jsonl
- [ ] Run `cargo nextest run -p swissarmyhammer-kanban`