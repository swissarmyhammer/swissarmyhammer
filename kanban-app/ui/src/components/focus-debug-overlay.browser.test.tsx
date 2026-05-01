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
import { Profiler, useRef } from "react";

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
import { FocusDebugOverlay, type FocusDebugKind } from "./focus-debug-overlay";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { FocusDebugProvider } from "@/lib/focus-debug-context";
import {
  asSegment
} from "@/types/spatial";

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
 * Direct-mount harness for `<FocusDebugOverlay>`. Renders a fixed-position
 * host `<div>` at the supplied rect and mounts the overlay against a ref to
 * that host. Lets the overlay tests exercise the component in isolation —
 * no `<FocusLayer>` / `<FocusZone>` machinery, no spatial-focus IPC.
 *
 * The host div carries `data-testid="overlay-host"` so tests can grab it to
 * mutate dimensions later (used by the dimension-change rerender test).
 */
function OverlayHarness({
  kind,
  label,
  hostStyle,
  onRender,
}: {
  kind: FocusDebugKind;
  label: string;
  hostStyle: React.CSSProperties;
  onRender?: (phase: "mount" | "update" | "nested-update") => void;
}) {
  const hostRef = useRef<HTMLDivElement | null>(null);
  // Force a re-render once after mount so the ref is populated before the
  // overlay's effect runs against it. Without this the first
  // `getBoundingClientRect()` may run against null on the very first frame.
  const overlay = (
    <FocusDebugOverlay kind={kind} label={label} hostRef={hostRef} />
  );
  return (
    <div
      ref={hostRef}
      data-testid="overlay-host"
      style={{ position: "fixed", ...hostStyle }}
    >
      {onRender ? (
        <Profiler id="overlay-probe" onRender={(_id, phase) => onRender(phase)}>
          {overlay}
        </Profiler>
      ) : (
        overlay
      )}
    </div>
  );
}

/**
 * Resolve the rendered debug label text for a given overlay kind. The label
 * sits in the overlay's second child `<span>` (the first is the dashed
 * border). Returns the trimmed `textContent` or null if the overlay is not
 * mounted.
 */
function readOverlayLabel(
  container: HTMLElement,
  kind: FocusDebugKind,
): string | null {
  const overlay = container.querySelector(`[data-debug="${kind}"]`);
  if (!overlay) return null;
  return overlay.textContent?.trim() ?? null;
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
          <FocusLayer name={asSegment("window")}>
            <FocusZone moniker={asSegment("ui:test")}>
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
          <FocusLayer name={asSegment("window")}>
            <FocusScope moniker={asSegment("ui:test.leaf")}>
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
          <FocusLayer name={asSegment("window")}>
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
          <FocusLayer name={asSegment("window")}>
            <FocusZone moniker={asSegment("ui:test")}>
              <FocusScope moniker={asSegment("ui:test.leaf")}>
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
        <FocusLayer name={asSegment("window")}>
          <FocusZone moniker={asSegment("ui:test")}>
            <FocusScope moniker={asSegment("ui:test.leaf")}>
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
          <FocusLayer name={asSegment("window")}>
            <FocusZone
              moniker={asSegment("ui:positioned")}
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
          <FocusLayer name={asSegment("window")}>
            <FocusZone moniker={asSegment("ui:zone-test")}>
              <FocusScope moniker={asSegment("ui:scope-test")}>
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
          <FocusLayer name={asSegment("window")}>
            <FocusScope moniker={asSegment("ui:click-test")}>
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
          <FocusLayer name={asSegment("window")}>
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

  it("zone_label_has_no_dimensions_suffix", async () => {
    // Mount the overlay against a fixed-rect host at (10, 20, 100, 50). The
    // visible label must be exactly `zone:ui:test (10,20)` — no width ×
    // height suffix. The original ask for the overlay was a tiny x/y read-
    // out for placement verification; the dimensions had crept in as
    // visual noise.
    const { container, unmount } = render(
      <OverlayHarness
        kind="zone"
        label="ui:test"
        hostStyle={{
          left: "10px",
          top: "20px",
          width: "100px",
          height: "50px",
        }}
      />,
    );
    await flushFrame();
    // Second frame to flush the rect commit back through React.
    await flushFrame();

    const text = readOverlayLabel(container, "zone");
    expect(text).toBe("zone:ui:test (10,20)");
    expect(text).not.toContain("100×50");
    expect(text).not.toContain("100x50");

    unmount();
  });

  it("scope_label_has_no_dimensions_suffix", async () => {
    // Same shape as the zone test, but with `kind="scope"` to pin that the
    // dimension-suffix removal applies to scopes too.
    const { container, unmount } = render(
      <OverlayHarness
        kind="scope"
        label="ui:test"
        hostStyle={{
          left: "10px",
          top: "20px",
          width: "100px",
          height: "50px",
        }}
      />,
    );
    await flushFrame();
    await flushFrame();

    const text = readOverlayLabel(container, "scope");
    expect(text).toBe("scope:ui:test (10,20)");
    expect(text).not.toContain("100×50");
    expect(text).not.toContain("100x50");

    unmount();
  });

  it("layer_label_unchanged", async () => {
    // Regression guard: layers omit coordinates entirely, so the label is
    // exactly `layer:<name>` with no rect at all. Removing the dimension
    // suffix from zones/scopes must not perturb the layer format.
    const { container, unmount } = render(
      <OverlayHarness
        kind="layer"
        label="window"
        hostStyle={{
          left: "0px",
          top: "0px",
          width: "100px",
          height: "100px",
        }}
      />,
    );
    await flushFrame();
    await flushFrame();

    const text = readOverlayLabel(container, "layer");
    expect(text).toBe("layer:window");

    unmount();
  });

  it("overlay_does_not_rerender_on_pure_dimension_change", async () => {
    // After dropping the width/height legs of the rect-equality short
    // circuit in `<FocusDebugOverlay>`, a host whose top-left stays put
    // but whose width/height changes must NOT cause the overlay to commit
    // a new render. Pin that here with a `<Profiler>` probe.
    const renderPhases: string[] = [];

    const { container, unmount } = render(
      <OverlayHarness
        kind="zone"
        label="ui:test"
        hostStyle={{
          left: "10px",
          top: "20px",
          width: "100px",
          height: "50px",
        }}
        onRender={(phase) => renderPhases.push(phase)}
      />,
    );
    // Two frames so the initial mount + first rect commit have settled.
    await flushFrame();
    await flushFrame();

    // Sanity: the harness has mounted at least once and the label reflects
    // the starting rect.
    expect(renderPhases.length).toBeGreaterThan(0);
    expect(readOverlayLabel(container, "zone")).toBe("zone:ui:test (10,20)");

    // Snapshot the commit count, then mutate width/height while keeping
    // the top-left fixed at (10, 20). A subsequent rAF tick reads the new
    // rect; with the equality short-circuit unchanged on x/y, `setRect`
    // should bail and no further commits should land.
    const commitsBeforeResize = renderPhases.length;
    const hostEl = container.querySelector(
      '[data-testid="overlay-host"]',
    ) as HTMLElement;
    expect(hostEl).toBeTruthy();
    hostEl.style.width = "250px";
    hostEl.style.height = "175px";

    // A few frames to give the rAF poll opportunity to observe the new
    // dimensions and (incorrectly) trigger a commit if the short-circuit
    // is broken.
    await flushFrame();
    await flushFrame();
    await flushFrame();

    expect(renderPhases.length).toBe(commitsBeforeResize);
    // Label still has the original (10,20) coordinates and no dim suffix.
    expect(readOverlayLabel(container, "zone")).toBe("zone:ui:test (10,20)");

    unmount();
  });
});
