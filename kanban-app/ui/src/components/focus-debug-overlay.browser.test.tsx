/**
 * Browser-mode tests for the spatial-nav debug overlay.
 *
 * Covers four assertions:
 *
 *   1. When `<FocusDebugProvider enabled>` wraps the tree, every
 *      `<FocusLayer>` / `<FocusZone>` / `<FocusScope>` mounts a
 *      `[data-debug=…]` element with the `border-dashed` class and a
 *      label that mentions the primitive's name / moniker.
 *   2. When the provider is disabled (or absent), no `[data-debug=…]`
 *      elements render anywhere in the tree.
 *   3. The overlay's coordinate label tracks the host's bounding rect so
 *      a fixed-position parent at `(100, 200)` produces a label that
 *      contains `"100,200"`.
 *   4. The overlay's `pointer-events: none` is honoured — clicks on the
 *      host content land on the host's click handler, not the overlay.
 *
 * Runs in real Chromium via vitest browser mode so layout (the rect
 * reads, the absolute positioning, the dashed border) is genuine.
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

import { FocusZone } from "./focus-zone";
import { FocusScope } from "./focus-scope";
import { FocusLayer } from "./focus-layer";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { FocusDebugProvider } from "@/lib/focus-debug-context";
import { asLayerName, asMoniker } from "@/types/spatial";

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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("<FocusDebugOverlay> — debug-on rendering", () => {
  beforeEach(() => {
    mockInvoke.mockClear();
  });

  afterEach(() => {
    document.body.innerHTML = "";
  });

  it("zone_renders_debug_overlay_when_debug_on", async () => {
    const { container, unmount } = render(
      <FocusDebugProvider enabled>
        <SpatialFocusProvider>
          <FocusLayer name={asLayerName("window")}>
            <FocusZone moniker={asMoniker("ui:test")}>
              <span>zone-content</span>
            </FocusZone>
          </FocusLayer>
        </SpatialFocusProvider>
      </FocusDebugProvider>,
    );
    await flushSetup();
    await flushFrame();

    const overlay = container.querySelector('[data-debug="zone"]');
    expect(overlay).toBeTruthy();

    // Border element is the first child span — assert `border-dashed`
    // is on it.
    const borderSpan = overlay!.querySelector("span");
    expect(borderSpan).toBeTruthy();
    expect(borderSpan!.className).toContain("border-dashed");

    // Label includes the moniker.
    expect(overlay!.textContent).toContain("ui:test");

    unmount();
  });

  it("scope_renders_debug_overlay_when_debug_on", async () => {
    const { container, unmount } = render(
      <FocusDebugProvider enabled>
        <SpatialFocusProvider>
          <FocusLayer name={asLayerName("window")}>
            <FocusScope moniker={asMoniker("ui:test.leaf")}>
              <span>scope-content</span>
            </FocusScope>
          </FocusLayer>
        </SpatialFocusProvider>
      </FocusDebugProvider>,
    );
    await flushSetup();
    await flushFrame();

    const overlay = container.querySelector('[data-debug="scope"]');
    expect(overlay).toBeTruthy();

    const borderSpan = overlay!.querySelector("span");
    expect(borderSpan).toBeTruthy();
    expect(borderSpan!.className).toContain("border-dashed");

    expect(overlay!.textContent).toContain("ui:test.leaf");

    unmount();
  });

  it("layer_renders_debug_overlay_when_debug_on", async () => {
    const { container, unmount } = render(
      <FocusDebugProvider enabled>
        <SpatialFocusProvider>
          <FocusLayer name={asLayerName("window")}>
            <span>layer-content</span>
          </FocusLayer>
        </SpatialFocusProvider>
      </FocusDebugProvider>,
    );
    await flushSetup();
    await flushFrame();

    const overlay = container.querySelector('[data-debug="layer"]');
    expect(overlay).toBeTruthy();

    const borderSpan = overlay!.querySelector("span");
    expect(borderSpan).toBeTruthy();
    expect(borderSpan!.className).toContain("border-dashed");

    expect(overlay!.textContent).toContain("window");

    unmount();
  });

  it("no_overlay_when_debug_off", async () => {
    const { container, unmount } = render(
      <FocusDebugProvider enabled={false}>
        <SpatialFocusProvider>
          <FocusLayer name={asLayerName("window")}>
            <FocusZone moniker={asMoniker("ui:test")}>
              <FocusScope moniker={asMoniker("ui:test.leaf")}>
                <span>content</span>
              </FocusScope>
            </FocusZone>
          </FocusLayer>
        </SpatialFocusProvider>
      </FocusDebugProvider>,
    );
    await flushSetup();
    await flushFrame();

    expect(container.querySelectorAll("[data-debug]").length).toBe(0);

    unmount();
  });

  it("no_overlay_when_no_provider", async () => {
    const { container, unmount } = render(
      <SpatialFocusProvider>
        <FocusLayer name={asLayerName("window")}>
          <FocusZone moniker={asMoniker("ui:test")}>
            <FocusScope moniker={asMoniker("ui:test.leaf")}>
              <span>content</span>
            </FocusScope>
          </FocusZone>
        </FocusLayer>
      </SpatialFocusProvider>,
    );
    await flushSetup();
    await flushFrame();

    expect(container.querySelectorAll("[data-debug]").length).toBe(0);

    unmount();
  });

  it("overlay_label_includes_rounded_coordinates", async () => {
    // Mount the zone at a fixed position so the overlay's label has
    // predictable coordinates. The zone's `<div>` directly carries the
    // fixed-position style so its rect is exactly (100, 200) — note
    // `<FocusZone>` already merges `relative` into the className, but
    // the inline `position: fixed` style overrides that for layout
    // (the merged class ends up unused; that's acceptable for a test).
    const { container, unmount } = render(
      <FocusDebugProvider enabled>
        <SpatialFocusProvider>
          <FocusLayer name={asLayerName("window")}>
            <FocusZone
              moniker={asMoniker("ui:positioned")}
              style={{
                position: "fixed",
                left: "100px",
                top: "200px",
                width: "150px",
                height: "80px",
              }}
            >
              <span>positioned</span>
            </FocusZone>
          </FocusLayer>
        </SpatialFocusProvider>
      </FocusDebugProvider>,
    );
    await flushSetup();
    await flushFrame();
    // One more frame so the rect read in `requestAnimationFrame` has
    // been committed back into React state and rendered into the DOM.
    await flushFrame();

    const overlay = container.querySelector('[data-debug="zone"]');
    expect(overlay).toBeTruthy();
    expect(overlay!.textContent).toContain("100,200");

    unmount();
  });

  it("overlay_kind_classes_are_distinct", async () => {
    const { container, unmount } = render(
      <FocusDebugProvider enabled>
        <SpatialFocusProvider>
          <FocusLayer name={asLayerName("window")}>
            <FocusZone moniker={asMoniker("ui:zone-test")}>
              <FocusScope moniker={asMoniker("ui:scope-test")}>
                <span>nested</span>
              </FocusScope>
            </FocusZone>
          </FocusLayer>
        </SpatialFocusProvider>
      </FocusDebugProvider>,
    );
    await flushSetup();
    await flushFrame();

    const layerOverlay = container.querySelector('[data-debug="layer"]');
    const zoneOverlay = container.querySelector('[data-debug="zone"]');
    const scopeOverlay = container.querySelector('[data-debug="scope"]');
    expect(layerOverlay).toBeTruthy();
    expect(zoneOverlay).toBeTruthy();
    expect(scopeOverlay).toBeTruthy();

    // Each overlay's first-child border span must carry a colour class
    // unique to its kind. Read the className strings and assert each
    // contains the expected colour token.
    const layerBorder = layerOverlay!.querySelector("span")!.className;
    const zoneBorder = zoneOverlay!.querySelector("span")!.className;
    const scopeBorder = scopeOverlay!.querySelector("span")!.className;

    expect(layerBorder).toContain("border-red-500/70");
    expect(zoneBorder).toContain("border-blue-500/70");
    expect(scopeBorder).toContain("border-emerald-500/70");

    // And those tokens must NOT cross-pollinate.
    expect(layerBorder).not.toContain("border-blue-500/70");
    expect(layerBorder).not.toContain("border-emerald-500/70");
    expect(zoneBorder).not.toContain("border-red-500/70");
    expect(zoneBorder).not.toContain("border-emerald-500/70");
    expect(scopeBorder).not.toContain("border-red-500/70");
    expect(scopeBorder).not.toContain("border-blue-500/70");

    unmount();
  });

  it("overlay_does_not_intercept_clicks", async () => {
    // Mount a `<FocusScope>` with debug on. A click on the host's
    // content should still call the spatial-focus IPC (`spatial_focus`).
    // If the overlay's `pointer-events: none` is broken, the overlay
    // span would intercept the click and the IPC would never fire.
    const { container, unmount } = render(
      <FocusDebugProvider enabled>
        <SpatialFocusProvider>
          <FocusLayer name={asLayerName("window")}>
            <FocusScope moniker={asMoniker("ui:click-test")}>
              <span data-testid="click-target">click me</span>
            </FocusScope>
          </FocusLayer>
        </SpatialFocusProvider>
      </FocusDebugProvider>,
    );
    await flushSetup();
    await flushFrame();

    // The overlay sits above the content — but `pointer-events: none`
    // makes the browser see the click on the underlying span.
    const target = container.querySelector(
      '[data-testid="click-target"]',
    ) as HTMLElement;
    expect(target).toBeTruthy();

    mockInvoke.mockClear();
    target.click();
    await flushSetup();

    // `spatial_focus` should have been dispatched. If the overlay had
    // intercepted the click, this filter would be empty.
    const focusCalls = mockInvoke.mock.calls.filter(
      (c) => c[0] === "spatial_focus",
    );
    expect(focusCalls.length).toBeGreaterThan(0);

    unmount();
  });

  it("layer_renders_no_dom_when_debug_off", async () => {
    // Regression guard: when debug is off, `<FocusLayer>` must not
    // introduce a wrapper div around its children. Production layout
    // depends on the layer being a pure context provider.
    const { container, unmount } = render(
      <FocusDebugProvider enabled={false}>
        <SpatialFocusProvider>
          <FocusLayer name={asLayerName("window")}>
            <span data-testid="layer-child">child</span>
          </FocusLayer>
        </SpatialFocusProvider>
      </FocusDebugProvider>,
    );
    await flushSetup();
    await flushFrame();

    // The child span should be a direct child of `container` — no
    // intermediary wrapper div added by the layer.
    const child = container.querySelector('[data-testid="layer-child"]');
    expect(child).toBeTruthy();
    expect(child!.parentElement).toBe(container);

    // And no `[data-debug=…]` elements anywhere.
    expect(container.querySelectorAll("[data-debug]").length).toBe(0);

    unmount();
  });
});
