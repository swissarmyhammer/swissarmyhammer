---
assignees:
- claude-code
position_column: todo
position_ordinal: a880
project: spatial-nav
title: 'Spatial-nav debug overlay: inspector `<FocusLayer>` paints into a 0×0 wrapper because all its children are `position: fixed`'
---
## What

When an inspector panel is open with `<FocusDebugProvider enabled>`, no red dashed `[data-debug="layer"]` border appears anywhere on screen. The layer is registered with the kernel (the panel's `<FocusZone>` arrow-nav and focus-claim work), but the visual aid for the layer itself is invisible.

Root cause: `<FocusLayer>` wraps its children in `<div className="relative">` when debug is on (`focus-layer.tsx:200-204`). For the inspector layer, the children are `<InspectorPanel>` components whose only DOM box is a `<SlidePanel>` rendered with `position: fixed top-0 z-30 h-full w-[420px]` (`slide-panel.tsx:30-33`). Fixed-positioned children contribute nothing to flow layout, so the wrapping `<div>` collapses to **0 × 0 pixels**. `<FocusDebugOverlay>` reads `hostRef.current.getBoundingClientRect()` on that 0×0 host — the dashed border has zero size and the `layer:inspector` label sits at the host's top-left corner (which is wherever the empty wrapper happens to land in the React tree, typically `(0, 0)` in viewport coords). Both are visually undetectable.

The window-root layer doesn't show this symptom because its children include real flow content (NavBar, ViewsContainer, etc.) that gives the wrapping `<div>` non-zero dimensions. The inspector layer is the canary; any future layer whose children are entirely `position: fixed`, `position: absolute`, or portaled out will hit the same bug.

## Where this lives

- `kanban-app/ui/src/components/focus-layer.tsx:198-209` — the conditional debug wrap.
- `kanban-app/ui/src/components/focus-debug-overlay.tsx:131-216` — the overlay that reads the host rect via `getBoundingClientRect()` on every animation frame and renders a `border border-dashed` span at `inset-0`.
- `kanban-app/ui/src/components/inspectors-container.tsx:184-188` — the inspector layer mount site (`<FocusLayer name={INSPECTOR_LAYER_NAME} parentLayerKey={windowLayerKey}>`).
- `kanban-app/ui/src/components/slide-panel.tsx:28-34` — `position: fixed top-0 z-30 h-full w-[420px]` is the load-bearing class chain that makes the panel a fixed positional layer outside flow.
- `kanban-app/ui/src/components/focus-debug-overlay.tsx:189-192` — the existing layer-rendering branch deliberately omits `(x, y, w, h)` from the label because "layers are pure context providers and have no rect of their own". So the visual value of the layer overlay is already just "label + dashed border showing the bounded region the layer captures." A 0×0 host loses both.

## Why the architectural intent matters here

The FocusLayer model says a layer represents a modal-boundary scope for keyboard nav — the inspector layer captures arrow keys for the open panel(s) and excludes the underlying board. The honest debug visualisation is **a dashed border around the area the layer logically occupies**. For the inspector layer, that area is the union of all open panels (each 420px wide, fixed to the right edge, stacked). A more permissive read: the inspector layer's effective footprint is the entire viewport because it intercepts arrow keys window-wide while open. Either definition is bigger than 0×0 and informative; the current rendering is neither.

## Approach

Two architectural options. Pick A.

### Option A — FocusLayer debug wrapper paints across the viewport (recommended)

Change the debug-mode wrapper at `focus-layer.tsx:200-204` to a fixed-position container that spans the viewport:

```jsx
<div ref={debugHostRef} className="fixed inset-0 pointer-events-none">
  <FocusDebugOverlay kind="layer" label={name} hostRef={debugHostRef} />
  {children}
</div>
```

`fixed inset-0` gives the wrapper a real, full-viewport box. `pointer-events-none` ensures the wrapper does NOT intercept clicks, drags, or hovers from the panels or the underlying board. Children inside the wrapper that are themselves `position: fixed` continue to position relative to the viewport (their containing block is the nearest ancestor with `position`, but since the wrapper now has `position: fixed` itself, fixed children bind to the same viewport — no visual regression for SlidePanel).

The overlay's dashed border now paints around the viewport while a layer is mounted, with the `layer:<name>` label in the top-left corner. Visually, this reads as "this is the kanban-app's currently-active modal boundary."

**Wrinkle**: the window-root layer's wrapper currently collapses to its flow children's height (NavBar + view content + mode indicator). Switching to `fixed inset-0` for the window layer means the wrapper no longer participates in flow, so any descendant that depends on the wrapper as a containing block (relative ancestor) would see a different containing block. **Verify**: does any descendant code path read the FocusLayer's wrapper as its `position: relative` containing block? If yes, the wrapper change is not safe for the window layer; introduce a per-layer `displayMode: "flow" | "viewport"` prop, default to `"flow"` for window-root and `"viewport"` for the inspector and any future fixed-positioned layer.

The simpler refactor: switch ALL FocusLayer debug wrappers to `fixed inset-0 pointer-events-none` and rely on the fact that no current consumer depends on the wrapper's flow/relative behavior (the wrapper exists solely as a debug-only host). Verify by `grep`-ing for any code that walks up to a "layer wrapper" element.

### Option B — Skip the wrapper, render the overlay as a sibling fixed-position element

Drop the `<div className="relative">` wrapper entirely. Always render `<FocusDebugOverlay>` as a sibling of children, with the overlay component itself owning a `fixed inset-0` host:

```jsx
<FocusLayerContext.Provider value={key}>
  {debugEnabled && <FocusLayerDebugOverlay name={name} />}
  {children}
</FocusLayerContext.Provider>
```

`<FocusLayerDebugOverlay>` is a thin component that renders `<div className="fixed inset-0 pointer-events-none border border-dashed border-red-500/70"><span ... label ... /></div>` directly. No host ref, no rAF poll, no `getBoundingClientRect()` — the layer has no rect of its own anyway, so the rAF loop is wasted work in the layer case.

Option B is more honest to the data model (layers have no rect; don't pretend) but introduces a separate code path for the layer debug overlay vs the zone/scope overlays.

**Default to Option A.** Smaller change, single overlay component, single rendering path.

## Acceptance Criteria

All asserted by automated tests below.

- [ ] When `<FocusDebugProvider enabled>` is mounted and any inspector panel is open, a red dashed `[data-debug="layer"]` element exists in the DOM AND its bounding rect spans the viewport (`width === window.innerWidth`, `height === window.innerHeight`).
- [ ] The label inside the inspector-layer overlay reads `layer:inspector` and is visible in the top-left of the viewport.
- [ ] The overlay does NOT intercept clicks, drags, or hovers — clicking on a panel still focuses it; clicking the backdrop still closes panels.
- [ ] When the last inspector panel closes (`<FocusLayer>` unmounts), the layer overlay disappears (no leftover `[data-debug="layer"][data-layer-name="inspector"]`).
- [ ] Window-root layer overlay continues to render (regression guard for Option A's wrapper change). Its rect is also viewport-sized after the change; this is acceptable per the layer-has-no-rect data model.
- [ ] Existing tests continue to pass: `kanban-app/ui/src/components/focus-debug-overlay.browser.test.tsx`, `kanban-app/ui/src/components/inspectors-container.spatial-nav.test.tsx`, `kanban-app/ui/src/components/inspector-dismiss.browser.test.tsx`.

## Tests

### `kanban-app/ui/src/components/focus-debug-overlay.layer.browser.test.tsx` (new file or extend existing browser test)

- [ ] `inspector_layer_overlay_renders_at_viewport_size` — mount `<App />` with debug enabled and one inspector panel open; assert `[data-debug="layer"]` (with the inspector layer's identifying attribute) has bounding rect `width === window.innerWidth` and `height === window.innerHeight`.
- [ ] `inspector_layer_overlay_label_includes_layer_name` — same mount; assert the overlay's label text contains `layer:inspector`.
- [ ] `inspector_layer_overlay_does_not_intercept_clicks` — same mount; click a panel's content; assert the click handler runs (focus claim fires; `pointer-events: none` on the overlay is honoured).
- [ ] `inspector_layer_overlay_unmounts_when_last_panel_closes` — same mount; close all panels; assert no `[data-debug="layer"][data-layer-name="inspector"]` remains.
- [ ] `window_layer_overlay_still_renders_after_wrapper_change` — mount `<App />` with no panels open; assert the window-root `[data-debug="layer"]` still renders with non-zero rect (regression guard for Option A's flow → viewport switch on the window layer).

If the layer overlay component lacks a stable per-layer-name selector (currently it carries `data-debug="layer"` only), add `data-layer-name={name}` to the overlay's outer span so the test selectors can target the inspector vs window layer specifically.

Test command: `cd kanban-app/ui && bun test focus-debug-overlay.layer.browser` — all five pass.

### Existing tests must keep passing

- [ ] `kanban-app/ui/src/components/focus-debug-overlay.browser.test.tsx` (the existing nine cases, including `layer_renders_no_dom_when_debug_off` — the regression guard for the production-layout-byte-identical-when-debug-off behaviour stays intact).
- [ ] `kanban-app/ui/src/components/inspectors-container.spatial-nav.test.tsx`
- [ ] `kanban-app/ui/src/components/inspector-dismiss.browser.test.tsx`
- [ ] `kanban-app/ui/src/components/focus-layer.test.tsx`

Test command: `cd kanban-app/ui && bun test focus-layer focus-debug-overlay inspectors-container inspector-dismiss` — all green.

## Workflow

- Use `/tdd` — write `inspector_layer_overlay_renders_at_viewport_size` first; it fails today (rect is 0×0). Then apply Option A's wrapper change. Re-run green.
- Verify no descendant code path depends on the FocusLayer's debug wrapper as a containing block — `grep` for any code that walks up to a `[data-debug="layer"]` or relies on the layer's wrapper for positioning. If any consumer is found, switch to a per-layer `displayMode` prop or fall back to Option B.
- Add `data-layer-name={name}` to `<FocusDebugOverlay kind="layer" label={name}>` (or the host span it renders) so tests can target specific layers — required for the new browser test selectors. This is a one-line additive change in `focus-debug-overlay.tsx`.

#frontend #spatial-nav #kanban-app