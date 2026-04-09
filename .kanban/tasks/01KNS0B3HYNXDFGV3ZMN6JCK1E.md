---
assignees:
- claude-code
depends_on:
- 01KNQXXF5W7G4JP73C6ZCMKYKX
position_column: todo
position_ordinal: a880
project: spatial-nav
title: 'Handle dynamic FocusScope lifecycle: unmount, virtualization, batch registration'
---
## What

Handle edge cases where FocusScopes mount/unmount dynamically — virtualized lists, deleted entities, inspector field changes. The core spatial registry from cards 1-2 handles the happy path (mount → register, unmount → unregister). This card handles the hard cases.

### Case 1: Focused scope unmounts

When the focused entity is deleted or scrolled out of the virtual viewport, its FocusScope unmounts and calls `spatial_unregister`. Now `navigate()` can't find the origin rect.

**Solution**: `navigate()` checks if the focused moniker exists in the registry. If not, it falls back:
- Return `First` direction result (top-left-most in active layer) as the new focus target
- React detects the stale focus via the response and calls `setFocus(fallback)`

Add a `navigate()` return type that distinguishes `Found(moniker)` from `FallbackToFirst(moniker)` so React can log the recovery.

### Case 2: Virtualized lists

Only visible cards have mounted FocusScopes. `nav.down` past the last visible card finds nothing below in the registry. The virtualizer must scroll to reveal the next card before spatial nav can resolve.

**Solution — placeholder rects**: The virtualizer registers **estimated rects** for off-screen items via `spatial_register(moniker, estimated_rect, layer)`. These are computed from item index × estimated height. When nav lands on a placeholder:
1. Rust returns the placeholder's moniker
2. React calls `setFocus(moniker)` which tells the virtualizer to scroll-to-item
3. Virtualizer scrolls, real FocusScope mounts, calls `spatial_register` with measured rect (overwrites the estimate)

This means `spatial_register` can be called by both FocusScope (measured) and the virtualizer (estimated). The Rust side doesn't distinguish — it's just a rect.

**Where to register placeholders**: `VirtualizedCardList` in `column-view.tsx` knows all task monikers and estimated positions. On mount, it registers estimated rects for all items (not just visible ones). On unmount of the list, it unregisters all. The real FocusScope mounts overwrite estimates with measurements.

### Case 3: Batch registration

When a virtualizer reveals 20 cards or the board first renders, many FocusScopes mount simultaneously. Each calls `spatial_register` individually — 20 Tauri invokes.

**Solution**: Add `spatial_register_batch(entries: Vec<(moniker, rect, layer)>)` Tauri command. FocusScope can still use the single-entry version. The virtualizer uses batch for placeholder registration.

### Subtasks
- [ ] `navigate()` handles missing focused moniker — falls back to First in active layer
- [ ] Add placeholder rect registration from VirtualizedCardList for off-screen items
- [ ] Add `spatial_register_batch` Tauri command for bulk registration
- [ ] Handle scroll-to-focus flow: nav lands on placeholder → virtualizer scrolls → real mount overwrites
- [ ] Add tests for all three cases

## Acceptance Criteria
- [ ] Deleting the focused entity doesn't break navigation — focus moves to a sensible fallback
- [ ] `nav.down` past the last visible card in a virtualized list scrolls to and focuses the next card
- [ ] Batch registration works for virtualizer placeholder setup
- [ ] No stale entries left in registry after component unmount
- [ ] `cargo test` passes, `pnpm vitest run` passes

## Tests
- [ ] `Rust unit tests` — navigate with missing focused moniker returns fallback
- [ ] `Rust unit tests` — navigate returns placeholder moniker when it's the nearest match
- [ ] `Rust unit tests` — register overwrites existing entry (placeholder → measured)
- [ ] `Rust unit tests` — register_batch adds multiple entries atomically
- [ ] `column-view.test.tsx` or integration — nav.down past visible area scrolls and focuses next card
- [ ] Run `cargo test` and `cd kanban-app/ui && npx vitest run` — all pass

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.