---
assignees:
- claude-code
position_column: todo
position_ordinal: a980
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

#### 1. New context — `kanban-app/ui/src/components/focus-layer.tsx`

Add a second context next to `FocusLayerContext`:

```ts
/** Z-index tier for the enclosing layer's descendants. */
export const FocusLayerZTierContext = createContext<number>(0);
```

The default value of `0` matches "no layer above us" — the window-root layer.

#### 2. Tier resolution per layer name

The simplest tier mapping uses each layer's existing modal-content z-index as its tier baseline:

| Layer name | Modal content z-index | Tier (overlay base) |
|------------|-----------------------|---------------------|
| `window`   | none (flow content)   | 10                  |
| `inspector` | 30 (SlidePanel)       | 30                  |
| `dialog`   | (per-dialog)          | parent + 20         |
| `palette`  | 50 (palette backdrop) | 60                  |

Implement as a per-name lookup at the top of the FocusLayer module:

```ts
const LAYER_Z_TIERS: Record<string, number> = {
  window: 10,
  inspector: 30,
  dialog: 50,
  palette: 60,
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
    style={{ zIndex: tier + 5 }}
  >
    {/* … */}
  </span>
);
```

`tier + 5` places the overlay just above its layer's modal content but below the next layer's overlays. With the tiers in step 2:

- Window-root overlays: z-15 — below inspector backdrop (z-20) and SlidePanel (z-30).
- Inspector overlays: z-35 — above SlidePanel content (z-30) but below palette overlays (z-65).
- Palette overlays: z-65 — above palette backdrop (z-50).

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

- [ ] When `<FocusDebugProvider enabled>` is mounted and an inspector panel is open, no `[data-debug]` element from a window-root descendant has computed `z-index >= 20`. (Pinned by the new browser test inspecting overlay z-index.)
- [ ] In the same scenario, every `[data-debug]` element from an inspector-layer descendant (i.e. inside a panel zone or its leaves) has computed `z-index >= 30`. (Pinned: inspector debug overlays still paint above the panel.)
- [ ] When the palette is open on top of the inspector, palette-layer descendant overlays have computed `z-index >= 60`; inspector-layer overlays have `z-index < 60` so they don't bleed through the palette.
- [ ] The user-reported symptom (column zone's blue dashed border crossing into the open inspector area) does not occur. Pinned by a browser test that snapshots `document.elementsFromPoint(panelLeftEdge - 1, panelMidY)` and asserts the topmost element is the SlidePanel (or its descendants), not a window-root overlay.
- [ ] No regression: `kanban-app/ui/src/components/focus-debug-overlay.browser.test.tsx`'s nine existing cases still pass after the z-index change. Update those that hardcoded a z-50 expectation, if any.
- [ ] No regression on click routing: clicking through the layered debug overlays still reaches the underlying UI. Pre-existing pointer-events tests still pass.

## Tests

### `kanban-app/ui/src/components/focus-debug-overlay.layer-z.browser.test.tsx` (new file)

- [ ] `window_layer_overlay_z_index_is_below_inspector_backdrop` — mount `<App />` with debug enabled and one inspector panel open; pick any column-zone overlay (`[data-moniker^="column:"] [data-debug="zone"]`); assert computed `getComputedStyle(el).zIndex` is a number < 20.
- [ ] `inspector_layer_overlay_z_index_is_above_slide_panel` — same mount; pick a panel-zone overlay (`[data-moniker^="panel:"] [data-debug="zone"]`); assert `zIndex > 30`.
- [ ] `column_overlay_does_not_paint_over_inspector_panel` — same mount; compute the SlidePanel's left edge x-coordinate; call `document.elementsFromPoint(panelLeft + 10, viewportHeight / 2)` (a point inside the panel); assert the topmost element is the SlidePanel or one of its descendants — NOT a `[data-debug]` span belonging to a window-layer ancestor.
- [ ] `palette_overlay_z_index_is_above_inspector_overlay` — open the command palette while an inspector panel is also open (or a synthetic palette-layer mount); assert palette-layer overlays have `zIndex > inspectorTier`.
- [ ] `nested_unnamed_layer_falls_through_to_parent_plus_twenty` — synthetic test: mount two custom-named FocusLayers and assert the inner layer's overlay z-index is exactly `parentTier + 20 + 5` (verifies the fallback path in step 2's tier resolution).

Test command: `cd kanban-app/ui && bun test focus-debug-overlay.layer-z.browser` — all five pass.

### Existing tests must keep passing

- [ ] `kanban-app/ui/src/components/focus-debug-overlay.browser.test.tsx` — nine existing cases. Update any case that hardcoded a z-50 expectation to read the per-layer tier instead. If none hardcoded the value (most assertions check class membership and rect, not z-index), no change is needed.
- [ ] `kanban-app/ui/src/components/focus-layer.test.tsx` — layer push / pop semantics unchanged.
- [ ] `kanban-app/ui/src/components/inspectors-container.spatial-nav.test.tsx` — inspector arrow-nav unchanged.
- [ ] `kanban-app/ui/src/lib/focus-debug-context.test.tsx` — provider on/off contract unchanged.

Test command: `cd kanban-app/ui && bun test focus-layer focus-debug-overlay focus-debug-context inspectors-container` — all green.

## Workflow

- Use `/tdd` — write `column_overlay_does_not_paint_over_inspector_panel` first; it fails today (the column overlay at z-50 is on top of the SlidePanel at z-30). Then add `FocusLayerZTierContext`, the per-name tier table, and the `<FocusDebugOverlay>` style change. Re-run green.
- Land independently of `01KQCHZW5R0WJXTP4BG67QE0Z7` (inspector layer 0×0 wrapper). The two fixes touch different files and don't conflict — although both interact with FocusLayer, this card adds a new context next to the existing layer context and the other card changes the wrapper's positioning. Coordinate review if both land in the same window of work but neither blocks the other.
- Keep the per-layer-name tier table at module scope in `focus-layer.tsx` — adding a new layer name later (e.g. `dialog` for confirm/alert dialogs) is a one-line edit at a single location.

#frontend #spatial-nav #kanban-app