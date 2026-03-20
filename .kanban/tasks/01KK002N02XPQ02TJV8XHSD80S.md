---
position_column: done
position_ordinal: ffffa580
title: File watcher for concurrent access with hash-based change detection
---
Watch the `.kanban/` directory for external file changes and feed them through the same event system as user actions.

## Scope

- Use `notify` crate to watch `.kanban/` directory recursively
- On file change: parse entity path to (entity_type, id), read file, hash content
- Compare hash to cached value — skip if unchanged
- If changed: update `EntityCache`, emit `entity-changed` event
- If new file: add to cache, emit `entity-changed`
- If deleted: remove from cache, emit `entity-deleted`
- Debounce rapid changes (e.g., editor save-then-rename patterns)
- Ignore changes to `.jsonl` changelog files (only watch entity data files)
- Start watcher on board open, stop on board close

## Testing

- Test: external file modification triggers cache update and event
- Test: external file creation triggers cache add and event  
- Test: external file deletion triggers cache remove and event
- Test: file touch without content change (same hash) triggers no event
- Test: rapid successive changes are debounced into single event
- Test: changelog file changes are ignored
- Test: watcher starts/stops cleanly with board lifecycle