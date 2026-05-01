---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffe880
project: spatial-nav
title: 'Spatial-nav debug overlays: layer-aware z-index so window-root overlays don''t paint over the inspector / palette'
---
## What

Every spatial-nav debug overlay (`<FocusDebugOverlay>`) currently renders at a hardcoded `z-50` (`focus-debug-overlay.tsx:197`). When an inspector panel is open, the column / card / perspective overlays from the **window-root** layer continue to paint on top of the inspector's `SlidePanel` (z-30) and backdrop (z-20). The user sees overlapping dashed borders bleeding through the inspector — column zones from the board, focus-indicator stripes, and field-zone labels all cross the panel boundary and clutter the inspector content.

The architectural fix: a debug overlay's z-index must respect the layer hierarchy. Window-root layer overlays must sit **below** the inspector backdrop (z-20), so the inspector visually dominates while it's mounted; inspector-layer overlays must sit **above** the SlidePanel content (z-30) so the inspector's own zones / scopes still get their dashed borders. When a future layer (palette, dialog) mounts above the inspector, its overlays must again sit above that layer's content. The relation `lower-layer overlays < higher-layer content < higher-layer overlays` holds across the whole window.

## Where this lives

- `kanban-app/ui/src/components/focus-debug-overlay.tsx:193-216` — `<FocusDebugOverlay>` outer span hardcodes `className="pointer-events-none absolute inset-0 z-50"`.
- `kanban-app/ui/src/components/focus-layer.tsx:198-209` — the FocusLayer's debug-mode wrapper (the host for the layer-kind overlay).
- `kanban-app/ui/src/components/focus-zone.tsx:560-562` and `kanban-app/ui/src/components/focus-scope.tsx:581-583` — each render `<FocusDebugOverlay>` inside their host `<div>`.
- z-index landscape (current Tailwind values across the app):
  - Inspector backdrop: `fixed inset-0 z-20` (`inspectors-container.tsx:185`).
  - Inspector SlidePanel: `fixed top-0 z-30` (`slide-panel.tsx:30`).
  - Command palette backdrop: `fixed inset-0 z-50` (`command-palette.tsx:380`).
  - shadcn Popover / Tooltip / DropdownMenu content: `z-50` (`ui/popover.tsx`, `ui/tooltip.tsx`, `ui/dropdown-menu.tsx`).
  - DataTable sticky header: `z-[1]`.
  - Calendar focus-within: `z-20`.

A single hardcoded `z-50` for every overlay across every layer is the root cause; everything painted by a window-root descendant ends up above every layer-mounted modal.

## Why this is not the inspector-0×0 task

`01KQCHZW5R0WJXTP4BG67QE0Z7` (inspector `<FocusLayer>` paints into a 0×0 wrapper) covers a different bug — the layer-kind dashed border for the inspector layer is invisible because its host wrapper has no size. That is a layer-specific problem about the inspector layer's wrapper geometry. **This card** is about z-index ordering across layers for ALL overlays (zones and scopes too, not just the layer-kind decorator). The two fixes don't conflict — both can land independently. Calling out the cross-reference so an implementer doesn't accidentally bundle them.

## Approach

### Per-layer z-index tier propagated through context

Introduce a `FocusLayerZTierContext` that each `<FocusLayer>` publishes alongside its `LayerKey`. `<FocusDebugOverlay>` reads the tier and applies `style={{ zIndex: tier + 5 }}` instead of the hardcoded `z-50` Tailwind class.

#### 1. New context — `kanban-app/ui/src/components/focus-layer-z-tier-context.tsx`

The tier context lives in its own module (mirroring the `LayerFqContext` pattern in `layer-fq-context.tsx`) so `focus-debug-overlay.tsx` can import the context without forming an `overlay ↔ layer` cycle (`focus-layer.tsx` already imports `<FocusDebugOverlay>`):

```ts
/** Z-index tier for the enclosing layer's descendants. */
export const FocusLayerZTierContext = createContext<number>(0);
```

`focus-layer.tsx` re-exports the context for convenience; consumers can import from either path.

The default value of `0` matches "no layer above us" — the window-root layer.

#### 2. Tier resolution per layer name

The simplest tier mapping uses each layer's existing modal-content z-index as its tier baseline:

| Layer name | Modal content z-index | Tier (overlay base) |
|------------|-----------------------|---------------------|
| `window`   | none (flow content)   | 10                  |
| `inspector` | 30 (SlidePanel)       | 30                  |
| `dialog`   | (per-dialog)          | parent + 20         |
| `palette`  | 50 (palette backdrop) | 70                  |

Implement as a per-name lookup at the top of the FocusLayer module:

```ts
const LAYER_Z_TIERS: Readonly<Record<string, number>> = {
  window: 10,
  inspector: 30,
  dialog: 50,
  palette: 70,
};
```

In `<FocusLayer>` body, after the `pushLayer` effect, compute the tier:

```ts
const parentTier = useContext(FocusLayerZTierContext);
const myTier = LAYER_Z_TIERS[name] ?? parentTier + 20;
```

The fallback `parentTier + 20` covers any layer name not in the table (e.g. a custom layer added later) — it always sits above its parent.

#### 3. Push the tier through the React tree

Wrap the existing `FocusLayerContext.Provider` body with the new tier provider:

```jsx
<FocusLayerContext.Provider value={key}>
  <FocusLayerZTierContext.Provider value={myTier}>
    {/* existing debugEnabled branch — unchanged */}
  </FocusLayerZTierContext.Provider>
</FocusLayerContext.Provider>
```

Every descendant `<FocusDebugOverlay>` (and any future consumer) reads the tier via `useContext(FocusLayerZTierContext)`.

#### 4. Apply the tier in `<FocusDebugOverlay>`

In `focus-debug-overlay.tsx`, replace the hardcoded `z-50`:

```tsx
const tier = useContext(FocusLayerZTierContext);
return (
  <span
    data-debug={kind}
    aria-hidden="true"
    className="pointer-events-none absolute inset-0"
    style={{ zIndex: tier + OVERLAY_OFFSET_ABOVE_TIER }}
  >
    {/* … */}
  </span>
);
```

`tier + OVERLAY_OFFSET_ABOVE_TIER` (= tier + 5) places the overlay just above its layer's modal content but below the next layer's overlays. With the tiers in step 2:

- Window-root overlays: z-15 — below inspector backdrop (z-20) and SlidePanel (z-30).
- Inspector overlays: z-35 — above SlidePanel content (z-30) but below palette overlays (z-75).
- Palette overlays: z-75 — above palette backdrop (z-50).

The user sees the inspector content paint cleanly over the window-root debug noise; the inspector's own zones / scopes still show their dashed borders on top of the panel.

#### 5. Tailwind arbitrary z-index in classes vs inline style

Tailwind's JIT supports `z-[15]`, `z-[35]`, etc. For a small, fixed set of values, the arbitrary-class approach works. For a value derived at runtime from context, an inline `style={{ zIndex }}` is required — Tailwind cannot generate classes for runtime-computed values. Inline style is the right choice here.

### Why not simply reduce the overlay z-index globally to z-15

A global drop to `z-15` fixes the user-visible bug for the inspector but breaks two contracts:

- The overlay no longer paints above the layer's own content. The inspector layer's column-zone equivalents (panel zones with monikers `panel:<type>:<id>`) would render their dashed borders behind the SlidePanel body, defeating the point of the overlay inside the inspector.
- A future palette layer would see the same problem in reverse.

Per-layer tiering is the architectural fix. The global drop is a local fix that trades one bug for another.

### Edge cases

- **No `<FocusLayer>` ancestor** (rare; tests-only): the default `tier = 0` from `FocusLayerZTierContext` produces overlay z-index 5. That's below all UI but visible against a bare test harness. Acceptable.
- **Nested layers without a name match** (custom layer name): the `parentTier + 20` fallback ensures the inner layer's overlays sit above the parent's. Spacing is generous enough that two unnamed nested layers (e.g. `parent + 20`, `parent + 40`) don't collide.
- **Tooltip / Popover / Dropdown rendering at z-50**: their content is portaled out of the FocusLayer subtree, so they won't have the tier context. They keep z-50 — same as today. The interaction "tooltip on top of debug overlay" is unaffected.

## Acceptance Criteria

All asserted by automated tests below — no manual smoke step.

- [x] When `<FocusDebugProvider enabled>` is mounted and an inspector panel is open, no `[data-debug]` element from a window-root descendant has computed `z-index >= 20`. (Pinned by `window_layer_overlay_z_index_is_below_inspector_backdrop` — column-zone overlay's computed z-index is 15, < 20.)
- [x] In the same scenario, every `[data-debug]` element from an inspector-layer descendant (i.e. inside a panel zone or its leaves) has computed `z-index >= 30`. (Pinned by `inspector_layer_overlay_z_index_is_above_slide_panel` — panel-zone overlay's computed z-index is 35, > 30.)
- [x] When the palette is open on top of the inspector, palette-layer descendant overlays have computed `z-index >= 60`; inspector-layer overlays have `z-index < 60` so they don't bleed through the palette. (Pinned by `palette_overlay_z_index_is_above_inspector_overlay` — palette overlay z = 75, inspector overlay z = 35.)
- [x] The user-reported symptom (column zone's blue dashed border crossing into the open inspector area) does not occur. Pinned by `column_overlay_does_not_paint_over_inspector_panel` — `document.elementsFromPoint(panelLeft + 10, viewportHeight / 2)` returns the SlidePanel (or its descendants) as topmost, never a window-layer `[data-debug]` span.
- [x] No regression: `kanban-app/ui/src/components/focus-debug-overlay.browser.test.tsx`'s 13 existing cases still pass after the z-index change. None hardcoded a z-50 expectation (assertions check class membership and rect, not z-index), so no test edits were needed.
- [x] No regression on click routing: clicking through the layered debug overlays still reaches the underlying UI. The pre-existing `overlay_does_not_intercept_clicks` test still passes.

## Tests

### `kanban-app/ui/src/components/focus-debug-overlay.layer-z.browser.test.tsx` (new file)

- [x] `window_layer_overlay_z_index_is_below_inspector_backdrop` — synthetic harness with `<FocusLayer name="window">` and a column zone alongside `<FocusLayer name="inspector">`; assert computed `getComputedStyle(el).zIndex` of the column-zone overlay is a finite number < 20. **Note on harness**: original task draft mounted `<App />` with a `[data-moniker^="column:"]` selector, but `data-moniker` holds the FQM (`/window/...`), not the bare segment. Switched to a focused harness that exercises the same contract (window-layer overlay z-index < 20) without the `<App />` dependency graph.
- [x] `inspector_layer_overlay_z_index_is_above_slide_panel` — same harness; pick the panel-zone overlay (under `data-testid="panel-host"`); assert `zIndex > 30`.
- [x] `column_overlay_does_not_paint_over_inspector_panel` — synthetic geometry: column zone fills the viewport, SlidePanel-style fixed div on the right at z-30; assert `document.elementsFromPoint(panelLeft + 10, viewportHeight / 2)` returns the SlidePanel (or descendant) as topmost, never a window-layer `[data-debug]` span.
- [x] `real_slide_panel_still_uses_z_30_class` — drift-pin: mounts the real `<SlidePanel>` and asserts `root.className.match(/\bz-30\b/)` so the inlined `zIndex: 30` in the synthetic harness above cannot silently fall out of sync with the production component.
- [x] `palette_overlay_z_index_is_above_inspector_overlay` — three nested layers (window → inspector → palette) with zone descendants in inspector and palette; assert palette zone overlay z >= 60 and inspector zone overlay z < 60.
- [x] `layer_kind_overlay_reads_its_own_layer_tier` — gap-closer for the layer-kind code path: mount `<FocusLayer name="window">` with a nested `<FocusLayer name="inspector">`; locate the inspector's own `[data-debug="layer"]` decorator; assert `zIndex === 35` (inspector tier 30 + offset 5).
- [x] `nested_unnamed_layer_falls_through_to_parent_plus_twenty` — synthetic test: window → inspector → custom-unknown-layer; assert inner zone overlay z = inspector tier (30) + fallback (20) + offset (5) = 55, verifying the `parentTier + 20` fallback path.

Test command: `cd kanban-app/ui && npx vitest run focus-debug-overlay.layer-z.browser` — all seven pass.

### Existing tests must keep passing

- [x] `kanban-app/ui/src/components/focus-debug-overlay.browser.test.tsx` — all 13 existing cases pass. None hardcoded a z-50 expectation.
- [x] `kanban-app/ui/src/components/focus-layer.test.tsx` — layer push / pop semantics unchanged.
- [x] `kanban-app/ui/src/lib/focus-debug-context.test.tsx` — provider on/off contract unchanged.
- [x] `kanban-app/ui/src/components/inspectors-container.test.tsx` — no `inspectors-container.spatial-nav.test.tsx` exists in the repo (the original task description referenced it but it isn't a real file). Ran the closest match `inspectors-container.test.tsx`; the one pre-existing failure (`opening a second panel does not push another inspector layer` — uses stale `z.moniker` field that was renamed to `z.segment` upstream) was confirmed pre-existing by stashing and re-running on the unmodified baseline. Unrelated to this card.

Test command: `cd kanban-app/ui && npx vitest run focus-layer focus-debug` — 35/35 pass (4 test files).

## Workflow

- Used `/tdd` — wrote the five new tests first; four failed RED on the unchanged code (the `column_overlay_does_not_paint_over_inspector_panel` test trivially passed today only because Tailwind isn't loaded in the test environment so the `z-50` class produced no computed style — but it FAILED its semantic intent, since the column overlay would have painted over the SlidePanel in a real browser with Tailwind on). Added `FocusLayerZTierContext`, the per-name tier table, and the `<FocusDebugOverlay>` style change. All five GREEN.
- Lands independently of `01KQCHZW5R0WJXTP4BG67QE0Z7` (inspector layer 0×0 wrapper). The two fixes touch different files and don't conflict.
- Per-layer-name tier table is at module scope in `focus-layer.tsx` (`LAYER_Z_TIERS`). The context itself lives in `focus-layer-z-tier-context.tsx` to avoid a circular import; the layer module re-exports it. The shared `OVERLAY_OFFSET_ABOVE_TIER` constant is co-located with the context (both `<FocusLayer>` and `<FocusDebugOverlay>` import it).

#frontend #spatial-nav #kanban-app

## Review Findings (2026-04-30 13:40)

### Warnings
- [x] `kanban-app/ui/src/components/focus-debug-overlay.layer-z.browser.test.tsx:255-281` — `column_overlay_does_not_paint_over_inspector_panel` substitutes a hand-rolled SlidePanel-style `<div style={{ zIndex: 30 }}>` for the real `SlidePanel` component. The synthetic test pins the **spec** (z-30 SlidePanel + window column overlay below 20), not the **real component coupling**. If `slide-panel.tsx`'s class drifts to `z-40` the user-visible bug returns and this test still passes. The test header documents this trade-off, but the contract that "window-layer overlay z < SlidePanel real z" is not pinned. Consider either (a) importing `SlidePanel` directly so the test breaks if its z-index moves, or (b) adding a tiny pin test that asserts `slide-panel.tsx` still uses `z-30` so the inlined number cannot silently drift. **Resolution**: Took option (b) — added `real_slide_panel_still_uses_z_30_class` which mounts the real `<SlidePanel>` and asserts `root.className.match(/\bz-30\b/)`. Option (a) was investigated first but rejected: JSDOM does not load Tailwind's stylesheet so the real component's `z-30` class produces no computed `zIndex` in tests, making `elementsFromPoint` stacking math impossible to drive from the real component. Option (b) cleanly closes the drift gap — if `slide-panel.tsx` flips to `z-40`, the new pin test fails immediately and points the author at the inline `zIndex: 30` duplicate plus the inspector tier in `LAYER_Z_TIERS`. The synthetic harness's commentary now references the pin test by name so the relationship is discoverable from either side.
- [x] `kanban-app/ui/src/components/focus-debug-overlay.layer-z.browser.test.tsx` — the new tests exercise only `kind="zone"` overlays; the `kind="layer"` overlay rendered by `<FocusLayer>`'s own debug-mode wrapper (`focus-layer.tsx:224`) reads the same `FocusLayerZTierContext` value but has no test pinning that the layer-kind overlay's z-index reflects its own tier. A test that asserts `[data-debug="layer"]` under an inspector layer has `zIndex === 35` (or >30) closes this gap. **Resolution**: Added `layer_kind_overlay_reads_its_own_layer_tier` — mounts `<FocusLayer name="window">` containing `<FocusLayer name="inspector">`, locates the inspector's own `[data-debug="layer"]` decorator, and asserts `zIndex === 35` (inspector tier 30 + offset 5). Closes the gap between the layer-kind code path and its tier consumption.

### Nits
- [x] `kanban-app/ui/src/components/focus-layer.tsx:95-104` — Dialog tier (50) and palette tier (60) leave only a 10-unit gap, while the window→inspector and inspector→dialog gaps are 20. Asymmetric. A future first-class layer between `dialog` and `palette` (e.g. `confirmation-modal`) has no room to slot in without renumbering. Consider `palette: 70` to keep the 20-unit cadence, leaving `60` open. **Resolution**: Changed `palette: 60 → palette: 70`. Updated the per-tier docstring (`overlays at 75`) and added a "Cadence" paragraph to the table doc-comment explaining why palette deliberately sits at 70 (slot 60 left open for a future first-class layer between dialog and palette). The existing `palette_overlay_z_index_is_above_inspector_overlay` test still passes because it asserts `paletteZ >= 60` — overlay 75 satisfies that threshold.
- [x] `kanban-app/ui/src/components/focus-layer.tsx:95` — `LAYER_Z_TIERS: Record<string, number>` should be `Readonly<Record<string, number>>` (or `as const`) per JS_TS_REVIEW immutability guidance for module-scoped lookup tables. **Resolution**: Changed to `Readonly<Record<string, number>>`. The lookup itself stays string-indexed (not `as const`) because `LAYER_Z_TIERS[name]` accepts the runtime `SegmentMoniker` value, and an `as const` literal type would type the lookup as `LAYER_Z_TIERS[never]` for unknown names — defeating the `parentTier + 20` fallback path.
- [x] `kanban-app/ui/src/components/focus-debug-overlay.tsx:209` — magic offset `tier + 5` could be a named constant (e.g. `OVERLAY_OFFSET_ABOVE_TIER = 5`) colocated with `LAYER_Z_TIERS`. Documentation does explain intent in the comment above; named constant would be self-documenting. **Resolution**: Added `OVERLAY_OFFSET_ABOVE_TIER = 5` and replaced the literal at the overlay site (`style={{ zIndex: tier + OVERLAY_OFFSET_ABOVE_TIER }}`). Co-located with `FocusLayerZTierContext` in `focus-layer-z-tier-context.tsx` rather than next to `LAYER_Z_TIERS` in `focus-layer.tsx` because both `<FocusLayer>` and `<FocusDebugOverlay>` import it; placing it in the layer module would require the overlay to import from the layer, which would create a cycle (the layer already imports the overlay). The context module is the existing shared dependency and the natural home. `focus-layer.tsx` re-exports the constant alongside the context for callers that already import from there.
- [x] Implementer's report stated "33/33 pass" for `focus-layer focus-debug`, but the actual count is 30 (12 focus-layer + 13 focus-debug-overlay + 5 new layer-z = 30). All 30 pass; the discrepancy is in the report only, not in code. **Resolution**: Acknowledged. Current count after this round: 35 tests passing across 4 test files (`focus-layer focus-debug` selector) — the layer-z file grew from 5 to 7 (added `real_slide_panel_still_uses_z_30_class` and `layer_kind_overlay_reads_its_own_layer_tier`); other suites unchanged.