---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffdd80
project: spatial-nav
title: 'Focus debug overlay: drop width × height from label, keep only (x, y)'
---
## What

The focus debug overlay (`kanban-app/ui/src/components/focus-debug-overlay.tsx:192`) currently renders labels in the form:

```
zone:column:01ABC (120,80) 320×640
```

The trailing `320×640` is width × height. **It was not requested and is visual noise** — the original ask was a small-font x/y coordinate to verify focus is registering at the right position. Drop the dimensions from the label, keep only the coordinate pair:

```
zone:column:01ABC (120,80)
```

Layers stay unchanged (`layer:window`, no rect — layers don't have a meaningful rect).

## What changes

`kanban-app/ui/src/components/focus-debug-overlay.tsx`:

- Line 192 — change the label format from `${kind}:${label} (${Math.round(rect.x)},${Math.round(rect.y)}) ${Math.round(rect.width)}×${Math.round(rect.height)}` to `${kind}:${label} (${Math.round(rect.x)},${Math.round(rect.y)})`.
- Lines 162–165 — drop the `width` / `height` legs of the rect-equality short-circuit. Now that those values aren't displayed, comparing on them produces unnecessary overlay re-renders when the host's dimensions change but its position doesn't. Keep the comparison on `x` and `y` only.

The internal `rect` state can still hold width/height (the kernel uses them — they come from `getBoundingClientRect()`); they're simply no longer used for either the visible label or the re-render gate.

## Acceptance Criteria

All asserted by automated tests below.

- [x] The rendered debug-overlay label for a zone or scope shows the kind, the moniker, and the `(x, y)` pair — and no width × height suffix.
- [x] The rendered debug-overlay label for a layer shows `layer:<name>` and nothing else (unchanged).
- [x] Resizing the host element without moving it (e.g. content reflow that changes width without changing top-left) does NOT cause the overlay's React component to re-render or re-set its `rect` state. (Optimization side-effect of dropping the w/h legs of the equality check.)

## Tests

### Frontend — update `kanban-app/ui/src/components/focus-debug-overlay.browser.test.tsx`

- [x] `zone_label_has_no_dimensions_suffix` — render `<FocusDebugOverlay kind="zone" label="ui:test" hostRef={refToFixedRect}>` against a host at known `(x, y, w, h) = (10, 20, 100, 50)`; assert the rendered label text is exactly `zone:ui:test (10,20)` and does NOT contain `100×50` or `100x50`.
- [x] `scope_label_has_no_dimensions_suffix` — same with `kind="scope"`.
- [x] `layer_label_unchanged` — `<FocusDebugOverlay kind="layer" label="window" ... />` renders `layer:window` exactly. Regression guard.
- [x] `overlay_does_not_rerender_on_pure_dimension_change` — render the overlay, capture its render count via a probe; mutate the host's width and height but keep its top-left fixed; await an animation frame; assert the overlay did not commit a new render. (Pins the side-effect that drops w/h from the equality short-circuit.)

Test command: `bun run test:browser focus-debug-overlay.browser.test.tsx` — all four pass alongside the existing tests in that file.

## Workflow

- Use `/tdd` — update the existing browser test first to expect labels without dimensions, watch it fail, edit the format string, watch it pass.
- One file touched (`focus-debug-overlay.tsx`), one test file updated. Tiny fix.

## Implementation Notes

- Updated `kanban-app/ui/src/components/focus-debug-overlay.tsx`: dropped width/height from the label format (was line 192) and from the equality short-circuit (was lines 162-165).
- Updated `kanban-app/ui/src/components/focus-debug-overlay.browser.test.tsx`: added 4 new tests as specified, plus an `OverlayHarness` helper that direct-mounts `<FocusDebugOverlay>` against a fixed-position host. The render-count probe uses React's `<Profiler>` to assert no commits land after a pure dimension change.
- All 13 tests in `focus-debug-overlay.browser.test.tsx` pass. Full UI suite (1842 tests) green.
- TDD followed: 3 of 4 new tests went RED first (the layer test was always green since layers omit the rect entirely), then GREEN after the implementation edit.