---
position_column: done
position_ordinal: ffffaa80
title: 'Migrate React to event-driven rendering: replace refresh() with event listeners'
---
Rewrite React's state management to be event-driven. React maintains a local entity mirror updated by Rust events instead of polling via `refresh()`.

## Scope

- Replace `refresh()` in `App.tsx` with per-event listeners:
  - `entity-changed` → update single entity in local `Map<string, Map<string, Entity>>`
  - `entity-deleted` → remove from local map
  - `board-structure-changed` → re-fetch board structure (columns/swimlanes)
  - `inspector-stack` → `setInspectorStack(payload)`
  - `active-view` → `setActiveViewId(payload)`
  - `palette-open` → `setPaletteOpen(payload)`
  - `keymap-mode` → `setKeymapMode(payload)`
- Initial load on board open: fetch all entities once into local map
- Remove `FieldUpdateProvider` (no more `onRefresh` callback)
- Remove `AppModeProvider` (palette/search state comes from Rust events)
- Remove `KeymapProvider` (keymap state comes from Rust events)
- Remove `UndoStackProvider` and `UndoStack` class (undo is Rust-only)
- Remove `ViewsContext` internal state management (active view comes from Rust)
- Simplify `EntityStoreProvider` to be the local entity cache driven by events
- Components subscribe to specific entity types/ids for targeted re-renders

## Testing

- Test: `entity-changed` event updates local cache for correct entity
- Test: `entity-deleted` event removes entity from local cache
- Test: `inspector-stack` event updates panel rendering
- Test: `keymap-mode` event updates keybinding resolution
- Test: initial load populates cache from Rust
- Test: component re-renders only when its subscribed entity changes
- Test: multiple rapid events don't cause render thrashing