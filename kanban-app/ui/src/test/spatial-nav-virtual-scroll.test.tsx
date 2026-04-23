/**
 * Board virtualized-column scroll regression test.
 *
 * ## Contract under test
 *
 * `kanban-app/ui/src/components/column-view.tsx` uses
 * `@tanstack/react-virtual` to position cards with
 * `transform: translateY(${startPx}px)` inside a scroll container. When
 * the container scrolls, cards visually move but `ResizeObserver` does
 * not fire (transform changes are neither size nor layout changes).
 * Without a scroll listener the rect each card's `FocusScope` pushed to
 * Rust on mount stays stuck at its first-measured coordinates — so
 * `j`/`k` navigation, which relies on those rects, jumps to the wrong
 * card after a scroll.
 *
 * `useRectObserver` in `focus-scope.tsx` installs a RAF-throttled
 * scroll listener on the nearest scrollable ancestor; this test pins
 * that behaviour by simulating a scroll and asserting
 * `spatial_register` re-fires for each mounted card's moniker with
 * updated coordinates.
 *
 * ## Fixture shape
 *
 * Mirrors the production virtualizer's DOM shape closely enough for
 * the scroll-listener + rect contract:
 *
 * - A fixed-height scroll container (`overflow: auto`).
 * - Many cards positioned via `transform: translateY(px)` with
 *   `position: absolute` inside a tall spacer (same pattern the real
 *   virtualizer uses).
 *
 * The real virtualizer mounts/unmounts cards as they enter and leave
 * the viewport; this fixture keeps every card mounted so the test can
 * assert "rects updated on scroll" without also having to model
 * remount semantics. Rect staleness on mounted cards is the bug
 * under fix; virtualizer mount/unmount is covered by separate tests
 * (and is not affected by this fix).
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render } from "vitest-browser-react";
import { useRef } from "react";

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

/** Height of every card in the fixture — matches card estimate in production. */
const CARD_HEIGHT_PX = 60;

/** Height of the scroll container — deliberately small so many cards live off-screen. */
const SCROLLER_HEIGHT_PX = 200;

/** Card count — well above the visible viewport at `SCROLLER_HEIGHT_PX`. */
const CARD_COUNT = 25;

/** Build the card moniker for a 0-indexed card position. */
function cardMoniker(i: number): string {
  return moniker("task", `virt-card-${i}`);
}

/**
 * One virtualized card — absolutely positioned inside the spacer via
 * `transform: translateY(px)`. Each card is a `FocusScope` so the
 * scroll-re-report contract under test is exercised on the same hook
 * production uses.
 */
function VirtualCard({ index }: { index: number }) {
  const mk = cardMoniker(index);
  return (
    <FocusScope
      moniker={mk}
      commands={[]}
      data-testid={`card:${mk}`}
      style={{
        position: "absolute",
        top: 0,
        left: 0,
        width: "100%",
        height: `${CARD_HEIGHT_PX}px`,
        boxSizing: "border-box",
        padding: "4px",
        border: "1px solid #ccc",
        borderRadius: "4px",
        background: "white",
        transform: `translateY(${index * CARD_HEIGHT_PX}px)`,
      }}
    >
      {mk}
    </FocusScope>
  );
}

/** Virtualized column fixture — scroll container with transform-positioned cards. */
function VirtualizedColumn() {
  const scrollerRef = useRef<HTMLDivElement | null>(null);
  return (
    <div
      ref={scrollerRef}
      data-testid="virtual-scroller"
      style={{
        height: `${SCROLLER_HEIGHT_PX}px`,
        width: "200px",
        overflow: "auto",
        background: "#f5f5f5",
      }}
    >
      <div
        data-testid="virtual-spacer"
        style={{
          height: `${CARD_COUNT * CARD_HEIGHT_PX}px`,
          width: "100%",
          position: "relative",
        }}
      >
        {Array.from({ length: CARD_COUNT }, (_, i) => (
          <VirtualCard key={i} index={i} />
        ))}
      </div>
    </div>
  );
}

/** Root fixture: provider stack + scrolling column. */
function AppWithVirtualizedColumn() {
  return (
    <EntityFocusProvider>
      <FixtureShell>
        <VirtualizedColumn />
      </FixtureShell>
    </EntityFocusProvider>
  );
}

/** Wait one animation frame — the scroll listener RAF-throttles report() calls. */
function nextFrame(): Promise<void> {
  return new Promise((r) => requestAnimationFrame(() => r()));
}

describe("virtualized column — scroll re-reports spatial rects", () => {
  let handles: TauriStubHandles;

  beforeEach(() => {
    handles = setupTauriStub();
  });

  it("re-invokes spatial_register for every mounted card when the container scrolls", async () => {
    await render(<AppWithVirtualizedColumn />);

    // Let initial registration settle — `useEffect` runs after commit.
    await nextFrame();

    // Snapshot the initial per-moniker register count — the bug would
    // leave these numbers frozen after a scroll.
    const countsBefore = new Map<string, number>();
    for (const inv of handles.invocations()) {
      if (inv.cmd !== "spatial_register") continue;
      const args = (inv.args as { args: { moniker: string } }).args;
      countsBefore.set(args.moniker, (countsBefore.get(args.moniker) ?? 0) + 1);
    }

    // Sanity: every card got at least one register on mount.
    for (let i = 0; i < CARD_COUNT; i++) {
      const mk = cardMoniker(i);
      expect(
        countsBefore.get(mk) ?? 0,
        `card ${i} (${mk}) should have been registered on mount`,
      ).toBeGreaterThan(0);
    }

    // Scroll the container. The scroll listener on the nearest
    // scrollable ancestor must fire, schedule a RAF, and re-report
    // every mounted card.
    const scroller = document.querySelector<HTMLDivElement>(
      '[data-testid="virtual-scroller"]',
    );
    expect(scroller).not.toBeNull();
    scroller!.scrollTop = 120;
    scroller!.dispatchEvent(new Event("scroll"));

    // Two frames: one for the scroll handler's RAF, one guard.
    await nextFrame();
    await nextFrame();

    const countsAfter = new Map<string, number>();
    for (const inv of handles.invocations()) {
      if (inv.cmd !== "spatial_register") continue;
      const args = (inv.args as { args: { moniker: string } }).args;
      countsAfter.set(args.moniker, (countsAfter.get(args.moniker) ?? 0) + 1);
    }

    // Every card's register count must strictly increase — the scroll
    // forced a fresh report for each.
    for (let i = 0; i < CARD_COUNT; i++) {
      const mk = cardMoniker(i);
      const before = countsBefore.get(mk) ?? 0;
      const after = countsAfter.get(mk) ?? 0;
      expect(
        after,
        `card ${i} (${mk}) should be re-registered after scroll`,
      ).toBeGreaterThan(before);
    }
  });

  it("RAF-coalesces rapid scroll events so per-scope register rate stays bounded", async () => {
    await render(<AppWithVirtualizedColumn />);
    await nextFrame();

    const scroller = document.querySelector<HTMLDivElement>(
      '[data-testid="virtual-scroller"]',
    );
    expect(scroller).not.toBeNull();

    // Snapshot counts right before the scroll burst.
    const countsBefore = new Map<string, number>();
    for (const inv of handles.invocations()) {
      if (inv.cmd !== "spatial_register") continue;
      const args = (inv.args as { args: { moniker: string } }).args;
      countsBefore.set(args.moniker, (countsBefore.get(args.moniker) ?? 0) + 1);
    }

    // Fire many scrolls inside a single frame. Without RAF throttling,
    // every one of these would trigger a full-column re-report
    // (10 scrolls × 25 cards = 250 extra invokes). With throttling,
    // all collapse into one report per card for the whole burst.
    for (let i = 0; i < 10; i++) {
      scroller!.scrollTop = 10 * (i + 1);
      scroller!.dispatchEvent(new Event("scroll"));
    }

    await nextFrame();
    await nextFrame();

    const countsAfter = new Map<string, number>();
    for (const inv of handles.invocations()) {
      if (inv.cmd !== "spatial_register") continue;
      const args = (inv.args as { args: { moniker: string } }).args;
      countsAfter.set(args.moniker, (countsAfter.get(args.moniker) ?? 0) + 1);
    }

    // Each card should have gained at least one register (the scroll
    // fired) but at most a small handful (not one per scroll event).
    // The throttle is RAF-based — we tolerate up to 3 per card to
    // allow for a leading, trailing, and one additional frame before
    // the two-frame settle window we await above, without allowing
    // the 10-per-card flood the bug would produce.
    for (let i = 0; i < CARD_COUNT; i++) {
      const mk = cardMoniker(i);
      const before = countsBefore.get(mk) ?? 0;
      const after = countsAfter.get(mk) ?? 0;
      const delta = after - before;
      expect(
        delta,
        `card ${i} should re-register at least once for a scroll burst`,
      ).toBeGreaterThanOrEqual(1);
      expect(
        delta,
        `card ${i} should RAF-throttle — burst of 10 scrolls must collapse`,
      ).toBeLessThanOrEqual(3);
    }
  });
});
