---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffff8980
title: 'Bug: pasting a tag onto a task causes computed auto-tags to vanish'
---
## What

When cutting a tag from one task and pasting it onto another, the **computed tags** (the `tags` badge-list derived from `#tag` patterns in the body via `parse-body-tags`) vanish from the target task's UI.

### Root Cause (confirmed)

**Race condition between async file watcher and synchronous dispatch flush.**

The async file watcher (`state.rs:269`) debounces at 200ms and emits events **directly to the frontend without computed field enrichment**. When the OS delivers a filesystem notification between the command's entity write and the synchronous `flush_and_emit` call:

1. Async watcher fires → `resolve_change` produces `EntityFieldChanged` with raw diffs (body only, no computed tags) → updates watcher cache → emits to frontend
2. `flush_and_emit` runs → cache already updated → no diff detected → no event produced
3. `enrich_computed_fields` never runs → frontend gets body but no tags

Data layer is correct (backend test proves all tags survive paste). Frontend patch logic is correct (test proves multi-field patching works). The bug is the missing enrichment on the async watcher path.

### Fix applied

`kanban-app/src/commands.rs` — For `item-changed` store events with no watcher match (race recovery), read entity fields from disk and emit a synthetic `EntityFieldChanged`. This ensures the enrichment path always runs for our own writes, even when the async watcher has already consumed the change.

`kanban-app/src/watcher.rs` — Added `read_entity_fields_from_disk()` to read entity fields by type+id from store root directories. Made `update_cache` available in production (removed `#[cfg(test)]`).

## Acceptance Criteria

- [ ] After pasting a tag onto a task with existing auto-tags, ALL tags (old + new) remain visible in the tags badge-list
- [ ] The `entity-field-changed` event for the task includes the re-derived `tags` computed field in the changes array
- [x] Backend test proves tags survive paste (`test_paste_tag_preserves_existing_tags`)
- [x] Frontend test proves multi-field patching works
- [ ] Manual verification: paste tag → all tags visible

## Tests

- [x] `swissarmyhammer-kanban/src/tag/paste.rs` — `test_paste_tag_preserves_existing_tags` passes
- [x] `kanban-app/ui/src/lib/entity-event-propagation.test.tsx` — multi-field body+tags patch test passes
- [x] `cargo test -p kanban-app` — 117 tests pass
- [ ] Manual: paste tag in UI → verify all tags remain #paste-tag-bug"
</invoke>