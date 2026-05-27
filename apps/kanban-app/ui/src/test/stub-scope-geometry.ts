/**
 * Test helper: stub BOTH geometry primitives the Jump-To overlay consults so
 * synthetic (no-real-layout) scope tests stay deterministic.
 *
 * The Jump-To overlay reads scope geometry two ways:
 *
 *   1. `Element.getBoundingClientRect()` — the rect each pill is positioned at
 *      and the rect enumeration reports.
 *   2. `document.elementFromPoint(x, y)` — the visibility / occlusion hit-test
 *      `useJumpTargets` runs at each scope's pill anchor (`rect.left + 4`,
 *      `rect.top + 4`) to drop off-screen / occluded scopes (vim-sneak / AceJump
 *      "you can only jump to what you can see" semantics).
 *
 * Synthetic tests pin each scope's rect to a fabricated value keyed by
 * `data-testid` so they can assert enumeration / matching logic without a real
 * Tailwind layout. But a fabricated rect does NOT move the real DOM element, so
 * a real `elementFromPoint` at the fabricated anchor would land on whatever
 * actually paints there (usually nothing, or the first stacked element) and the
 * visibility filter would wrongly drop the scope. This helper keeps the two
 * primitives consistent: `elementFromPoint` returns the host element whose
 * fabricated rect contains the queried point, so a scope the test positioned
 * on-screen also passes the hit-test.
 *
 * Use this in place of a bare `getBoundingClientRect` stub in any test that
 * fabricates scope rects AND opens the Jump-To overlay.
 */

/** Build a `DOMRect`-shaped object from `(x, y, w, h)`. */
export function mkRect(x: number, y: number, w: number, h: number): DOMRect {
  return {
    x,
    y,
    left: x,
    top: y,
    width: w,
    height: h,
    right: x + w,
    bottom: y + h,
    toJSON: () => ({}),
  } as DOMRect;
}

/**
 * Install consistent `getBoundingClientRect` + `elementFromPoint` stubs for the
 * scopes named by the `rects` map (keyed by `data-testid`).
 *
 * - `getBoundingClientRect` returns the fabricated rect for any element whose
 *   `data-testid` is in the map, falling through to the real implementation
 *   otherwise.
 * - `elementFromPoint` returns the host element (the `data-testid` node) whose
 *   fabricated rect contains the queried point. When several overlap, the last
 *   one in iteration (Map insertion) order wins. This stub is only valid for
 *   fixtures whose rects do NOT overlap (the grids these tests build); for
 *   overlapping rects its insertion-order tie-break is NOT guaranteed to match
 *   the browser's real top-most paint order, so do not rely on it to encode an
 *   occlusion expectation — write such a case against real layout instead.
 *   Falls through to the real implementation when the point is inside none of
 *   the fabricated rects.
 *
 * Returns a cleanup function that restores both prototype / document methods —
 * call it in `afterEach` (or at the end of the test).
 */
export function stubScopeGeometry(rects: Map<string, DOMRect>): () => void {
  const origRect = Element.prototype.getBoundingClientRect;
  const origFromPoint = document.elementFromPoint.bind(document);

  Element.prototype.getBoundingClientRect = function () {
    const testId = (this as HTMLElement).dataset?.testid;
    if (testId !== undefined && rects.has(testId)) {
      return rects.get(testId)!;
    }
    return origRect.call(this);
  };

  document.elementFromPoint = ((x: number, y: number): Element | null => {
    let match: Element | null = null;
    for (const [testId, r] of rects) {
      if (x >= r.left && x < r.right && y >= r.top && y < r.bottom) {
        const host = document.querySelector(`[data-testid="${testId}"]`);
        if (host !== null) match = host;
      }
    }
    if (match !== null) return match;
    return origFromPoint(x, y);
  }) as typeof document.elementFromPoint;

  return () => {
    Element.prototype.getBoundingClientRect = origRect;
    document.elementFromPoint = origFromPoint;
  };
}
