/**
 * Browser-mode tests for the spatial-nav debug overlay.
 *
 * Covers four assertions:
 *
 *   1. When `<FocusDebugProvider enabled>` wraps the tree, every
 *      `<FocusLayer>` / `<FocusScope>` / `<FocusScope>` mounts a
 *      `[data-debug=…]` element with the `border-dashed` class and a
 *      hover-revealed tooltip that mentions the primitive's name /
 *      moniker.
 *   2. When the provider is disabled (or absent), no `[data-debug=…]`
 *      elements render anywhere in the tree.
 *   3. The overlay's coordinate label tracks the host's bounding rect so
 *      a fixed-position parent at `(100, 200)` produces a tooltip that
 *      contains `"100,200"`.
 *   4. The overlay's `pointer-events: none` (on the wrapper / border) is
 *      honoured — clicks on the host content land on the host's click
 *      handler, not the overlay. Clicks on the *handle* (the only
 *      `pointer-events: auto` region) are stopped at the handle and do
 *      NOT reach the host.
 *
 * Runs in real Chromium via vitest browser mode so layout (the rect
 * reads, the absolute positioning, the dashed border, hover events) is
 * genuine.
 */

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, act } from "@testing-library/react";
import { userEvent } from "vitest/browser";
import { Profiler, useRef, type ReactNode } from "react";

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

import { FocusScope } from "./focus-scope";
import { FocusLayer } from "./focus-layer";
import { FocusDebugOverlay, type FocusDebugKind } from "./focus-debug-overlay";
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
 * Direct-mount harness for `<FocusDebugOverlay>`. Renders a fixed-position
 * host `<div>` at the supplied rect and mounts the overlay against a ref to
 * that host. Lets the overlay tests exercise the component in isolation —
 * no `<FocusLayer>` / `<FocusScope>` machinery, no spatial-focus IPC.
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
    <TooltipProvider delayDuration={0}>
      <div
        ref={hostRef}
        data-testid="overlay-host"
        style={{ position: "fixed", ...hostStyle }}
      >
        {onRender ? (
          <Profiler
            id="overlay-probe"
            onRender={(_id, phase) => onRender(phase)}
          >
            {overlay}
          </Profiler>
        ) : (
          overlay
        )}
      </div>
    </TooltipProvider>
  );
}

/**
 * Resolve the debug overlay's hover handle for a given kind. The handle
 * is the only `pointer-events: auto` region of the overlay; tests fire
 * hover and click events against it.
 *
 * Returns null when the overlay is not mounted (e.g. when the debug
 * provider is disabled).
 */
function getHandle(
  container: HTMLElement,
  kind: FocusDebugKind,
): HTMLElement | null {
  return container.querySelector<HTMLElement>(
    `[data-debug="${kind}"] [data-debug-handle="${kind}"]`,
  );
}

/**
 * Read the overlay's label text for a given kind without opening the
 * tooltip — the handle's `aria-label` mirrors the tooltip content
 * exactly so screen readers announce the same string the visual
 * tooltip would. This is the deterministic way to assert label content
 * across both production and test environments; the explicit hover
 * path is exercised in `tooltip_opens_on_handle_hover` below.
 *
 * Returns null when the overlay (or the handle within it) is not
 * mounted.
 */
function readOverlayLabel(
  container: HTMLElement,
  kind: FocusDebugKind,
): string | null {
  const handle = getHandle(container, kind);
  if (!handle) return null;
  return handle.getAttribute("aria-label");
}

/**
 * Read the visible tooltip's text content from the document body.
 * Radix portals `<TooltipContent>` outside the test container, so this
 * helper queries the entire document for the `[data-slot="tooltip-
 * content"]` portal element.
 *
 * Note: Radix renders both a visible `Slottable` *and* an offscreen
 * `<VisuallyHidden role="tooltip">` clone of the same children inside
 * the same content node, so `textContent` would naturally double the
 * label string. We strip the `[role="tooltip"]` clone before reading
 * `textContent` so the result matches what the user actually sees.
 *
 * Returns null when no tooltip is currently open.
 */
function readOpenTooltipText(): string | null {
  const content = document.body.querySelector<HTMLElement>(
    '[data-slot="tooltip-content"]',
  );
  if (!content) return null;
  const visuallyHidden = content.querySelector('[role="tooltip"]');
  // textContent of just the non-VisuallyHidden subtree.
  const visibleText = Array.from(content.childNodes)
    .filter((node) => node !== visuallyHidden)
    .map((node) => node.textContent ?? "")
    .join("")
    .trim();
  return visibleText;
}

/**
 * Wrapper that ensures a `<TooltipProvider>` ancestor is present. The
 * production `<FocusDebugOverlay>` lives under the
 * `<TooltipProvider>` mounted at `<WindowContainer>`; tests must
 * supply an equivalent wrapper because the integration trees here
 * mount overlays outside the real provider hierarchy.
 *
 * `delayDuration={0}` makes hover-to-open instant so tests do not need
 * to wait for the production 400ms hover delay.
 */
function withTooltipProvider(children: ReactNode) {
  return <TooltipProvider delayDuration={0}>{children}</TooltipProvider>;
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

  it("focusscope_renders_debug_overlay_when_debug_on", async () => {
    // After parent task `01KQSDP4ZJY5ERAJ68TFPVFRRE` collapsed the
    // legacy split primitives into a single `<FocusScope>`, every
    // spatial primitive composes the debug overlay with `kind="scope"`.
    // The test was previously paired (zone + scope variants) when the
    // legacy split primitives produced two distinct `data-debug`
    // values.
    const { container, unmount } = render(
      withTooltipProvider(
        <FocusDebugProvider enabled>
          <SpatialFocusProvider>
            <FocusLayer name={asSegment("window")}>
              <FocusScope moniker={asSegment("ui:test")}>
                <span>scope-content</span>
              </FocusScope>
            </FocusLayer>
          </SpatialFocusProvider>
        </FocusDebugProvider>,
      ),
    );
    await flushSetup();
    await flushFrame();

    const overlay = container.querySelector('[data-debug="scope"]');
    expect(overlay).toBeTruthy();

    // Border element is the first child span — assert `border-dashed`
    // is on it.
    const borderSpan = overlay!.querySelector("span");
    expect(borderSpan).toBeTruthy();
    expect(borderSpan!.className).toContain("border-dashed");

    // The label (now a tooltip) carries the moniker. Read it via the
    // handle's `aria-label`, which mirrors the tooltip content
    // verbatim — deterministic without a hover round-trip.
    expect(readOverlayLabel(container, "scope")).toContain("ui:test");

    unmount();
  });

  it("scope_renders_debug_overlay_when_debug_on", async () => {
    const { container, unmount } = render(
      withTooltipProvider(
        <FocusDebugProvider enabled>
          <SpatialFocusProvider>
            <FocusLayer name={asSegment("window")}>
              <FocusScope moniker={asSegment("ui:test.leaf")}>
                <span>scope-content</span>
              </FocusScope>
            </FocusLayer>
          </SpatialFocusProvider>
        </FocusDebugProvider>,
      ),
    );
    await flushSetup();
    await flushFrame();

    const overlay = container.querySelector('[data-debug="scope"]');
    expect(overlay).toBeTruthy();

    const borderSpan = overlay!.querySelector("span");
    expect(borderSpan).toBeTruthy();
    expect(borderSpan!.className).toContain("border-dashed");

    expect(readOverlayLabel(container, "scope")).toContain("ui:test.leaf");

    unmount();
  });

  it("layer_renders_debug_overlay_when_debug_on", async () => {
    const { container, unmount } = render(
      withTooltipProvider(
        <FocusDebugProvider enabled>
          <SpatialFocusProvider>
            <FocusLayer name={asSegment("window")}>
              <span>layer-content</span>
            </FocusLayer>
          </SpatialFocusProvider>
        </FocusDebugProvider>,
      ),
    );
    await flushSetup();
    await flushFrame();

    const overlay = container.querySelector('[data-debug="layer"]');
    expect(overlay).toBeTruthy();

    const borderSpan = overlay!.querySelector("span");
    expect(borderSpan).toBeTruthy();
    expect(borderSpan!.className).toContain("border-dashed");

    expect(readOverlayLabel(container, "layer")).toContain("window");

    unmount();
  });

  it("no_overlay_when_debug_off", async () => {
    const { container, unmount } = render(
      withTooltipProvider(
        <FocusDebugProvider enabled={false}>
          <SpatialFocusProvider>
            <FocusLayer name={asSegment("window")}>
              <FocusScope moniker={asSegment("ui:test")}>
                <FocusScope moniker={asSegment("ui:test.leaf")}>
                  <span>content</span>
                </FocusScope>
              </FocusScope>
            </FocusLayer>
          </SpatialFocusProvider>
        </FocusDebugProvider>,
      ),
    );
    await flushSetup();
    await flushFrame();

    expect(container.querySelectorAll("[data-debug]").length).toBe(0);

    unmount();
  });

  it("no_overlay_when_no_provider", async () => {
    const { container, unmount } = render(
      withTooltipProvider(
        <SpatialFocusProvider>
          <FocusLayer name={asSegment("window")}>
            <FocusScope moniker={asSegment("ui:test")}>
              <FocusScope moniker={asSegment("ui:test.leaf")}>
                <span>content</span>
              </FocusScope>
            </FocusScope>
          </FocusLayer>
        </SpatialFocusProvider>,
      ),
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
    // `<FocusScope>` already merges `relative` into the className, but
    // the inline `position: fixed` style overrides that for layout
    // (the merged class ends up unused; that's acceptable for a test).
    const { container, unmount } = render(
      withTooltipProvider(
        <FocusDebugProvider enabled>
          <SpatialFocusProvider>
            <FocusLayer name={asSegment("window")}>
              <FocusScope
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
              </FocusScope>
            </FocusLayer>
          </SpatialFocusProvider>
        </FocusDebugProvider>,
      ),
    );
    await flushSetup();
    await flushFrame();
    // One more frame so the rect read in `requestAnimationFrame` has
    // been committed back into React state and rendered into the DOM.
    await flushFrame();

    const overlay = container.querySelector('[data-debug="scope"]');
    expect(overlay).toBeTruthy();
    // The (x,y) coordinates live in the tooltip — assert via the
    // handle's `aria-label`, which mirrors the tooltip text exactly.
    expect(readOverlayLabel(container, "scope")).toContain("100,200");

    unmount();
  });

  it("overlay_kind_classes_are_distinct", async () => {
    // After parent task `01KQSDP4ZJY5ERAJ68TFPVFRRE` collapsed the
    // legacy split primitives into a single `<FocusScope>`, the only
    // overlay kinds the production tree composes are `"layer"` (from
    // `<FocusLayer>`) and `"scope"` (from `<FocusScope>`). The
    // distinction between them must remain colour-coded so nested
    // primitives are visually distinguishable.
    const { container, unmount } = render(
      withTooltipProvider(
        <FocusDebugProvider enabled>
          <SpatialFocusProvider>
            <FocusLayer name={asSegment("window")}>
              <FocusScope moniker={asSegment("ui:scope-test")}>
                <span>scope-content</span>
              </FocusScope>
            </FocusLayer>
          </SpatialFocusProvider>
        </FocusDebugProvider>,
      ),
    );
    await flushSetup();
    await flushFrame();

    const layerOverlay = container.querySelector('[data-debug="layer"]');
    const scopeOverlay = container.querySelector('[data-debug="scope"]');
    expect(layerOverlay).toBeTruthy();
    expect(scopeOverlay).toBeTruthy();

    // Each overlay's first-child border span must carry a colour class
    // unique to its kind. Read the className strings and assert each
    // contains the expected colour token.
    const layerBorder = layerOverlay!.querySelector("span")!.className;
    const scopeBorder = scopeOverlay!.querySelector("span")!.className;

    expect(layerBorder).toContain("border-red-500/70");
    expect(scopeBorder).toContain("border-emerald-500/70");

    // And those tokens must NOT cross-pollinate.
    expect(layerBorder).not.toContain("border-emerald-500/70");
    expect(scopeBorder).not.toContain("border-red-500/70");

    unmount();
  });

  it("overlay_does_not_intercept_clicks", async () => {
    // Mount a `<FocusScope>` with debug on. A click on the host's
    // content should still call the spatial-focus IPC (`spatial_focus`).
    // If the overlay's `pointer-events: none` (on the wrapper / border)
    // were broken, the overlay span would intercept the click and the
    // IPC would never fire.
    //
    // Sub-assertion (added for the hover-handle redesign): the *handle*
    // is the only `pointer-events: auto` region of the overlay and must
    // explicitly stop click propagation — clicking the handle is the
    // affordance for opening the tooltip, NOT for activating the host.
    // If the handle's `stopPropagation` is removed, this sub-assertion
    // catches the regression.
    const { container, unmount } = render(
      withTooltipProvider(
        <FocusDebugProvider enabled>
          <SpatialFocusProvider>
            <FocusLayer name={asSegment("window")}>
              <FocusScope moniker={asSegment("ui:click-test")}>
                <span data-testid="click-target">click me</span>
              </FocusScope>
            </FocusLayer>
          </SpatialFocusProvider>
        </FocusDebugProvider>,
      ),
    );
    await flushSetup();
    await flushFrame();

    // 1) Click on the host content — the overlay's wrapper / border
    // are `pointer-events: none` so the click reaches the host's
    // `spatial_focus` handler.
    const target = container.querySelector(
      '[data-testid="click-target"]',
    ) as HTMLElement;
    expect(target).toBeTruthy();

    mockInvoke.mockClear();
    target.click();
    await flushSetup();

    const focusCallsFromHost = mockInvoke.mock.calls.filter(
      (c) => c[0] === "spatial_focus",
    );
    expect(focusCallsFromHost.length).toBeGreaterThan(0);

    // 2) Click on the handle itself — the handle stops propagation so
    // the host's click handler MUST NOT fire. Clear the mock so any
    // new `spatial_focus` calls would come from this synthetic event
    // alone.
    const handle = getHandle(container, "scope");
    expect(handle).toBeTruthy();

    mockInvoke.mockClear();
    handle!.click();
    await flushSetup();

    const focusCallsFromHandle = mockInvoke.mock.calls.filter(
      (c) => c[0] === "spatial_focus",
    );
    expect(focusCallsFromHandle.length).toBe(0);

    unmount();
  });

  it("layer_renders_no_dom_when_debug_off", async () => {
    // Regression guard: when debug is off, `<FocusLayer>` must not
    // introduce a wrapper div around its children. Production layout
    // depends on the layer being a pure context provider.
    const { container, unmount } = render(
      withTooltipProvider(
        <FocusDebugProvider enabled={false}>
          <SpatialFocusProvider>
            <FocusLayer name={asSegment("window")}>
              <span data-testid="layer-child">child</span>
            </FocusLayer>
          </SpatialFocusProvider>
        </FocusDebugProvider>,
      ),
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

  it("tooltip_opens_on_handle_hover", async () => {
    // End-to-end check on the hover affordance: when the user hovers
    // the handle, Radix opens a `<TooltipContent>` portal whose text
    // is exactly the computed `labelText` (which the handle's
    // `aria-label` mirrors). Uses `userEvent.hover()` from
    // vitest/browser so the real Chromium pointer plumbing fires the
    // `pointerenter` event Radix listens for.
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
    await flushFrame();

    // Tooltip starts closed — no portal in the document.
    expect(readOpenTooltipText()).toBeNull();

    const handle = getHandle(container, "zone");
    expect(handle).toBeTruthy();

    await userEvent.hover(handle!);
    // Allow Radix's open-state effect + portal mount to flush.
    await flushSetup();

    // Tooltip is now open and its content matches the handle's
    // aria-label exactly.
    expect(readOpenTooltipText()).toBe("zone:ui:test (10,20)");
    expect(readOverlayLabel(container, "zone")).toBe("zone:ui:test (10,20)");

    unmount();
  });

  it("tooltip_for_layer_kind_shows_kind_and_label", async () => {
    // Layer overlays omit (x,y) — the tooltip text is `layer:<name>`.
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

    const handle = getHandle(container, "layer");
    expect(handle).toBeTruthy();

    await userEvent.hover(handle!);
    await flushSetup();

    expect(readOpenTooltipText()).toBe("layer:window");

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
