/**
 * CSS-transform-animation rect re-report regression test.
 *
 * ## Contract under test
 *
 * Production `SlidePanel` (`kanban-app/ui/src/components/slide-panel.tsx`)
 * animates its open/close via a Tailwind
 * `translate-x-0` ↔ `translate-x-full` class flip with
 * `transition-transform duration-200`. Inside the panel, every
 * `<InspectorFocusBridge>` `<FocusScope>` mounts during the slide-in.
 * `useRectObserver` fires `getBoundingClientRect()` while the panel is
 * still mid-animation — the returned rect reflects the current (stale)
 * transform, not the post-animation position. `ResizeObserver` never
 * fires for the rest of the animation because the element's size is
 * unchanged. The scroll listener never fires because nothing scrolled.
 *
 * Result: inspector-field rects registered with Rust stay frozen at
 * mid-animation coordinates for the lifetime of the panel — `nav.down`
 * and friends score the wrong targets because the spatial graph has
 * lies in it.
 *
 * The fix in `useRectObserver`:
 *   1. Listen for `transitionend` events on `document` (bubbling from
 *      any animated ancestor), filter by `propertyName` in
 *      `transform` / `translate` / `left` / `top` / `right` / `bottom`,
 *      re-run `report()`.
 *   2. Schedule a `requestAnimationFrame` on mount that re-reports the
 *      rect one frame after the initial invoke, catching layout-settle
 *      races even when no `transitionend` fires (for animations that
 *      don't use a CSS transition, or for raw layout shifts that happen
 *      after the observer's first tick).
 *
 * This test pins both mechanisms: a FocusScope mounts inside a
 * `<div style="transform: translateX(100%); transition: transform 100ms">`
 * that flips to `translateX(0)` immediately after mount. The test
 * asserts a second `spatial_register` invocation fires for the scope's
 * moniker after the `transitionend` event, with an `x` coordinate
 * matching the post-animation rect — not the stale mid-animation one.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render } from "vitest-browser-react";
import { useEffect, useState } from "react";

vi.mock("@tauri-apps/api/core", async () => {
  const { tauriCoreMock } = await import("./setup-tauri-stub");
  return tauriCoreMock();
});
vi.mock("@tauri-apps/api/event", async () => {
  const { tauriEventMock } = await import("./setup-tauri-stub");
  return tauriEventMock();
});
vi.mock("@tauri-apps/api/window", async () => {
  const { tauriWindowMock } = await import("./setup-tauri-stub");
  return tauriWindowMock();
});
vi.mock("@tauri-apps/api/webviewWindow", async () => {
  const { tauriWebviewWindowMock } = await import("./setup-tauri-stub");
  return tauriWebviewWindowMock();
});
vi.mock("@tauri-apps/plugin-log", async () => {
  const { tauriPluginLogMock } = await import("./setup-tauri-stub");
  return tauriPluginLogMock();
});

import { setupTauriStub, type TauriStubHandles } from "./setup-tauri-stub";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { FocusScope } from "@/components/focus-scope";
import { FixtureShell } from "./spatial-fixture-shell";
import { moniker } from "@/lib/moniker";

/** Moniker of the scope under test — one FocusScope inside the animated ancestor. */
const FIELD_MONIKER = moniker("task", "slide-panel-field");

/** Pixel width of the sliding panel — determines the translation distance. */
const PANEL_WIDTH_PX = 300;

/**
 * Animated ancestor that mirrors `SlidePanel`: starts off-screen
 * (`translateX(100%)`) and animates to `translateX(0)` on mount. The
 * child `FocusScope` mounts while the transform is still mid-flight —
 * the exact race condition the fix under test has to repair.
 */
function SlidingPanelHarness() {
  const [open, setOpen] = useState(false);
  useEffect(() => {
    // Flip to "open" on the next frame so the transition observes
    // `translateX(100%)` first, then transitions to `translateX(0)`.
    // Without the RAF delay React batches both states into the same
    // style commit and no transition runs.
    const id = requestAnimationFrame(() => setOpen(true));
    return () => cancelAnimationFrame(id);
  }, []);
  return (
    <div
      data-testid="sliding-panel"
      style={{
        position: "fixed",
        top: 0,
        right: 0,
        width: `${PANEL_WIDTH_PX}px`,
        height: "200px",
        transform: open ? "translateX(0)" : `translateX(${PANEL_WIDTH_PX}px)`,
        transition: "transform 100ms ease",
        background: "#fff",
      }}
    >
      <FocusScope
        moniker={FIELD_MONIKER}
        commands={[]}
        data-testid="sliding-field"
        style={{
          width: "100%",
          height: "40px",
          padding: "8px",
          border: "1px solid #ccc",
        }}
      >
        field-inside-panel
      </FocusScope>
    </div>
  );
}

/** Root fixture: provider stack + sliding panel. */
function AppWithSlidingPanel() {
  return (
    <EntityFocusProvider>
      <FixtureShell>
        <SlidingPanelHarness />
      </FixtureShell>
    </EntityFocusProvider>
  );
}

/** Wait one animation frame — useEffect runs after commit, RAF after that. */
function nextFrame(): Promise<void> {
  return new Promise((r) => requestAnimationFrame(() => r()));
}

/**
 * Poll `predicate` every animation frame until it returns `true`, or
 * throw after ~500ms.
 *
 * Used to wait out the real browser CSS transition: the panel animates
 * for 100ms, then Chromium fires `transitionend`. The poll window must
 * be long enough to cover that plus slack for test-runner jitter, but
 * short enough that a genuine fix failure surfaces as a timeout rather
 * than a hang.
 */
async function waitFor(
  what: string,
  predicate: () => boolean | Promise<boolean>,
): Promise<void> {
  const maxFrames = 40; // ~660ms at 60fps — covers 100ms transition + margin.
  for (let i = 0; i < maxFrames; i++) {
    if (await predicate()) return;
    await nextFrame();
  }
  throw new Error(`waitFor timed out: ${what}`);
}

/**
 * Snapshot every `spatial_register` invocation for the field's moniker,
 * returning the `x` coordinate reported in each call in order. The test
 * uses this to compare "mid-animation x" vs "post-animation x".
 */
function xCoordsReported(
  handles: TauriStubHandles,
  targetMoniker: string,
): number[] {
  const xs: number[] = [];
  for (const inv of handles.invocations()) {
    if (inv.cmd !== "spatial_register") continue;
    const a = (inv.args as { args: { moniker: string; x: number } }).args;
    if (a.moniker !== targetMoniker) continue;
    xs.push(a.x);
  }
  return xs;
}

describe("transform-animation ancestor — rect re-reports after transitionend", () => {
  let handles: TauriStubHandles;

  beforeEach(() => {
    handles = setupTauriStub();
  });

  it("re-invokes spatial_register after the ancestor's transform transition completes", async () => {
    await render(<AppWithSlidingPanel />);

    // Let initial registration settle — useEffect runs after commit,
    // and the harness flips `open` on the next frame.
    await nextFrame();
    await nextFrame();

    // Snapshot whatever registrations landed during mount + RAF-settle.
    // The fix under test must produce AT LEAST one more register after
    // the transition completes, with coordinates reflecting the settled
    // panel position (not the mid-slide stale value).
    const xsBefore = xCoordsReported(handles, FIELD_MONIKER);
    expect(
      xsBefore.length,
      "field must be registered at least once on mount",
    ).toBeGreaterThan(0);

    // Wait for the real browser transition (100ms) to finish. Chromium
    // fires a native `transitionend` when the `transition: transform`
    // settles; the fix must listen for it and re-report the now-settled
    // rect. Waiting ~160ms covers the 100ms transition plus a couple
    // of RAF frames for the handler to fire.
    await waitFor("panel transition to settle", async () => {
      const panel = document.querySelector<HTMLElement>(
        '[data-testid="sliding-panel"]',
      );
      if (!panel) return false;
      const rect = panel.getBoundingClientRect();
      // The panel anchors `right: 0` width `PANEL_WIDTH_PX` — its
      // settled left edge is `window.innerWidth - PANEL_WIDTH_PX`.
      // Consider it settled when the rect is within 1px of that
      // target (no more ongoing transition).
      return Math.abs(rect.x - (window.innerWidth - PANEL_WIDTH_PX)) < 1;
    });

    // Two guard frames so any RAF-throttled handler for the final
    // `transitionend` has definitely fired.
    await nextFrame();
    await nextFrame();

    const xsAfter = xCoordsReported(handles, FIELD_MONIKER);
    expect(
      xsAfter.length,
      "transitionend on the animated ancestor must trigger a second spatial_register",
    ).toBeGreaterThan(xsBefore.length);

    // The last invoke must carry the post-animation rect. After the
    // panel settles at `translateX(0)` its child field sits at the
    // panel's fixed right-edge anchor — x is strictly less than
    // `window.innerWidth` and at the panel's settled left edge. The
    // bug under test leaves every call at the mid-animation x
    // (off-screen right), so this assertion also guards against a fix
    // that fires a second `spatial_register` with a still-stale rect.
    const finalX = xsAfter[xsAfter.length - 1];
    expect(
      finalX,
      "post-animation x must be inside the viewport (panel fully visible)",
    ).toBeLessThan(window.innerWidth);
    // The panel anchors at `right: 0` with width `PANEL_WIDTH_PX`, so
    // when it is fully open the field's left edge sits at
    // `window.innerWidth - PANEL_WIDTH_PX`. During the stale mount-time
    // report it sits at least `PANEL_WIDTH_PX` further to the right
    // (off-screen). A small tolerance absorbs rounding and border
    // pixels without masking the bug.
    expect(
      finalX,
      "post-animation x must reflect the settled panel position, not the stale mid-slide one",
    ).toBeLessThanOrEqual(window.innerWidth - PANEL_WIDTH_PX + 2);
  });

  it("ignores transitionend events for non-positional properties (e.g. opacity)", async () => {
    // Regression guard: the fix must filter `propertyName` so random
    // `transitionend` events (opacity fades, color transitions, …) do
    // not spam `spatial_register`. Only transform / translate / the
    // physical position properties should trigger a re-report.
    await render(<AppWithSlidingPanel />);

    // Wait for the real `transition: transform 100ms` in the harness to
    // fully settle BEFORE snapshotting the pre-dispatch count. Otherwise
    // the real `transform` transitionend could land inside the
    // post-dispatch guard window (especially on slow CI runners or a
    // 30Hz frame cap) and trip the "no new re-report" assertion by
    // coincidence, masking the actual behavior we are guarding.
    const panel = document.querySelector<HTMLDivElement>(
      '[data-testid="sliding-panel"]',
    );
    expect(panel).not.toBeNull();
    await waitFor("panel transition to settle", () => {
      const rect = panel!.getBoundingClientRect();
      return Math.abs(rect.x - (window.innerWidth - PANEL_WIDTH_PX)) < 1;
    });
    // A couple of extra frames so any RAF-throttled handler the real
    // transitionend scheduled has definitely flushed.
    await nextFrame();
    await nextFrame();

    const xsBefore = xCoordsReported(handles, FIELD_MONIKER);

    panel!.dispatchEvent(
      new TransitionEvent("transitionend", {
        bubbles: true,
        cancelable: false,
        propertyName: "opacity",
      }),
    );

    await nextFrame();
    await nextFrame();

    const xsAfter = xCoordsReported(handles, FIELD_MONIKER);
    expect(
      xsAfter.length,
      "transitionend for opacity must not trigger a re-report",
    ).toBe(xsBefore.length);
  });
});
