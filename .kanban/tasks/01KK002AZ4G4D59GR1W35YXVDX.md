---
position_column: done
position_ordinal: ffffa180
title: 'Granular event system: entity change events with content hashing'
---
Replace React's `refresh()` polling with granular Tauri events emitted by the Rust engine when state changes. Content hashing prevents spurious events.

## Scope

- Define event types: `EntityChanged { entity_type, id, version }`, `EntityDeleted { entity_type, id }`, `BoardStructureChanged`
- After every command execution, determine which entities changed by comparing pre/post hashes in the cache
- Emit individual `entity-changed` events for each changed entity
- Emit `board-structure-changed` when column/swimlane order changes
- UI state events already handled by UIState card — this card covers entity data events
- Remove the `board-changed` blanket event
- Emit events from `EntityCache` write/delete methods, not from individual commands — centralized

## Testing

- Test: writing an entity emits `entity-changed` with correct type, id, version
- Test: deleting an entity emits `entity-deleted`
- Test: writing an entity with same content (same hash) emits no event
- Test: batch operation (e.g., column reorder) emits events for each affected entity
- Test: `BoardStructureChanged` emitted when column order changes
- Test: version numbers are monotonically increasing per entity