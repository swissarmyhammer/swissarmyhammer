/**
 * Layer-aware z-index tests for `<FocusDebugOverlay>`.
 *
 * Pins task `01KQFHVQP3NDXDRAA1WB4R92DA` — the overlay's z-index must
 * respect the spatial-nav layer hierarchy so that:
 *
 *   - Window-root overlays sit BELOW the inspector backdrop (z-20) and
 *     SlidePanel (z-30), so the column / card / perspective overlays do
 *     not bleed across the open inspector.
 *   - Inspector-layer overlays sit ABOVE the SlidePanel content (z-30),
 *     so the inspector's own zones still render their dashed borders.
 *   - Palette-layer overlays sit ABOVE inspector overlays (z >= 60).
 *   - A nested layer with an unrecognised name falls back to
 *     `parentTier + 20` so its overlays stay above the parent's.
 *
 * # Why a synthetic harness instead of `<App />`
 *
 * The task description sketches a test that mounts `<App />` and queries
 * `[data-moniker^="column:"]`. In the live kanban-app codebase
 * `data-moniker` holds the *fully-qualified* moniker
 * (`/window/ui:board/column:todo`), not the bare segment, and `<App />`
 * brings a heavy dependency graph (rust-engine container, schema
 * provider, perspective router, virtualised lists). The architectural
 * contract being tested is "a window-root descendant overlay's
 * computed z-index is below an inspector-mounted descendant overlay's"
 * — that contract holds independent of the rest of the app. A focused
 * harness composed of `<FocusLayer name="window">` containing a column
 * zone, plus an `<InspectorsContainer>`-style inspector-layer block
 * with a panel zone, exercises the same code paths as `<App />` while
 * staying readable and stable. The contract is what matters, not the
 * particular real component that emits the overlay.
 */

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, act } from "@testing-library/react";

// ---------------------------------------------------------------------------
// Tauri API mocks — must come before component imports.
// ---------------------------------------------------------------------------

const mockInvoke = vi.hoisted(() =>
  vi.fn((..._args: unknown[]) => Promise.resolve()),
);

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

vi.mock("@tauri-apps/api/event", () => ({
  emit: vi.fn(() => Promise.resolve()),
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    label: "main",
    listen: vi.fn(() => Promise.resolve(() => {})),
  }),
}));

vi.mock("@tauri-apps/plugin-log", () => ({
  error: vi.fn(),
  warn: vi.fn(),
  info: vi.fn(),
  debug: vi.fn(),
  trace: vi.fn(),
  attachConsole: vi.fn(() => Promise.resolve()),
}));

// ---------------------------------------------------------------------------
// Imports — after mocks
// ---------------------------------------------------------------------------

import type { ReactNode } from "react";
import { FocusScope } from "./focus-scope";
import { FocusLayer } from "./focus-layer";
import { SlidePanel } from "./slide-panel";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { FocusDebugProvider } from "@/lib/focus-debug-context";
import { TooltipProvider } from "@/components/ui/tooltip";
import { asSegment } from "@/types/spatial";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/**
 * Flush microtasks so the spatial-register effects scheduled in
 * `useEffect` have run. Two ticks: first lets the effect callbacks run,
 * second lets any Promise-resolution-driven follow-on settle.
 */
async function flushSetup() {
  await act(async () => {
    await Promise.resolve();
  });
  await act(async () => {
    await Promise.resolve();
  });
}

/**
 * Wait one animation frame and then flush microtasks. The overlay reads
 * its host's rect on `requestAnimationFrame`, so the label only contains
 * coordinates after the first frame.
 */
async function flushFrame() {
  await act(async () => {
    await new Promise<void>((resolve) =>
      requestAnimationFrame(() => resolve()),
    );
    await Promise.resolve();
  });
}

/**
 * Read computed z-index of an element as a number. Returns `NaN` if the
 * value is `auto` or otherwise non-numeric — z-index assertions in this
 * suite always require a concrete number.
 */
function readZIndex(el: Element): number {
  return Number.parseInt(window.getComputedStyle(el).zIndex, 10);
}

/**
 * Resolve the first `[data-debug=…]` overlay descendant of an element
 * matching the given test-id. Throws if not found.
 */
function getOverlayUnder(
  container: HTMLElement,
  testId: string,
  kind: "layer" | "zone" | "scope",
): HTMLElement {
  const host = container.querySelector(`[data-testid="${testId}"]`);
  if (!host) throw new Error(`host with data-testid="${testId}" not found`);
  const overlay = host.querySelector(`[data-debug="${kind}"]`);
  if (!overlay) {
    throw new Error(
      `[data-debug="${kind}"] not found under data-testid="${testId}"`,
    );
  }
  return overlay as HTMLElement;
}

/**
 * Wrap a tree in `<TooltipProvider>` so the hover-handle tooltip in
 * `<FocusDebugOverlay>` has the Radix context it requires. Production
 * supplies this via `<WindowContainer>`'s `<TooltipProvider>`; the
 * synthetic harnesses in this suite mount overlays outside that
 * provider hierarchy.
 *
 * `delayDuration={0}` is fine here because none of the layer-z tests
 * exercise hover; the wrapper exists purely so the Tooltip primitives
 * render without throwing.
 */
function withTooltipProvider(children: ReactNode) {
  return <TooltipProvider delayDuration={0}>{children}</TooltipProvider>;
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("<FocusDebugOverlay> — layer-aware z-index", () => {
  beforeEach(() => {
    mockInvoke.mockClear();
  });

  afterEach(() => {
    document.body.innerHTML = "";
  });

  it("window_layer_overlay_z_index_is_below_inspector_backdrop", async () => {
    // Mount a window-layer column zone alongside an inspector layer with
    // a panel zone. The window-layer column zone's overlay must have
    // z-index < 20 so it cannot paint over the inspector backdrop
    // (z-20 in `inspectors-container.tsx`).
    const { container, unmount } = render(
      withTooltipProvider(
        <FocusDebugProvider enabled>
          <SpatialFocusProvider>
            <FocusLayer name={asSegment("window")}>
              <div data-testid="column-host">
                <FocusScope moniker={asSegment("column:todo")}>
                  <span>column body</span>
                </FocusScope>
              </div>
              <FocusLayer name={asSegment("inspector")}>
                <FocusScope moniker={asSegment("task:T1")}>
                  <span>panel body</span>
                </FocusScope>
              </FocusLayer>
            </FocusLayer>
          </SpatialFocusProvider>
        </FocusDebugProvider>,
      ),
    );
    await flushSetup();
    await flushFrame();

    const columnOverlay = getOverlayUnder(container, "column-host", "zone");
    const z = readZIndex(columnOverlay);
    expect(Number.isFinite(z)).toBe(true);
    expect(z).toBeLessThan(20);

    unmount();
  });

  it("inspector_layer_overlay_z_index_is_above_slide_panel", async () => {
    // Same shape — assert that the panel zone's overlay sits above the
    // SlidePanel content (z-30).
    const { container, unmount } = render(
      withTooltipProvider(
        <FocusDebugProvider enabled>
          <SpatialFocusProvider>
            <FocusLayer name={asSegment("window")}>
              <div data-testid="column-host">
                <FocusScope moniker={asSegment("column:todo")}>
                  <span>column body</span>
                </FocusScope>
              </div>
              <FocusLayer name={asSegment("inspector")}>
                <div data-testid="panel-host">
                  <FocusScope moniker={asSegment("task:T1")}>
                    <span>panel body</span>
                  </FocusScope>
                </div>
              </FocusLayer>
            </FocusLayer>
          </SpatialFocusProvider>
        </FocusDebugProvider>,
      ),
    );
    await flushSetup();
    await flushFrame();

    const panelOverlay = getOverlayUnder(container, "panel-host", "zone");
    const z = readZIndex(panelOverlay);
    expect(Number.isFinite(z)).toBe(true);
    expect(z).toBeGreaterThan(30);

    unmount();
  });

  it("column_overlay_does_not_paint_over_inspector_panel", async () => {
    // The user-reported symptom: a column overlay's blue dashed border
    // crosses into the open inspector area. Pin that the topmost
    // element at a point inside the SlidePanel's geometry is the
    // SlidePanel (or one of its descendants) — NOT a `[data-debug]`
    // span owned by a window-layer ancestor.
    //
    // Synthetic geometry: render a column overlay that fills the
    // viewport-left half and a SlidePanel-like fixed div on the right.
    // After the fix, the column's overlay z-index drops below the
    // SlidePanel z-30 so the panel wins at points inside the panel.
    //
    // Note on the inlined `zIndex: 30`: the value is a duplicate of
    // `slide-panel.tsx`'s real `z-30` class. JSDOM does not load
    // Tailwind's stylesheet, so `<SlidePanel>`'s computed z-index is
    // `auto` in this environment — making it useless as a stacking
    // baseline for `elementsFromPoint`. The inline number stays here
    // for the stacking math; the next test
    // (`real_slide_panel_still_uses_z_30_class`) pins the real
    // component's class so this duplicated number cannot silently
    // drift out of sync.
    const { container, unmount } = render(
      withTooltipProvider(
        <FocusDebugProvider enabled>
          <SpatialFocusProvider>
            <FocusLayer name={asSegment("window")}>
              <div data-testid="column-host">
                <FocusScope
                  moniker={asSegment("column:todo")}
                  style={{
                    position: "fixed",
                    left: 0,
                    top: 0,
                    width: "100%",
                    height: "100%",
                  }}
                >
                  <span>column body fills viewport</span>
                </FocusScope>
              </div>
              <div
                data-testid="backdrop"
                style={{
                  position: "fixed",
                  inset: 0,
                  zIndex: 20,
                  background: "rgba(0,0,0,0.2)",
                }}
              />
              <div
                data-testid="slide-panel"
                style={{
                  position: "fixed",
                  top: 0,
                  right: 0,
                  zIndex: 30,
                  width: "420px",
                  height: "100%",
                  background: "white",
                }}
              >
                <FocusLayer name={asSegment("inspector")}>
                  <FocusScope moniker={asSegment("task:T1")}>
                    <span>panel body</span>
                  </FocusScope>
                </FocusLayer>
              </div>
            </FocusLayer>
          </SpatialFocusProvider>
        </FocusDebugProvider>,
      ),
    );
    await flushSetup();
    await flushFrame();

    const slidePanel = container.querySelector(
      '[data-testid="slide-panel"]',
    ) as HTMLElement;
    expect(slidePanel).toBeTruthy();
    const rect = slidePanel.getBoundingClientRect();
    // Pick a point well inside the panel — 10px right of its left edge
    // and at vertical mid-screen. `elementsFromPoint` returns the stack
    // top-down; the topmost element must be the SlidePanel or one of
    // its descendants.
    const x = rect.left + 10;
    const y = window.innerHeight / 2;
    const stack = document.elementsFromPoint(x, y);
    expect(stack.length).toBeGreaterThan(0);
    const topmost = stack[0];

    // The topmost element must be the SlidePanel itself or a descendant
    // of it — NOT a `[data-debug]` span owned by a window-layer
    // ancestor (i.e. by the column-host outside the panel).
    const isPanelOrDescendant =
      topmost === slidePanel || slidePanel.contains(topmost);
    const isWindowLayerDebug =
      topmost instanceof HTMLElement &&
      topmost.hasAttribute("data-debug") &&
      // Defensive: the panel's own debug overlay is allowed; only flag
      // ones that are NOT inside the SlidePanel.
      !slidePanel.contains(topmost);
    expect(isWindowLayerDebug).toBe(false);
    expect(isPanelOrDescendant).toBe(true);

    unmount();
  });

  it("real_slide_panel_still_uses_z_30_class", async () => {
    // Drift-pin: the previous test's stacking math assumes
    // SlidePanel's z-index is 30. That number is duplicated as an
    // inline `zIndex: 30` in the synthetic harness because JSDOM does
    // not load Tailwind, so the real component's `z-30` class produces
    // no computed stacking-context z-index in tests. To keep the
    // duplicated number from silently falling out of sync with the
    // production component, this test mounts the actual `<SlidePanel>`
    // and asserts it still declares `z-30` on its root `<div>`.
    //
    // If `slide-panel.tsx` is ever changed to a different z-tier
    // (e.g. `z-40`), this test fails immediately and the author knows
    // to:
    //   1. Update the inline `zIndex: 30` in
    //      `column_overlay_does_not_paint_over_inspector_panel`.
    //   2. Update the inspector tier in `LAYER_Z_TIERS`
    //      (`focus-layer.tsx`) so the inspector overlay still sits
    //      above the new SlidePanel z-index.
    //   3. Update the regex in this test to match the new class.
    const { container, unmount } = render(
      <SlidePanel open onClose={() => {}}>
        <span>panel body</span>
      </SlidePanel>,
    );
    await flushSetup();

    // `<SlidePanel>` renders a single fixed root `<div>` whose
    // className carries the z-tier. It is the only top-level element
    // in the rendered output.
    const root = container.firstElementChild as HTMLElement;
    expect(root).toBeTruthy();
    expect(root.className).toMatch(/\bz-30\b/);

    unmount();
  });

  it("palette_overlay_z_index_is_above_inspector_overlay", async () => {
    // Synthetic stack: window → inspector → palette. The palette's
    // overlay must sit above the inspector's overlay. The fix's
    // tier table sets palette = 60, inspector = 30, so the gap is
    // generous (palette overlays at 65, inspector overlays at 35).
    const { container, unmount } = render(
      withTooltipProvider(
        <FocusDebugProvider enabled>
          <SpatialFocusProvider>
            <FocusLayer name={asSegment("window")}>
              <FocusLayer name={asSegment("inspector")}>
                <div data-testid="panel-host">
                  <FocusScope moniker={asSegment("task:T1")}>
                    <span>panel body</span>
                  </FocusScope>
                </div>
                <FocusLayer name={asSegment("palette")}>
                  <div data-testid="palette-host">
                    <FocusScope moniker={asSegment("ui:command-palette")}>
                      <span>palette body</span>
                    </FocusScope>
                  </div>
                </FocusLayer>
              </FocusLayer>
            </FocusLayer>
          </SpatialFocusProvider>
        </FocusDebugProvider>,
      ),
    );
    await flushSetup();
    await flushFrame();

    const inspectorOverlay = getOverlayUnder(container, "panel-host", "zone");
    const paletteOverlay = getOverlayUnder(container, "palette-host", "zone");

    const inspectorZ = readZIndex(inspectorOverlay);
    const paletteZ = readZIndex(paletteOverlay);

    expect(Number.isFinite(inspectorZ)).toBe(true);
    expect(Number.isFinite(paletteZ)).toBe(true);
    expect(paletteZ).toBeGreaterThanOrEqual(60);
    expect(inspectorZ).toBeLessThan(60);

    unmount();
  });

  it("layer_kind_overlay_reads_its_own_layer_tier", async () => {
    // The `kind="layer"` overlay is rendered by `<FocusLayer>` itself
    // (see `focus-layer.tsx`'s debug-mode wrapper) and reads the same
    // `FocusLayerZTierContext` value that descendant `kind="zone"` and
    // `kind="scope"` overlays read. The other tests in this suite
    // exercise the zone path; this one closes the gap on the
    // layer-kind overlay so a regression that decoupled the layer's
    // own decorator from its tier (e.g. a stray hardcoded z-index in
    // the layer-kind branch) cannot pass review.
    //
    // Inspector tier is 30, overlay offset is 5, so the layer-kind
    // overlay rendered by the inspector layer must compute to z-35.
    const { container, unmount } = render(
      withTooltipProvider(
        <FocusDebugProvider enabled>
          <SpatialFocusProvider>
            <FocusLayer name={asSegment("window")}>
              <div data-testid="inspector-host">
                <FocusLayer name={asSegment("inspector")}>
                  <span>inspector body</span>
                </FocusLayer>
              </div>
            </FocusLayer>
          </SpatialFocusProvider>
        </FocusDebugProvider>,
      ),
    );
    await flushSetup();
    await flushFrame();

    // The inspector layer's own `kind="layer"` decorator lives inside
    // the wrapper `<div className="relative">` that `<FocusLayer>`
    // renders in debug mode. Find the deepest `[data-debug="layer"]`
    // under the inspector host — i.e. the inspector's own decorator
    // (the window layer's own decorator is also `[data-debug="layer"]`
    // but sits outside the inspector-host container).
    const overlay = getOverlayUnder(container, "inspector-host", "layer");
    const z = readZIndex(overlay);
    // Inspector tier 30 + offset 5 = 35.
    expect(z).toBe(35);

    unmount();
  });

  it("nested_unnamed_layer_falls_through_to_parent_plus_twenty", async () => {
    // Custom layer name not in `LAYER_Z_TIERS` — the tier should
    // resolve via the `parentTier + 20` fallback. Mount inspector
    // (tier 30) → custom (tier 50 = 30 + 20). The custom layer's
    // descendant overlay should sit at exactly 50 + 5 = 55.
    const { container, unmount } = render(
      withTooltipProvider(
        <FocusDebugProvider enabled>
          <SpatialFocusProvider>
            <FocusLayer name={asSegment("window")}>
              <FocusLayer name={asSegment("inspector")}>
                <FocusLayer name={asSegment("custom-unknown-layer")}>
                  <div data-testid="custom-host">
                    <FocusScope moniker={asSegment("ui:custom")}>
                      <span>custom body</span>
                    </FocusScope>
                  </div>
                </FocusLayer>
              </FocusLayer>
            </FocusLayer>
          </SpatialFocusProvider>
        </FocusDebugProvider>,
      ),
    );
    await flushSetup();
    await flushFrame();

    const customOverlay = getOverlayUnder(container, "custom-host", "zone");
    const z = readZIndex(customOverlay);
    // inspector tier 30 → custom tier 30 + 20 = 50 → overlay 50 + 5 = 55.
    expect(z).toBe(55);

    unmount();
  });
});
