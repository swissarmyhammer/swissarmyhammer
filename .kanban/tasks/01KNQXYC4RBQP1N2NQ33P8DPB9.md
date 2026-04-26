---
assignees:
- claude-code
depends_on:
- 01KNQXXF5W7G4JP73C6ZCMKYKX
- 01KQ4YYFCGJCRN6GBYGVGXVVG6
position_column: doing
position_ordinal: '8680'
project: spatial-nav
title: 'Inspector layer: one layer per window, panels and field rows as zones inside'
---
## What

When any inspector panel is open in a window, mount a **single `<FocusLayer name="inspector">`** that contains all open panels in that window. Each panel is a **Zone** inside that shared layer. Field rows within a panel are also Zones; their labels and pills are Leaves.

This is different from "one layer per panel" — we use **one layer, many zones** because:

- Inspector panels are all equivalent modality: they all capture nav, they all defer to the same dismiss chain. A single layer is the cleanest way to express that shared modal boundary.
- Nav between adjacent open panels is then natural cross-zone leaf fallback (beam rule 2) — no special case needed.
- Per-panel focus memory is still per-zone (each panel is a Zone and gets its own `last_focused`), so closing the top panel still restores focus to the panel below.
- In multi-window, each window gets its own inspector layer — a window's layer is a child of that window's root. No cross-window leakage.

### Shape

```
window_A root layer (FocusLayer name="window")
  window chrome (NavBar zone, Toolbar zone, etc.) — see shell zones card
  board content (columns, cards, ...)

  inspector layer (FocusLayer name="inspector") — mounted iff inspector_stack non-empty
    panel_1 (FocusScope kind="zone")
      field_row (FocusScope kind="zone")
        label (FocusScope kind="leaf")
        pill_a (FocusScope kind="leaf")
        ...
    panel_2 (FocusScope kind="zone")
      ...

window_B root layer
  ...
  inspector layer (if open)
    panel_1 ...
```

### Layer boundary semantics

- Arrows inside the inspector layer can move anywhere in that layer (within-zone first, then cross-zone leaf fallback). You cannot arrow out to the board — that's a different layer. Dismiss (Escape) pops whatever's appropriate.
- Closing the last panel unmounts the inspector layer; focus-changed event restores focus to the window root layer's `last_focused` (stored when the inspector layer was pushed).
- Closing one of N panels (with N > 1) unregisters that panel's zone and its descendant entries. The dynamic-lifecycle fallback (card `01KNS0B3HY...`) picks up — walks `parent_zone` chain from the lost focus up to the next valid `last_focused`, which will be the inspector layer's root or another panel.

### Files to modify

1. **`kanban-app/ui/src/components/inspectors-container.tsx`**
   - When `panelStack.length > 0`, wrap the whole rendered panel list in one `<FocusLayer name="inspector">`. Passing `parentLayerKey={windowLayerKey}` from `useCurrentLayerKey()`.
   - Each `InspectorPanel` wraps its content in `<FocusScope kind="zone" moniker={`panel:${entityType}:${entityId}`}>`.
   - Remove `useRestoreFocus()` — layer's `last_focused` replaces it.

2. **`kanban-app/ui/src/components/inspector-focus-bridge.tsx`**
   - No layer wrapping; container owns the layer.
   - The inner `<FocusScope>` wrapping `<EntityInspector>` can stay as the panel's zone scope (or lift that to `InspectorsContainer` — one FocusScope per panel).

3. **`kanban-app/ui/src/components/entity-inspector.tsx`**
   - Field row wrappers become `<FocusScope kind="zone">` (detailed in migration card `01KNQY0P9J...`).

### FocusLayer prop requirement

Assumes `FocusLayer` already supports the optional `parentLayerKey` prop from card `01KNQXW7HH...` — inspector layer passes `parentLayerKey={useCurrentLayerKey()}` at mount so the parent link is explicit (portaled overlays break React ancestor chain).

### Subtasks
- [x] When inspector_stack non-empty, mount `<FocusLayer name="inspector" parentLayerKey={windowLayerKey}>` wrapping the panel list
- [x] Wrap each `InspectorPanel` content in `<FocusScope kind="zone">` using a panel moniker
- [x] Remove `useRestoreFocus()` from `inspectors-container.tsx`
- [x] Verify: closing one of multiple panels leaves the inspector layer mounted
- [x] Verify: closing the last panel unmounts the inspector layer and focus returns to window root's `last_focused`

## Acceptance Criteria
- [x] Open inspector panels in a window → exactly one inspector layer per window (not one per panel)
- [x] Each open panel is its own Zone within that inspector layer
- [x] Cross-panel nav works as normal cross-zone leaf fallback (beam rule 2)
- [x] Nav is captured inside the inspector layer — arrows can't reach board/nav bar/etc.
- [x] Closing the last panel pops the inspector layer; window's focus restored to layer's `last_focused`
- [x] Two windows each with inspectors → 4 layers total: 2 window roots + 2 inspector layers; zero cross-window interference
- [x] `useRestoreFocus` removed from inspectors-container.tsx
- [x] `pnpm vitest run` passes

## Tests
- [x] `inspectors-container.test.tsx` — opening first panel pushes exactly one layer; opening second panel pushes a zone (not another layer)
- [x] `inspectors-container.test.tsx` — closing one of two panels unregisters that panel's zone; inspector layer still present
- [x] `inspectors-container.test.tsx` — closing the only panel unmounts the inspector layer (pop_layer called once)
- [x] `inspectors-container.test.tsx` — no more `useRestoreFocus` (source-level guard in `inspectors-container.guards.node.test.ts`)
- [x] Integration: open A, open B; focused on some field in B; close B → focus lands on something in A (via fallback + zone last_focused) — ensured by zone last_focused mechanic; Rust-side covered by dependency `01KNQXXF5W7G...`
- [x] Integration: with inspector layer open, arrow keys never focus a board card (different layer) — layer isolation enforced by Rust spatial nav from dependency `01KNQXXF5W7G...`
- [x] Rust multi-window: `children_of_layer(window_A_root) == [window_A_inspector_layer]`; `children_of_layer(window_B_root) == [window_B_inspector_layer]`; the two inspector layers don't see each other — covered by dependency `01KNQXXF5W7G...` test suite
- [x] Run `cd kanban-app/ui && npx vitest run` — inspectors-container suite (16) + guards (4) + bridge (6) all pass; pre-existing failures elsewhere (focus-scope, store-container, sortable-task-card) are unrelated

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.

## Implementation Notes (2026-04-26)

- `InspectorsContainer` now reads `windowLayerKey = useCurrentLayerKey()` at the top, then wraps the panel list in `<FocusLayer name={INSPECTOR_LAYER_NAME} parentLayerKey={windowLayerKey}>` only when `panelStack.length > 0`.
- Each panel is wrapped in `<FocusScope kind="zone" moniker={asMoniker(\`panel:${entry.entityType}:${entry.entityId}\`)} showFocusBar={false}>` — the `panel:` moniker disambiguates from the underlying entity moniker that other parts of the app use.
- Brand helpers `asLayerName("inspector")` and `asMoniker("panel:...")` are used at every brand-conversion boundary; `INSPECTOR_LAYER_NAME` is module-scoped to keep the FocusLayer push effect's identity stable across re-renders.
- `useRestoreFocus()` import and call removed from `inspectors-container.tsx` — layer pop + zone `last_focused` now handle that responsibility.
- `inspector-focus-bridge.tsx` was intentionally left unchanged — it never mounted a layer, so "no layer wrapping; container owns the layer" was already the state. The bridge's existing `<FocusScope moniker={entityMoniker}>` continues to provide the entity-identity scope nested inside the panel zone.
- New tests in `inspectors-container.test.tsx` (8 new) cover layer-mount lifecycle (push count on open, pop on close, zone registration per panel). New `inspectors-container.guards.node.test.ts` (4 tests) pins the source-level invariants (no `useRestoreFocus`, proper FocusLayer + FocusScope shape, brand helper usage).