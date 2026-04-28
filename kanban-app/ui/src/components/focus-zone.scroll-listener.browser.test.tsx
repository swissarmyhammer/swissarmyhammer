/**
 * Browser-mode unit tests for the ancestor-scroll listener inside
 * `<FocusZone>` (and shared with `<FocusScope>`).
 *
 * Background: `<FocusZone>` registers its bounding rect with the Rust
 * spatial registry on mount and refreshes it via a `ResizeObserver` when
 * its own box changes size. `ResizeObserver` does NOT fire when an
 * ancestor scrolls — so a card inside a scrolling column would keep its
 * mount-time rect in the kernel while moving on screen, and beam-search
 * would run on stale geometry. This file pins the regression: the
 * primitive must also re-publish its rect when any scrollable ancestor
 * scrolls.
 *
 * Three cases:
 *   1. `focus_zone_rect_tracks_ancestor_scroll` — single scrollable
 *      ancestor, scroll the wrapper and assert the kernel sees the new
 *      viewport-y.
 *   2. `focus_zone_rect_tracks_nested_scrollable_ancestors` — scrollable
 *      wrapper inside another scrollable wrapper, scroll both, assert the
 *      stored rect reflects the combined offset.
 *   3. `focus_zone_unmount_removes_scroll_listeners` — after unmount, a
 *      scroll on the (still-mounted) ancestor must NOT produce any
 *      `spatial_update_rect` IPC.
 *
 * The test runs in real Chromium via vitest-browser-react so the layout
 * and scroll machinery are real — no jsdom fake-rect plumbing.
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
import { FocusLayer } from "./focus-layer";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { asLayerName, asMoniker } from "@/types/spatial";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/**
 * Flush microtasks so the register effects scheduled in `useEffect`
 * have run and their `spatial_register_zone` calls have landed in
 * `mockInvoke`. Two ticks: first lets the effect callbacks run, second
 * lets any Promise-resolution-driven follow-on (e.g. listen-attach)
 * settle.
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
 * Wait for one animation frame followed by a microtask flush. The scroll
 * listener throttles via `requestAnimationFrame`, so a synchronous scroll
 * event needs one rAF tick to land in the IPC mock.
 */
async function flushScroll() {
  await act(async () => {
    await new Promise<void>((resolve) =>
      requestAnimationFrame(() => resolve()),
    );
    await Promise.resolve();
  });
}

/** Pull the most recent `spatial_register_zone` argument bag. */
function lastRegisterZoneArgs() {
  const calls = mockInvoke.mock.calls.filter(
    (c) => c[0] === "spatial_register_zone",
  );
  if (calls.length === 0) {
    throw new Error("expected spatial_register_zone call");
  }
  return calls[calls.length - 1][1] as {
    key: string;
    rect: { x: number; y: number; width: number; height: number };
  };
}

/** Pull every `spatial_update_rect` invocation. */
function updateRectCalls(): Array<{
  key: string;
  rect: { x: number; y: number; width: number; height: number };
}> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "spatial_update_rect")
    .map(
      (c) =>
        c[1] as {
          key: string;
          rect: { x: number; y: number; width: number; height: number };
        },
    );
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("<FocusZone> — ancestor scroll listener", () => {
  beforeEach(() => {
    mockInvoke.mockClear();
  });

  afterEach(() => {
    document.body.innerHTML = "";
  });

  // -------------------------------------------------------------------------
  // 1. Single scrollable ancestor — scroll updates the kernel's stored rect.
  // -------------------------------------------------------------------------

  it("focus_zone_rect_tracks_ancestor_scroll", async () => {
    // Mount under a fixed-height scrollable wrapper. Inner content is
    // taller than the wrapper so the wrapper genuinely scrolls.
    const { container, unmount } = render(
      <SpatialFocusProvider>
        <FocusLayer name={asLayerName("window")}>
          <div
            data-testid="scroller"
            style={{
              height: "200px",
              overflowY: "auto",
              border: "1px solid black",
            }}
          >
            <div style={{ height: "1000px", padding: "20px" }}>
              <FocusZone moniker={asMoniker("ui:tracked-zone")}>
                <span style={{ display: "block", padding: "20px" }}>zone</span>
              </FocusZone>
            </div>
          </div>
        </FocusLayer>
      </SpatialFocusProvider>,
    );
    await flushSetup();

    const initial = lastRegisterZoneArgs();
    const initialKey = initial.key;
    const initialY = initial.rect.y;

    const scroller = container.querySelector(
      "[data-testid='scroller']",
    ) as HTMLElement;
    expect(scroller).toBeTruthy();

    // Scroll the wrapper by 100px. The zone's viewport-y must drop by
    // 100 (its content-y is fixed; only the viewport is shifting).
    mockInvoke.mockClear();
    await act(async () => {
      scroller.scrollTop = 100;
      scroller.dispatchEvent(new Event("scroll"));
    });
    await flushScroll();

    const updates = updateRectCalls();
    expect(updates.length).toBeGreaterThan(0);

    const last = updates[updates.length - 1];
    expect(last.key).toBe(initialKey);
    // Within 1px tolerance for sub-pixel rounding. The Y must have
    // moved by approximately -100 from the initial register.
    expect(Math.abs(last.rect.y - (initialY - 100))).toBeLessThanOrEqual(1);

    unmount();
  });

  // -------------------------------------------------------------------------
  // 2. Nested scrollable ancestors — scroll on either updates the rect.
  // -------------------------------------------------------------------------

  it("focus_zone_rect_tracks_nested_scrollable_ancestors", async () => {
    const { container, unmount } = render(
      <SpatialFocusProvider>
        <FocusLayer name={asLayerName("window")}>
          <div
            data-testid="outer"
            style={{ height: "400px", overflowY: "auto" }}
          >
            <div style={{ height: "800px", padding: "20px" }}>
              <div
                data-testid="inner"
                style={{ height: "200px", overflowY: "auto" }}
              >
                <div style={{ height: "1000px", padding: "20px" }}>
                  <FocusZone moniker={asMoniker("ui:nested-zone")}>
                    <span style={{ display: "block", padding: "20px" }}>
                      nested
                    </span>
                  </FocusZone>
                </div>
              </div>
            </div>
          </div>
        </FocusLayer>
      </SpatialFocusProvider>,
    );
    await flushSetup();

    const initial = lastRegisterZoneArgs();
    const initialKey = initial.key;
    const initialY = initial.rect.y;

    const outer = container.querySelector(
      "[data-testid='outer']",
    ) as HTMLElement;
    const inner = container.querySelector(
      "[data-testid='inner']",
    ) as HTMLElement;
    expect(outer).toBeTruthy();
    expect(inner).toBeTruthy();

    mockInvoke.mockClear();

    // Scroll the inner first.
    await act(async () => {
      inner.scrollTop = 60;
      inner.dispatchEvent(new Event("scroll"));
    });
    await flushScroll();

    // Then scroll the outer.
    await act(async () => {
      outer.scrollTop = 90;
      outer.dispatchEvent(new Event("scroll"));
    });
    await flushScroll();

    const updates = updateRectCalls();
    expect(updates.length).toBeGreaterThan(0);

    const last = updates[updates.length - 1];
    expect(last.key).toBe(initialKey);
    // Combined offset is 60 + 90 = 150. Both scrolls subtract from
    // the zone's viewport-y; tolerance 1px for sub-pixel rounding.
    expect(Math.abs(last.rect.y - (initialY - 150))).toBeLessThanOrEqual(1);

    unmount();
  });

  // -------------------------------------------------------------------------
  // 3. Listener cleanup on unmount.
  // -------------------------------------------------------------------------

  it("focus_zone_unmount_removes_scroll_listeners", async () => {
    const { container, unmount } = render(
      <SpatialFocusProvider>
        <FocusLayer name={asLayerName("window")}>
          <div
            data-testid="scroller"
            style={{ height: "200px", overflowY: "auto" }}
          >
            <div style={{ height: "1000px" }}>
              <FocusZone moniker={asMoniker("ui:cleanup-zone")}>
                <span style={{ display: "block", padding: "20px" }}>x</span>
              </FocusZone>
            </div>
          </div>
        </FocusLayer>
      </SpatialFocusProvider>,
    );
    await flushSetup();

    const scroller = container.querySelector(
      "[data-testid='scroller']",
    ) as HTMLElement;
    expect(scroller).toBeTruthy();

    // Unmount, THEN scroll — no `updateRect` IPC should fire because the
    // listener detached on cleanup.
    unmount();
    mockInvoke.mockClear();

    scroller.scrollTop = 100;
    scroller.dispatchEvent(new Event("scroll"));
    await flushScroll();

    const updates = updateRectCalls();
    expect(updates.length).toBe(0);
  });
});
