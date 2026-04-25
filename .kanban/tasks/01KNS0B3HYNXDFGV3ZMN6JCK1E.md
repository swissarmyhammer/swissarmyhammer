---
assignees:
- claude-code
depends_on:
- 01KNQXXF5W7G4JP73C6ZCMKYKX
- 01KQ2E7RPBPJ8T8KZX39N2SZ0A
position_column: todo
position_ordinal: a880
project: spatial-nav
title: 'Dynamic FocusScope lifecycle: zone-aware fallback, virtualization, batch (newtype signatures)'
---
## What

Handle edge cases where focusables and zones mount/unmount dynamically — virtualized lists, deleted entities, inspector field changes. The core registry (cards `01KNQXW7HH...` and `01KNQXXF5W...`) handles the happy path. This card covers the hard cases, respecting both the layer boundary and the zone tree, with **every struct / argument newtyped**.

### Crate placement

Per the commit-`b81336d42` refactor pattern:
- Fallback resolution logic in `swissarmyhammer-focus/src/state.rs` (`SpatialState::handle_unregister`)
- `FallbackResolution` enum in `swissarmyhammer-focus/src/state.rs`
- `RegisterEntry` enum + `spatial_register_batch` command: type in `focus/registry.rs`, Tauri adapter in `kanban-app/src/commands.rs`
- Virtualization placeholder wiring is React-only (`kanban-app/ui/src/components/column-view.tsx`)
- Tests in `swissarmyhammer-focus/tests/fallback.rs` and `swissarmyhammer-focus/tests/batch_register.rs`

### Case 1: Focused scope unmounts (zone-aware fallback)

When the focused entry's scope unmounts and calls `spatial_unregister_scope(SpatialKey)`, the registry has no origin rect. The state manager computes a fallback by walking outward through the zone tree, then the layer tree:

1. **Sibling in same zone.** Pick the nearest remaining entry where `candidate.parent_zone() == lost.parent_zone()` in the same `LayerKey`. Prefer matching variant (Leaf→Leaf, Zone→Zone) if possible.
2. **Walk up parent zones.** If the lost entry's zone is now empty, move to `lost.parent_zone()`. If that zone's `last_focused: Option<SpatialKey>` is still registered, use it. Otherwise pick the nearest entry in that zone.
3. **Walk up to layer root.** Keep walking `parent_zone` until reaching `None`.
4. **Walk up layer tree.** If the layer root has no remaining entries, walk `layer.parent`. Use that layer's `last_focused: Option<SpatialKey>` if valid.
5. **No-focus.** Emit `FocusChangedEvent { next_key: None, next_moniker: None }`.

Fallback never returns an entry whose `window_label: WindowLabel` differs from the lost entry's window.

Internal return type (all variants carry typed data):

```rust
pub enum FallbackResolution {
    Found(SpatialKey, Moniker),
    FallbackSiblingInZone(SpatialKey, Moniker),
    FallbackParentZoneLastFocused(SpatialKey, Moniker),
    FallbackParentZoneNearest(SpatialKey, Moniker),
    FallbackParentLayer(SpatialKey, Moniker),
    NoFocus,
}
```

### Case 2: Virtualized lists

Only visible cards have mounted primitives. The virtualizer registers **estimated rects** for off-screen items using `spatial_register_batch` — all entries using the same `LayerKey` / `parent_zone` (`Option<SpatialKey>`) as the virtualizer's enclosing zone. Placeholders register as `FocusScope::Zone` or `FocusScope::Focusable` matching the shape of the real mount.

When nav lands on a placeholder:
1. Rust returns `Option<Moniker>` pointing at the placeholder
2. React's `FocusChangedEvent` handler calls `setFocus(moniker)` → virtualizer scrolls-to-item
3. The real primitive mounts with the **same `SpatialKey`** (virtualizer generates and threads it as a prop) — registration is idempotent on key; rect is overwritten, hierarchy preserved

So `spatial_register_focusable` and `spatial_register_zone` are both idempotent on `SpatialKey`. Whoever registers last wins the rect; `kind`/`layer_key`/`parent_zone` must match.

### Case 3: Batch registration

Twenty simultaneous mounts → one Tauri invoke. The batch entry uses newtypes throughout:

```rust
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RegisterEntry {
    Focusable {
        key: SpatialKey,
        moniker: Moniker,
        rect: Rect,
        layer_key: LayerKey,
        parent_zone: Option<SpatialKey>,
        overrides: HashMap<Direction, Option<Moniker>>,
    },
    Zone {
        key: SpatialKey,
        moniker: Moniker,
        rect: Rect,
        layer_key: LayerKey,
        parent_zone: Option<SpatialKey>,
        overrides: HashMap<Direction, Option<Moniker>>,
    },
}

#[tauri::command]
async fn spatial_register_batch(
    window: tauri::Window,
    entries: Vec<RegisterEntry>,
) -> Result<()>;
```

Single lock on the registry, one iteration over `entries`.

### Subtasks
- [ ] Implement zone-aware fallback walking `parent_zone` then `layer.parent`, returning a `FallbackResolution` with typed fields
- [ ] Emit `FocusChangedEvent` with typed fields based on the resolution
- [ ] `VirtualizedCardList` reads `parent_zone` from `FocusZoneContext` and `layer_key` from `FocusLayerContext`; generates stable branded `SpatialKey` per index
- [ ] Add `spatial_register_batch` with `Vec<RegisterEntry>` — newtyped throughout
- [ ] Real-mount reuses placeholder's `SpatialKey`; registry overwrites rect, keeps hierarchy

## Acceptance Criteria
- [ ] `RegisterEntry` enum uses newtypes for every field; no bare `String` / `f64`
- [ ] `FallbackResolution` variants carry typed `SpatialKey` / `Moniker`; none are raw strings
- [ ] Deleting the focused entry restores focus within the same zone first, then walks up the zone chain, then the layer chain
- [ ] Fallback never crosses `WindowLabel` boundaries
- [ ] Deleting the sole entry in a zone falls back to the parent zone's `last_focused` if valid
- [ ] Deleting the sole entry in a window root with no parent layer → `focus_by_window[WindowLabel]` cleared; `FocusChangedEvent { next_key: None, next_moniker: None }`
- [ ] `nav.down` past the last visible card in a virtualized list scrolls to and focuses the next card
- [ ] Placeholders inherit `LayerKey` and `parent_zone` from enclosing zone
- [ ] Batch registration is atomic (single lock)
- [ ] `cargo test` and `pnpm vitest run` pass

## Tests
- [ ] Rust: fallback returns `FallbackSiblingInZone(SpatialKey, Moniker)` with typed values
- [ ] Rust: fallback returns `FallbackParentZoneLastFocused` when zone empties but parent has live `last_focused`
- [ ] Rust: fallback returns `FallbackParentLayer` walking up `layer.parent`
- [ ] Rust: fallback returns `NoFocus` at a lone window root
- [ ] Rust: fallback never returns an entry with a different `WindowLabel`
- [ ] Rust: `spatial_register_focusable` with existing key overwrites rect; type stays `Focusable`
- [ ] Rust: `spatial_register_zone` called for a key previously registered as `Focusable` is an error (kind must match)
- [ ] Rust: `spatial_register_batch` deserializes `RegisterEntry` enum via `"kind"` tag; registers N entries under one lock
- [ ] Integration: nav.down past visible area scrolls and focuses next card
- [ ] Run `cargo test` and `cd kanban-app/ui && npx vitest run` — all pass

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.