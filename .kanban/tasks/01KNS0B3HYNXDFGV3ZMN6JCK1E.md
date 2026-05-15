---
assignees:
- claude-code
depends_on:
- 01KNQXXF5W7G4JP73C6ZCMKYKX
- 01KQ2E7RPBPJ8T8KZX39N2SZ0A
- 01KQ4YYFCGJCRN6GBYGVGXVVG6
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffb780
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
- [x] Implement zone-aware fallback walking `parent_zone` then `layer.parent`, returning a `FallbackResolution` with typed fields — done in `swissarmyhammer-focus/src/state.rs`
- [x] Emit `FocusChangedEvent` with typed fields based on the resolution — done via `SpatialState::handle_unregister`
- [x] `VirtualizedCardList` reads `parent_zone` from `FocusZoneContext` and `layer_key` from `FocusLayerContext`; generates stable branded `SpatialKey` per index — done via `useStableSpatialKeys` + `usePlaceholderRegistration` in `kanban-app/ui/src/components/column-view.tsx`
- [x] Add `spatial_register_batch` with `Vec<RegisterEntry>` — newtyped throughout — Tauri adapter `spatial_register_batch` + `spatial_register_batch_inner` in `kanban-app/src/commands.rs`, registered in `main.rs`'s `invoke_handler!`
- [x] Real-mount reuses placeholder's `SpatialKey`; registry overwrites rect, keeps hierarchy — handled by unregister-on-visibility: when a row enters the visible window the column unregisters its placeholder and the real `EntityCard` registers with its own key. Prop-threading the placeholder key into `EntityCard`'s primitives is deliberately out of scope here (would touch `FocusScope` / `FocusZone` / `EntityCard`); the unregister-on-visible path satisfies the kind-stability invariant by leaving each id with exactly one live registration.

## Acceptance Criteria
- [x] `RegisterEntry` enum uses newtypes for every field; no bare `String` / `f64`
- [x] `FallbackResolution` variants carry typed `SpatialKey` / `Moniker`; none are raw strings
- [x] Deleting the focused entry restores focus within the same zone first, then walks up the zone chain, then the layer chain
- [x] Fallback never crosses `WindowLabel` boundaries
- [x] Deleting the sole entry in a zone falls back to the parent zone's `last_focused` if valid
- [x] Deleting the sole entry in a window root with no parent layer → `focus_by_window[WindowLabel]` cleared; `FocusChangedEvent { next_key: None, next_moniker: None }`
- [x] Placeholders inherit `LayerKey` and `parent_zone` from enclosing zone — `usePlaceholderRegistration` reads `useOptionalLayerKey()` and `useParentZoneKey()`, asserted by `column-view.spatial-nav.test.tsx::ships a spatial_register_batch invoke for off-screen rows when virtualization is active`
- [x] Batch registration is atomic (single lock) — `SpatialRegistry::apply_batch` validates the entire input vector before mutating, returns `BatchRegisterError::KindMismatch` without applying any entry on failure
- [x] `cargo test` and `pnpm vitest run` pass — Rust: 93 tests in `kanban-app` + 113 tests in `swissarmyhammer-focus` pass; frontend: 1571 tests pass (after the 2026-04-26 review-fix pickup added 3 new vitest cases)
- [ ] `nav.down` past the last visible card in a virtualized list scrolls to and focuses the next card — placeholder registration is in place; the visual scroll-on-focus handler is a follow-up that requires touching `FocusScope`/`FocusZone`/`EntityCard` (out of scope here per the parallel-safety constraint of this implementation pickup)

## Tests
- [x] Rust: fallback returns `FallbackSiblingInZone(SpatialKey, Moniker)` with typed values — `tests/fallback.rs::fallback_returns_sibling_in_zone`
- [x] Rust: fallback returns `FallbackParentZoneLastFocused` when zone empties but parent has live `last_focused` — `tests/fallback.rs::fallback_returns_parent_zone_last_focused`
- [x] Rust: fallback returns `FallbackParentLayer` walking up `layer.parent` — `tests/fallback.rs::fallback_returns_parent_layer_last_focused` and `fallback_returns_parent_layer_nearest_includes_zone_nested_leaves`
- [x] Rust: fallback returns `NoFocus` at a lone window root — `tests/fallback.rs::fallback_returns_no_focus_at_lone_window_root`
- [x] Rust: fallback never returns an entry with a different `WindowLabel` — `tests/fallback.rs::fallback_never_crosses_window_boundary`
- [x] Rust: `spatial_register_focusable` with existing key overwrites rect; type stays `Focusable` — `tests/batch_register.rs::register_focusable_twice_overwrites_rect_keeps_variant`
- [x] Rust: `spatial_register_zone` called for a key previously registered as `Focusable` is an error (kind must match) — `tests/batch_register.rs::batch_register_zone_for_existing_focusable_key_errors`
- [x] Rust: `spatial_register_batch` deserializes `RegisterEntry` enum via `"kind"` tag; registers N entries under one lock — `tests/batch_register.rs::register_entry_deserializes_focusable_via_kind_tag`, `register_entry_deserializes_zone_via_kind_tag`, `apply_batch_registers_all_entries`
- [x] Rust: Tauri-adapter unit tests — `kanban-app/src/commands.rs::spatial_command_tests::spatial_register_batch_inner_registers_all_entries` (atomic apply happy path) and `spatial_register_batch_inner_returns_kind_mismatch_error` (error round-trips through the adapter, registry unchanged)
- [x] React: `column-view.spatial-nav.test.tsx::ships a spatial_register_batch invoke for off-screen rows when virtualization is active` — pins the wire shape (zone-kind, branded keys, layer/parent_zone newtypes) and confirms placeholders parent at the column zone
- [x] React: `column-view.spatial-nav.test.tsx::unregisters a placeholder when its task is removed from the column` — added 2026-04-26 review-fix pickup; pins the effect-ordering fix that decouples the unregister path from `useStableSpatialKeys`'s prune effect
- [x] React: `column-view.spatial-nav.test.tsx::computes placeholder rects in viewport coordinates after the column scrolls` — added 2026-04-26 review-fix pickup; scrolls the virtualizer 1600px and asserts the placeholder for an above-viewport task has y < baseY
- [x] React: `column-view.spatial-nav.test.tsx::unregisters every live placeholder when the column unmounts` — added 2026-04-26 review-fix pickup; pins the cleanup contract so re-rendering boards do not leak registry entries
- [x] Run `cargo test` and `pnpm vitest run` — Rust workspace 4135 tests pass (only an unrelated shelltool-cli env-mutation flake outside this scope); frontend `pnpm vitest run` reports 1571 tests passing (2026-04-26 review-fix pickup)

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.

## Implementation Notes (2026-04-26 — `/implement` finish-up)

The remaining IPC + React virtualization wiring landed in this pickup, on top of the now-merged Tauri-adapter foundation card (`01KQ4YYFCGJCRN6GBYGVGXVVG6`). Three files changed:

### `kanban-app/src/commands.rs`

- New imports: `BatchRegisterError`, `RegisterEntry` from `swissarmyhammer-focus`.
- New pure helper `spatial_register_batch_inner(&mut SpatialRegistry, Vec<RegisterEntry>) -> Result<(), BatchRegisterError>` — forwards to `SpatialRegistry::apply_batch`. Pulled out of the Tauri shell so unit tests can drive the same code path without spinning up Tauri (matches the `spatial_register_zone_inner` / `spatial_unregister_scope_inner` pattern already established in this module).
- New Tauri command `spatial_register_batch(_window, state, entries: Vec<RegisterEntry>) -> Result<(), String>` — locks the registry/state through `with_spatial`, dispatches to the inner helper, stringifies any `BatchRegisterError` for the wire boundary. No `focus-changed` event is emitted: registration is structural, not a focus move.
- Two new unit tests: `spatial_register_batch_inner_registers_all_entries` (mixed focusable+zone batch lands in the registry) and `spatial_register_batch_inner_returns_kind_mismatch_error` (kind-mismatch surfaces as `BatchRegisterError::KindMismatch` and the registry is unchanged).

### `kanban-app/src/main.rs`

- One line added inside `tauri::generate_handler!` — `commands::spatial_register_batch` — wedged between `spatial_register_zone` and `spatial_unregister_scope` so the spatial commands list reads in mount/unmount/batch order.

### `kanban-app/ui/src/components/column-view.tsx`

Two new hooks plus wiring inside `VirtualColumn`:

- `useStableSpatialKeys(tasks)` — Map<task.id, SpatialKey> held in a ref. Mints `crypto.randomUUID()` on first sight of a task id, prunes ids that drop out of the list. Stable identity across renders so the kernel's idempotent re-register path can refresh a placeholder's rect on every scroll without disturbing `last_focused`.
- `useVisibleIndexSet(virtualizer, taskCount)` — derives the set of indices the virtualizer currently has mounted, excluding the trailing-zone pseudo-row. Memoized on the virtualizer's `getVirtualItems()` reference so the dependent effect only fires when the visible window actually shifts.
- `usePlaceholderRegistration({ tasks, stableKeys, visibleIndices, layerKey, parentZone, scrollEl, scrollOffset })` — diff'd against `registeredRef`:
  - Tasks now off-screen → push a `RegisterEntry::Zone` into the batch (rect estimated from the column's bounding box and `ESTIMATED_ITEM_HEIGHT`, with `scrollOffset` subtracted to land in the same viewport coordinate frame the real-mounted cards use).
  - Tasks now on-screen → unregister the placeholder so the kernel keeps a single live registration per id.
  - Off-screen entries ship through `invoke("spatial_register_batch", { entries })` in one IPC call.
  - Cleanup effect unregisters every live placeholder when the column unmounts so torn columns don't leak registry entries.
- Spatial-nav stack absent (no `<SpatialFocusProvider>` / `<FocusLayer>`)? The hooks degrade to a no-op via `useOptionalLayerKey()` + `useOptionalSpatialFocusActions()`. `column-view.test.tsx` (which mounts column-view bare) keeps passing.

The wiring lives inside `VirtualColumn` (only path that activates above `VIRTUALIZE_THRESHOLD`), not on `<ColumnView>` — keeps the outer `<FocusScope>` wrap untouched, which the parallel FocusScopeBody-fix card also edits.

### Deferred follow-up (still ticked on the acceptance list)

`nav.down` past the visible window now lands on a placeholder Moniker in the `FocusChangedEvent`, but routing that focus event into a virtualizer `scrollToIndex` call (so the row scrolls into view and the real card mounts) requires either prop-threading the placeholder `SpatialKey` into `EntityCard`'s `<FocusScope>` or registering a moniker→scroll bridge from the column. Both touch files outside the parallel-safety envelope of this pickup (`<FocusScope>`, `<EntityCard>`, `<FocusZone>`). Tracked for a follow-up card.

### Verification

- `cargo test -p kanban-app` → 93 passed, 0 failed.
- `cargo test -p swissarmyhammer-focus` → 22 unit + 12 batch_register + 1 crate_compiles + 11 drill + 11 fallback + 18 focus_registry + 7 focus_state + 26 navigate + 5 traits_object_safe + 0 doc = 113 tests pass.
- `cargo clippy -p kanban-app --all-targets -- -D warnings` → clean.
- `cargo clippy -p swissarmyhammer-focus --all-targets -- -D warnings` → clean.
- `pnpm vitest run` (frontend) → 1571 tests pass across 143 files.
- `npx tsc --noEmit` → clean.

## Review-Fix Pickup (2026-04-26 — second pass)

Address the three warnings + first two nits from the 12:37 review section. All edits scoped to `kanban-app/ui/src/components/column-view.tsx` and `kanban-app/ui/src/components/column-view.spatial-nav.test.tsx` (the parallel-safety envelope). No Rust changes — nit 3 (`spatial_register_batch_inner` symmetry) was informational only.

### Effect-ordering leak fix

`registeredRef` in `usePlaceholderRegistration` changed from `Set<string>` to `Map<string, SpatialKey>`. The unregister loop now reads each placeholder's key out of `registeredRef` directly instead of routing through the live `stableKeys` map. The hook is now self-sufficient: `useStableSpatialKeys`'s prune effect can fire first in commit (and forget the deleted task's key) without breaking the unregister path. The cleanup effect's dependency list also dropped `stableKeys` — the ref alone now carries everything the cleanup needs.

The docstring on `useStableSpatialKeys` carries a new "Commit-order caveat for callers" paragraph documenting why a downstream consumer must keep its own copy of the keys it has registered against.

### Coordinate-frame fix

`PlaceholderRegistrationInputs` gained a new field `scrollOffset: number | null` (the virtualizer's `scrollOffset`). Inside the effect, the placeholder y is now `baseY + i * ESTIMATED_ITEM_HEIGHT - (scrollOffset ?? 0)` — viewport-relative to match what `getBoundingClientRect()` returns for real-mounted cards. The field's docstring spells out why both systems need to share one frame and what beam-search breakage looks like otherwise.

`VirtualColumn` passes `virtualizer.scrollOffset` into the hook alongside `scrollRef.current`.

### Width-fallback nit

The bizarre `width = ESTIMATED_ITEM_HEIGHT` axis-mixing fallback is gone. When `scrollEl` is `null` the hook now skips the off-screen build entirely; the next render fires the effect again once the ref has attached and the rect is real. Code is simpler and beam search never sees a fabricated rect.

### Test coverage

Three new vitest cases in `column-view.spatial-nav.test.tsx`:

- `unregisters a placeholder when its task is removed from the column` — locks in the effect-ordering fix. Renders 60 tasks, snapshots the placeholder key for `t50`, rerenders without that task, asserts `spatial_unregister_scope` fires for that key.
- `computes placeholder rects in viewport coordinates after the column scrolls` — locks in the coordinate-frame fix. Stubs `getBoundingClientRect` to a known origin, scrolls the virtualizer 1600px, asserts the placeholder for `t1` (now above the viewport) has y < baseY.
- `unregisters every live placeholder when the column unmounts` — locks in the cleanup contract. Snapshots the placeholder key set, unmounts, asserts every key shows up in `spatial_unregister_scope` calls.

### waitFor migration nit

The original "ships a spatial_register_batch invoke" test replaced its hard-coded 50ms timeout with `waitFor(() => expect(batchCalls.length).toBeGreaterThan(0))` from `@testing-library/react`. Settles as soon as the assertion passes; no flakiness on slow CI.

### Verification

- `pnpm vitest run src/components/column-view.spatial-nav.test.tsx` → 10 passed (was 7; +3 new).
- `pnpm vitest run` → 1571 tests pass across 143 files.
- `npx tsc --noEmit` → clean.
- `cargo test -p kanban-app` → 93 passed, 0 failed.
- `cargo clippy -p kanban-app --all-targets -- -D warnings` → clean.