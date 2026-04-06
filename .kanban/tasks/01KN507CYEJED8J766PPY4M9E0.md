---
assignees:
- claude-code
depends_on:
- 01KN506PPQ09AQ2TW9FDW2V85Z
position_column: done
position_ordinal: ffffffffffffffffef80
title: Wire entity writes through StoreHandle
---
## What

`EntityContext::write()` and `delete()` delegate to `StoreHandle<EntityTypeStore>` instead of doing their own file I/O and changelog. The old per-entity changelog continues for activity history. The new store-level `changelog.jsonl` becomes the undo source of truth.

**Files to modify:**
- `swissarmyhammer-entity/src/context.rs` — `write()` calls `validate_for_write()` then `store_handle.write()`. `delete()` calls `store_handle.delete()`.
- `swissarmyhammer-kanban/src/context.rs` or wherever `EntityContext` is constructed — pass `StoreHandle` references per entity type

**Approach:**
- `EntityContext` gains a map of `entity_type → Arc<StoreHandle<EntityTypeStore>>`
- `write()`: validate → serialize via store → store_handle.write() → return UndoEntryId
- `delete()`: store_handle.delete() → return UndoEntryId
- Old `push_undo_stack()` in EntityContext is NOT removed yet (that's the next card)
- The old per-entity `.jsonl` changelogs can stay — they serve the activity feed
- The new `changelog.jsonl` per entity type directory is the undo source

**What stays in EntityContext:**
- In-memory entity registry
- Schema, field definitions
- Validation engine (called before write)
- Computed field derivation (called on read)

## Acceptance Criteria
- [ ] `EntityContext::write()` delegates to StoreHandle
- [ ] `EntityContext::delete()` delegates to StoreHandle
- [ ] Files written via atomic temp-file pattern from StoreHandle
- [ ] New `changelog.jsonl` created per entity type directory
- [ ] Old per-entity `.jsonl` still works for activity
- [ ] All existing tests pass

## Tests
- [ ] `cargo nextest run -E 'rdeps(swissarmyhammer-entity)'` — all pass
- [ ] `cargo nextest run -E 'rdeps(swissarmyhammer-kanban)'` — all pass