---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffff080
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

## Implementation note (post-implementation update)

Option A was attempted first (wrap children in `fixed inset-0 pointer-events-none`). Implementation revealed a CSS-inheritance problem: `pointer-events` is an inherited property, and setting `pointer-events: none` on the wrapper made every descendant inherit `none`. `document.elementsFromPoint(...)` then returned only `<body>`/`<html>` for points inside the slide-panel area, breaking the existing `column_overlay_does_not_paint_over_inspector_panel` regression guard (and would break real clicks in production).

The fallback (Option B in the card) was adopted: render the overlay as a SIBLING of children, inside its own `position: fixed; inset: 0; pointer-events: none` host. The inheritance is now confined to the overlay host, which has no children that need pointer events (just `<FocusDebugOverlay>`, with its own `pointer-events: auto` handle for tooltip hover). Descendants of the layer keep their default `pointer-events: auto`.

The overlay component is unchanged (single rendering path is preserved); only the layout structure switched from "wrap children" to "render alongside children." `data-layer-name={name}` was added to the layer-kind overlay span as required.

## Acceptance Criteria

All asserted by automated tests below.

- [x] When `<FocusDebugProvider enabled>` is mounted and any inspector panel is open, a red dashed `[data-debug="layer"]` element exists in the DOM AND its bounding rect spans the viewport (`width === window.innerWidth`, `height === window.innerHeight`).
- [x] The label inside the inspector-layer overlay reads `layer:inspector` and is visible in the top-left of the viewport.
- [x] The overlay does NOT intercept clicks, drags, or hovers — clicking on a panel still focuses it; clicking the backdrop still closes panels.
- [x] When the last inspector panel closes (`<FocusLayer>` unmounts), the layer overlay disappears (no leftover `[data-debug="layer"][data-layer-name="inspector"]`).
- [x] Window-root layer overlay continues to render (regression guard for Option A's wrapper change). Its rect is also viewport-sized after the change; this is acceptable per the layer-has-no-rect data model.
- [x] Existing tests continue to pass: `kanban-app/ui/src/components/focus-debug-overlay.browser.test.tsx`, `kanban-app/ui/src/components/inspectors-container.spatial-nav.test.tsx`, `kanban-app/ui/src/components/inspector-dismiss.browser.test.tsx`.

## Tests

### `kanban-app/ui/src/components/focus-debug-overlay.layer.browser.test.tsx` (new file or extend existing browser test)

- [x] `inspector_layer_overlay_renders_at_viewport_size` — mount `<App />` with debug enabled and one inspector panel open; assert `[data-debug="layer"]` (with the inspector layer's identifying attribute) has bounding rect `width === window.innerWidth` and `height === window.innerHeight`.
- [x] `inspector_layer_overlay_label_includes_layer_name` — same mount; assert the overlay's label text contains `layer:inspector`.
- [x] `inspector_layer_overlay_does_not_intercept_clicks` — same mount; click a panel's content; assert the click handler runs (focus claim fires; `pointer-events: none` on the overlay is honoured).
- [x] `inspector_layer_overlay_unmounts_when_last_panel_closes` — same mount; close all panels; assert no `[data-debug="layer"][data-layer-name="inspector"]` remains.
- [x] `window_layer_overlay_still_renders_after_wrapper_change` — mount `<App />` with no panels open; assert the window-root `[data-debug="layer"]` still renders with non-zero rect (regression guard for Option A's flow → viewport switch on the window layer).

If the layer overlay component lacks a stable per-layer-name selector (currently it carries `data-debug="layer"` only), add `data-layer-name={name}` to the overlay's outer span so the test selectors can target the inspector vs window layer specifically.

Test command: `cd kanban-app/ui && bun test focus-debug-overlay.layer.browser` — all five pass.

### Existing tests must keep passing

- [x] `kanban-app/ui/src/components/focus-debug-overlay.browser.test.tsx` (the existing nine cases, including `layer_renders_no_dom_when_debug_off` — the regression guard for the production-layout-byte-identical-when-debug-off behaviour stays intact).
- [x] `kanban-app/ui/src/components/inspectors-container.spatial-nav.test.tsx`
- [x] `kanban-app/ui/src/components/inspector-dismiss.browser.test.tsx`
- [x] `kanban-app/ui/src/components/focus-layer.test.tsx`

Test command: `cd kanban-app/ui && bun test focus-layer focus-debug-overlay inspectors-container inspector-dismiss` — all green.

## Workflow

- Use `/tdd` — write `inspector_layer_overlay_renders_at_viewport_size` first; it fails today (rect is 0×0). Then apply Option A's wrapper change. Re-run green.
- Verify no descendant code path depends on the FocusLayer's debug wrapper as a containing block — `grep` for any code that walks up to a `[data-debug="layer"]` or relies on the layer's wrapper for positioning. If any consumer is found, switch to a per-layer `displayMode` prop or fall back to Option B.
- Add `data-layer-name={name}` to `<FocusDebugOverlay kind="layer" label={name}>` (or the host span it renders) so tests can target specific layers — required for the new browser test selectors. This is a one-line additive change in `focus-debug-overlay.tsx`.

#frontend #spatial-nav #kanban-app

## Review Findings (2026-05-02 09:27)

### Warnings
- [x] `kanban-app/ui/src/components/focus-debug-overlay.layer.browser.test.tsx:301` — The `inspector_layer_overlay_does_not_intercept_clicks` test invokes `target!.click()`, which dispatches a synthetic click directly to the target element and bypasses CSS hit-testing entirely. The acceptance criterion ("clicks pass through the overlay") is about pointer-event interception, which is a hit-testing property — the current assertion would pass even if the overlay sat on top with `pointer-events: auto`. Strengthen by computing a coordinate inside the panel and asserting `document.elementFromPoint(x, y) === target` (or that `target` is in the `elementsFromPoint(x, y)` stack and is the topmost non-debug element). The existing `column_overlay_does_not_paint_over_inspector_panel` test in `focus-debug-overlay.layer-z.browser.test.tsx:330` uses exactly this `elementsFromPoint` pattern — mirror it here.

  Resolution: rewrote the test to mirror the `elementsFromPoint(x, y)` pattern from the column-overlay regression guard. The test now picks a point well inside the panel rect, asserts the topmost element returned by `document.elementsFromPoint` is the panel (or a descendant) and is NOT a `[data-debug="layer"]` span. The synthetic `.click()` is kept as a belt-and-braces tail check.

### Nits
- [x] `kanban-app/ui/src/components/focus-debug-overlay.tsx:281-292` — The implementation comment claims it preserves "single rendering path," but the layer/zone/scope branching has actually grown: `kind === "layer"` short-circuits in `labelText` (line 236), in the `layerNameAttr` spread (line 268), and now in `overlayInlineStyle` (line 282). Three runtime branches in one render means the "single component" win is mostly cosmetic. Either embrace the divergence with a small `<FocusLayerOverlay>` companion that drops the unused rAF poll and the `hostRef`, or consolidate the three branches into one early `if (kind === "layer") return <LayerOverlay ... />`. Lower priority — the code works; the architectural argument for keeping it together is just thinner than the comments imply.

  Resolution: factored a `<FocusLayerOverlay>` companion component that owns the layer overlay's viewport-sized host, dashed border, and label-handle. `<FocusDebugOverlay>` now early-returns to it via `if (kind === "layer") return <FocusLayerOverlay name={label} />`. The hover handle + tooltip are extracted into a shared `<OverlayHandle>` so layer and zone/scope overlays share one code path for the click-stop / pointer-events / TooltipProvider conventions. `<FocusLayer>` no longer wraps the overlay in a host div — it imports `<FocusLayerOverlay>` directly and renders it as a sibling of `children`. The three runtime branches that previously short-circuited inside `<FocusDebugOverlay>` collapse to a single early `if`.

- [x] `kanban-app/ui/src/components/focus-layer.tsx:271-281` and `focus-debug-overlay.tsx:281-291`, `focus-debug-overlay.tsx:336-358` — The Tailwind classes (`fixed inset-0 pointer-events-none`, `absolute inset-0`, etc.) are duplicated by inline `style` objects. Documented as a deliberate vitest-without-Tailwind fallback, but if a future change updates one source and not the other, the drift is silent. Consider extracting the inline-style fallbacks into named constants near the top of each file (e.g. `LAYER_HOST_FALLBACK_STYLE`, `LAYER_OVERLAY_FALLBACK_STYLE`) so a reader can see at a glance that `className` and `style` are intentional pairs, and ESLint/grep can find them together. Alternatively pull the test-env Tailwind plugin in for these tests so the duplication can be deleted.

  Resolution: extracted three named constants near the top of `focus-debug-overlay.tsx` — `LAYER_OVERLAY_HOST_STYLE`, `OVERLAY_BORDER_STYLE`, `HANDLE_BASE_STYLE`. Each is documented with the className chain it pairs with. The `<FocusLayer>` component no longer carries any inline-style fallback at all (the host wrapper that needed one was removed when the layer overlay became a sibling), removing the second duplication site entirely.

- [x] `kanban-app/ui/src/components/focus-debug-overlay.tsx:181-228` — The rAF rect-poll continues running for `kind === "layer"` overlays even though the result is unused (the `labelText` short-circuit at line 236 drops the coordinates, and there is no other consumer for the rect on layer overlays). The task description's Option B explicitly identified this as "wasted work in the layer case." Either skip scheduling the rAF when `kind === "layer"`, or factor a tiny `<LayerOverlay>` that doesn't take a `hostRef` at all. Negligible perf cost in production (debug overlays are off), but the dead code path is misleading when reading the component cold.

  Resolution: `<FocusLayerOverlay>` does not take a `hostRef` and does not run the rAF rect poll. Layer overlays no longer execute the dead code path. `<FocusDebugOverlay>` only schedules the rAF for `kind === "zone"` and `kind === "scope"`, where the rect is actually consumed by the label.