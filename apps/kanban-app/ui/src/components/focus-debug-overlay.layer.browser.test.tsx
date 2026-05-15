/**
 * Browser-mode tests for the spatial-nav layer debug overlay's wrapper
 * geometry.
 *
 * Pins task `01KQCHZW5R0WJXTP4BG67QE0Z7`. The original `<FocusLayer>`
 * debug wrapper was `<div className="relative">`; for the inspector
 * layer (whose only DOM children are `position: fixed` SlidePanels),
 * that wrapper collapsed to 0×0 because fixed-position children
 * contribute nothing to flow layout. The dashed-border overlay then
 * rendered at zero size with its label pinned to the wrapper's
 * top-left corner — visually invisible.
 *
 * The fix (Option A in the task) switches the debug-mode wrapper to
 * `position: fixed; inset: 0` (with `pointer-events: none` so it
 * doesn't intercept clicks), giving the wrapper a real, viewport-sized
 * box that the dashed border + label can paint against. The
 * `pointer-events: none` lets clicks, drags, and hovers pass through
 * to the panels and the underlying board unchanged.
 *
 * # Test coverage
 *
 *   1. `inspector_layer_overlay_renders_at_viewport_size` — the
 *      dashed-border overlay under the inspector `<FocusLayer>` has a
 *      bounding rect that spans the full viewport.
 *   2. `inspector_layer_overlay_label_includes_layer_name` — the
 *      tooltip handle's `aria-label` (which mirrors the tooltip text)
 *      contains `layer:inspector`.
 *   3. `inspector_layer_overlay_does_not_intercept_clicks` — clicking
 *      a host span behind the overlay reaches the host's `onClick`
 *      (the wrapper's `pointer-events: none` is honoured).
 *   4. `inspector_layer_overlay_unmounts_when_layer_unmounts` —
 *      removing the inspector layer (e.g. when the last panel closes)
 *      removes the inspector layer overlay from the DOM.
 *   5. `window_layer_overlay_still_renders_after_wrapper_change` —
 *      regression guard for the window-layer overlay; switching its
 *      wrapper from `relative` to `fixed inset-0` must not break the
 *      window layer overlay registration.
 *
 * The harness mounts a synthetic `<FocusLayer name="window">` with a
 * nested `<FocusLayer name="inspector">` (the production composition
 * from `<InspectorsContainer>`). Mounting `<App />` per the task
 * sketch is over-broad: the architectural contract being tested is
 * "the layer wrapper has a real viewport-sized box," which holds at
 * any composition level. A focused harness keeps the test stable
 * against unrelated app churn.
 */

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, act } from "@testing-library/react";
import type { ReactNode } from "react";

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

import { FocusLayer } from "./focus-layer";
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
 * Wrap a tree in `<TooltipProvider>` so the hover-handle tooltip in
 * `<FocusDebugOverlay>` has the Radix context it requires. Production
 * supplies this via `<WindowContainer>`'s `<TooltipProvider>`; the
 * synthetic harnesses in this suite mount overlays outside that
 * provider hierarchy.
 */
function withTooltipProvider(children: ReactNode) {
  return <TooltipProvider delayDuration={0}>{children}</TooltipProvider>;
}

/**
 * Resolve the layer overlay wrapper for a specific `data-layer-name`.
 *
 * `<FocusDebugOverlay>` carries `data-debug="layer"` on its outer span,
 * but multiple layer overlays exist concurrently when both the window
 * and inspector layers are mounted. `data-layer-name` (added in this
 * task) is the per-layer selector tests use to disambiguate.
 */
function getLayerOverlay(
  container: HTMLElement,
  layerName: string,
): HTMLElement | null {
  return container.querySelector<HTMLElement>(
    `[data-debug="layer"][data-layer-name="${layerName}"]`,
  );
}

/**
 * Read the layer overlay's hover-handle `aria-label`. The handle's
 * `aria-label` mirrors the tooltip content verbatim, so this is the
 * deterministic way to assert the visible label without going through a
 * hover round-trip.
 */
function readLayerOverlayLabel(
  container: HTMLElement,
  layerName: string,
): string | null {
  const overlay = getLayerOverlay(container, layerName);
  if (!overlay) return null;
  const handle = overlay.querySelector<HTMLElement>(
    '[data-debug-handle="layer"]',
  );
  return handle?.getAttribute("aria-label") ?? null;
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("<FocusLayer> debug wrapper — viewport-sized geometry", () => {
  beforeEach(() => {
    mockInvoke.mockClear();
  });

  afterEach(() => {
    document.body.innerHTML = "";
  });

  it("inspector_layer_overlay_renders_at_viewport_size", async () => {
    // Mount a window layer with a nested inspector layer. The
    // inspector layer's child is a fixed-position panel (a stand-in for
    // a real `<SlidePanel>`); without the wrapper fix, the inspector
    // layer's debug wrapper would collapse to 0×0 because its only
    // child is out of flow. After the fix, the wrapper is `fixed
    // inset-0` and its bounding rect spans the viewport.
    const { container, unmount } = render(
      withTooltipProvider(
        <FocusDebugProvider enabled>
          <SpatialFocusProvider>
            <FocusLayer name={asSegment("window")}>
              <FocusLayer name={asSegment("inspector")}>
                <div
                  data-testid="fake-panel"
                  style={{
                    position: "fixed",
                    top: 0,
                    right: 0,
                    width: 420,
                    height: "100%",
                  }}
                >
                  panel content
                </div>
              </FocusLayer>
            </FocusLayer>
          </SpatialFocusProvider>
        </FocusDebugProvider>,
      ),
    );
    await flushSetup();
    await flushFrame();

    const overlay = getLayerOverlay(container, "inspector");
    expect(overlay, "inspector layer overlay must render").toBeTruthy();

    const rect = overlay!.getBoundingClientRect();
    expect(
      rect.width,
      "inspector layer overlay rect must span viewport width",
    ).toBe(window.innerWidth);
    expect(
      rect.height,
      "inspector layer overlay rect must span viewport height",
    ).toBe(window.innerHeight);
    expect(rect.x, "overlay must be at viewport origin x").toBe(0);
    expect(rect.y, "overlay must be at viewport origin y").toBe(0);

    unmount();
  });

  it("inspector_layer_overlay_label_includes_layer_name", async () => {
    const { container, unmount } = render(
      withTooltipProvider(
        <FocusDebugProvider enabled>
          <SpatialFocusProvider>
            <FocusLayer name={asSegment("window")}>
              <FocusLayer name={asSegment("inspector")}>
                <span>panel content</span>
              </FocusLayer>
            </FocusLayer>
          </SpatialFocusProvider>
        </FocusDebugProvider>,
      ),
    );
    await flushSetup();
    await flushFrame();

    const label = readLayerOverlayLabel(container, "inspector");
    expect(label).toBe("layer:inspector");

    unmount();
  });

  it("inspector_layer_overlay_does_not_intercept_clicks", async () => {
    // The acceptance criterion is "clicks pass through the overlay,"
    // which is a hit-testing property. A synthetic `target.click()`
    // would bypass CSS hit-testing entirely (the event is dispatched
    // straight at the target regardless of what is on top), so the
    // assertion has to go through `document.elementsFromPoint(x, y)` —
    // the same pattern used by the
    // `column_overlay_does_not_paint_over_inspector_panel` regression
    // guard in `focus-debug-overlay.layer-z.browser.test.tsx`.
    //
    // We mount a panel-shaped host at a known position inside the
    // viewport, pick a point well inside it, and assert the topmost
    // element at that point is the panel (or a descendant), NOT the
    // layer-debug overlay. The synthetic `.click()` is kept as a
    // belt-and-braces check that the click handler still fires once
    // we've established hit-testing reaches the panel.
    const onClick = vi.fn();
    const { container, unmount } = render(
      withTooltipProvider(
        <FocusDebugProvider enabled>
          <SpatialFocusProvider>
            <FocusLayer name={asSegment("window")}>
              <FocusLayer name={asSegment("inspector")}>
                <div
                  data-testid="click-target"
                  onClick={onClick}
                  style={{
                    position: "fixed",
                    top: 0,
                    right: 0,
                    width: 420,
                    height: "100%",
                    background: "white",
                  }}
                >
                  panel content
                </div>
              </FocusLayer>
            </FocusLayer>
          </SpatialFocusProvider>
        </FocusDebugProvider>,
      ),
    );
    await flushSetup();
    await flushFrame();

    const target = container.querySelector<HTMLElement>(
      '[data-testid="click-target"]',
    );
    expect(target).toBeTruthy();

    // Pick a point well inside the panel — 10px right of its left edge
    // at vertical mid-screen — so we're not at any edge where rounding
    // could put us outside the panel rect.
    const rect = target!.getBoundingClientRect();
    const x = rect.left + 10;
    const y = window.innerHeight / 2;
    const stack = document.elementsFromPoint(x, y);
    expect(stack.length).toBeGreaterThan(0);

    // The topmost element at that point must be the panel itself or a
    // descendant — NOT a layer-debug overlay span. If the overlay's
    // `pointer-events: none` is honoured, hit-testing skips it
    // entirely and the panel wins. If we regressed to
    // `pointer-events: auto` (the original 0×0 bug-fix attempt that
    // broke clicks), the overlay would land on top here.
    const topmost = stack[0];
    const isPanelOrDescendant = topmost === target || target!.contains(topmost);
    const isLayerDebugOverlay =
      topmost instanceof HTMLElement &&
      topmost.getAttribute("data-debug") === "layer";
    expect(
      isLayerDebugOverlay,
      "layer overlay must not sit on top of panel content",
    ).toBe(false);
    expect(
      isPanelOrDescendant,
      "panel content must be the topmost hit-tested element",
    ).toBe(true);

    // Belt-and-braces: with hit-testing pointing at the panel, the
    // click handler still fires when invoked.
    target!.click();
    expect(onClick).toHaveBeenCalledTimes(1);

    unmount();
  });

  it("inspector_layer_overlay_unmounts_when_layer_unmounts", async () => {
    // The inspector layer mounts conditionally in production (only
    // while at least one panel is open). When the last panel closes,
    // the layer unmounts, and its debug overlay must disappear with
    // it. Toggle the layer with React state and assert the overlay
    // selector finds nothing afterwards.
    function Harness({ showInspector }: { showInspector: boolean }) {
      return withTooltipProvider(
        <FocusDebugProvider enabled>
          <SpatialFocusProvider>
            <FocusLayer name={asSegment("window")}>
              {showInspector ? (
                <FocusLayer name={asSegment("inspector")}>
                  <span>panel content</span>
                </FocusLayer>
              ) : null}
            </FocusLayer>
          </SpatialFocusProvider>
        </FocusDebugProvider>,
      );
    }

    const { container, rerender, unmount } = render(
      <Harness showInspector={true} />,
    );
    await flushSetup();
    await flushFrame();

    expect(getLayerOverlay(container, "inspector")).toBeTruthy();

    rerender(<Harness showInspector={false} />);
    await flushSetup();
    await flushFrame();

    expect(
      getLayerOverlay(container, "inspector"),
      "inspector layer overlay must disappear when the layer unmounts",
    ).toBeNull();

    unmount();
  });

  it("window_layer_overlay_still_renders_after_wrapper_change", async () => {
    // Regression guard for the wrapper switch: the window-root layer
    // overlay must still register and render after the `relative` →
    // `fixed inset-0` change. Its rect is now viewport-sized, which is
    // acceptable per the layer-has-no-rect data model — what matters
    // is that the overlay element exists and its rect is non-zero.
    const { container, unmount } = render(
      withTooltipProvider(
        <FocusDebugProvider enabled>
          <SpatialFocusProvider>
            <FocusLayer name={asSegment("window")}>
              <span>app content</span>
            </FocusLayer>
          </SpatialFocusProvider>
        </FocusDebugProvider>,
      ),
    );
    await flushSetup();
    await flushFrame();

    const overlay = getLayerOverlay(container, "window");
    expect(overlay, "window layer overlay must render").toBeTruthy();

    const rect = overlay!.getBoundingClientRect();
    expect(rect.width, "window layer rect must be non-zero width").toBe(
      window.innerWidth,
    );
    expect(rect.height, "window layer rect must be non-zero height").toBe(
      window.innerHeight,
    );

    unmount();
  });
});
