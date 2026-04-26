---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffff8f80
title: Remove EntityFieldChanged.fields enrichment — use watcher diffs for all entity changes
---
## What

Remove the `fields: Option<HashMap>` full-state enrichment from `EntityFieldChanged` and the `EntityContext.read()` call in `flush_and_emit_for_handle`. Instead, let the watcher's `diff_fields` produce field-level `FieldChange(field, value)` diffs for ALL entity changes — both store-written and externally-edited files.

### Architecture rule (from memory: event-architecture)

Events have exactly two granularities:
- **Entity-level**: `(entity_type, id, event_kind)` — created, removed
- **Field-level**: `(entity_type, id, changes: Vec<FieldChange>)` — one entry per changed field, each carrying the new value

No full-state enrichment. No re-fetch round-trips.

### Current problem

`flush_and_emit_for_handle` in `kanban-app/src/commands.rs` (~line 1462-1496):
1. Drains store events via `flush_all()` — gets `(store, id, event_name)`
2. Calls `EntityContext.read()` to enrich with full fields — **WRONG, violates architecture**
3. Discards watcher entity events (line 1498) — **WRONG, the watcher produces the correct diffs**

### Fix

In `flush_and_emit_for_handle`:
1. Call `watcher::flush_and_emit()` FIRST — it scans disk and produces `EntityFieldChanged` with `changes: Vec<FieldChange>` from `diff_fields`
2. Drain store events via `flush_all()` — these tell us which entities were written by commands
3. For `item-created` store events: emit `EntityCreated` (frontend will `get_entity` once)
4. For `item-removed` store events: emit `EntityRemoved`
5. For `item-changed` store events: the watcher already detected the file change and produced `FieldChange` diffs. Use the watcher event. If the watcher didn't produce a diff (hash unchanged = idempotent write), skip — nothing actually changed.
6. Pass through ALL watcher events (not just attachments) — remove the attachment-only filter
7. Deduplicate: if both store and watcher report the same `(entity_type, id)` change, prefer the watcher event (it has the field diffs)

### Files to modify

1. **`kanban-app/src/commands.rs`** (`flush_and_emit_for_handle`, ~line 1416)
   - Remove `EntityContext.read()` enrichment (line ~1462-1470)
   - Remove attachment-only filter on watcher events (line ~1498-1503)
   - Add dedup logic: store `item-changed` events are covered by watcher `EntityFieldChanged`
   - Keep store `item-created` and `item-removed` as-is (watcher handles these too, but store is authoritative for timing)

2. **`kanban-app/src/watcher.rs`** (`WatchEvent::EntityFieldChanged`)
   - Remove `fields: Option<HashMap<String, serde_json::Value>>` from `EntityFieldChanged` — it's the enrichment field, no longer needed
   - Update all construction sites and serialization

### Comment to add

Add a doc comment block at the top of `flush_and_emit_for_handle` explaining the architecture:
```
/// Events are thin signals with two granularities:
/// - Entity-level: (entity_type, id) for created/removed
/// - Field-level: (entity_type, id, field, value) for changes
///
/// The watcher produces field-level diffs by comparing file content hashes
/// and running diff_fields. Store events tell us WHICH entities were written;
/// the watcher tells us WHAT changed. We never read entities back to enrich events.
```

## Acceptance Criteria

- [ ] `flush_and_emit_for_handle` does NOT call `EntityContext.read()` or any entity read for enrichment
- [ ] `EntityFieldChanged` no longer has a `fields` option — only `changes: Vec<FieldChange>`
- [ ] Watcher entity events are NOT filtered out (attachment-only filter removed)
- [ ] Store `item-changed` events are deduplicated against watcher events
- [ ] `cargo test -p kanban-app` passes
- [ ] `cargo test --workspace` passes

## Tests

- [ ] `kanban-app/src/commands.rs` — update `test_store_event_extraction` to verify no enrichment read happens
- [ ] `kanban-app/src/watcher.rs` — verify `EntityFieldChanged` no longer has `fields` option
- [ ] Run `cargo test --workspace` — all pass

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass. #events