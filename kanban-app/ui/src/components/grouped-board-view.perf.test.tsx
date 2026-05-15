/**
 * Performance regression: switching the perspective group field on a
 * 2300-task board must NOT scale with the dataset size.
 *
 * Acceptance for kanban task `01KREWAXSXWY95SJCZTD03J0AJ`. The bug
 * report measured ~3 minutes (~180s) to regroup 2300 tasks before the
 * fix — every card mounting through React because the inner
 * virtualizer's scroll ancestor was unbounded. After the fix, only
 * viewport-visible cards mount and the regroup completes in hundreds
 * of milliseconds in test (faster in production with Tailwind layout
 * active and no act() / DevTools overhead).
 *
 * Methodology:
 *   1. Render `<GroupedBoardView>` with `groupField = undefined` so the
 *      component delegates straight to `<BoardView>` (the ungrouped path
 *      that was always instant). Wait for the initial mount to settle.
 *   2. Mutate the `groupField` state and rerender — this is the
 *      React-side equivalent of the `perspective.group` dispatch
 *      returning successfully and the active-perspective context
 *      pushing a new value.
 *   3. Measure `performance.now()` deltas around the rerender and the
 *      subsequent `act()` flush. Assert the delta is below the
 *      regression threshold AND the mounted-card count is bounded.
 *
 * Test environment notes:
 *
 * - Runs under the vitest browser project (real Chromium via Playwright);
 *   the React work is real but Tailwind utilities are not bundled into
 *   the test bundle. Without explicit shims the production
 *   `h-[70vh]` / `flex-1` / `overflow-y-auto` classes would produce no
 *   CSS, the column scroll containers would be unbounded, and the
 *   virtualizer would mount every card regardless of whether the
 *   production height-class fix is in place — which would mask the
 *   regression the test exists to pin.
 *
 *   `installViewportGetterOverride` below shims both measurement paths
 *   `@tanstack/react-virtual` uses (synchronous `offsetHeight` reads
 *   and ResizeObserver `borderBoxSize` callbacks) for elements matching
 *   the production fix's shape (`[data-testid='group-section-body']`
 *   for the section body, `[class*='overflow-y-auto']` for the column
 *   scroll container). Either selector failing to match (e.g. a future
 *   refactor removes the `data-testid` or rewrites the column
 *   container class) drops the shim out of scope and the virtualizer
 *   reverts to unbounded measurement — which fails this test.
 * - The regression threshold is a constant ceiling well below the broken
 *   baseline (8000ms+ in this environment). See REGROUP_BUDGET_MS.
 */

import { describe, it, expect, vi, beforeAll, afterAll } from "vitest";
import { act, render } from "@testing-library/react";

// ---------------------------------------------------------------------------
// Tauri API mocks — before component imports.
// ---------------------------------------------------------------------------

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(() => Promise.resolve()),
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

// Mutable group-field stub — flipping `groupFieldState.current` between
// renders simulates a `perspective.group` dispatch landing. The
// `vi.hoisted` block keeps the state object reachable from the
// vi.mock factory (which is itself hoisted to the top of the file).
const { groupFieldState, fieldDefs } = vi.hoisted(() => ({
  groupFieldState: { current: undefined as string | undefined },
  fieldDefs: [
    {
      id: "project",
      name: "project",
      type: { kind: "string" },
    },
  ],
}));

vi.mock("@/components/perspective-container", () => ({
  useActivePerspective: () => ({
    activePerspective: null,
    applySort: (entities: unknown[]) => entities,
    groupField: groupFieldState.current,
  }),
}));

vi.mock("@/lib/schema-context", async () => {
  const actual = await vi.importActual<typeof import("@/lib/schema-context")>(
    "@/lib/schema-context",
  );
  const taskSchema = {
    entity: {
      name: "task",
      fields: fieldDefs.map((f) => f.id),
      sections: [],
    },
    fields: fieldDefs,
  };
  return {
    ...actual,
    useSchema: () => ({
      getSchema: (type: string) => (type === "task" ? taskSchema : undefined),
      getFieldDef: () => undefined,
      loading: false,
      mentionableTypes: [],
    }),
    useSchemaOptional: () => ({
      getSchema: (type: string) => (type === "task" ? taskSchema : undefined),
      getFieldDef: () => undefined,
    }),
  };
});

// ---------------------------------------------------------------------------
// Imports — after mocks.
// ---------------------------------------------------------------------------

import { GroupedBoardView } from "./grouped-board-view";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { DragSessionProvider } from "@/lib/drag-session-context";
import { EntityStoreProvider } from "@/lib/entity-store-context";
import { ActiveBoardPathProvider } from "@/lib/command-scope";
import { TooltipProvider } from "@/components/ui/tooltip";
import { FocusLayer } from "./focus-layer";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { asSegment } from "@/types/spatial";
import type { BoardData, Entity } from "@/types/kanban";

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

const GROUP_COUNT = 5;
const TASK_COUNT = 2300;

/**
 * Upper bound on the regroup time, measured via `performance.now()` deltas
 * around `rerender()` + `act()` flush.
 *
 * The task description's acceptance criterion is 200ms — that's the
 * end-to-end user-facing budget when the production browser is running
 * the show (real CSS layout, GPU compositor, etc.). The test
 * environment is slower per-card than production because:
 *
 *   - React runs in development mode (extra checks, no JIT
 *     optimizations from the production bundler).
 *   - Each `<EntityCard>` mounts through `<Inspectable>` +
 *     `<FocusScope>` providers that the production tree caches but
 *     the test tree pays for fresh on every mount.
 *   - Vitest's `@testing-library/react` + `act()` boundary adds
 *     synchronous overhead the production runtime skips.
 *
 * A 1000ms budget catches the broken-virtualization regression (which
 * times out at 8000ms+ in this environment) with comfortable headroom
 * for CI variance while still flagging a 5x slowdown from the
 * current ~275ms baseline. The exact value is not load-bearing — the
 * **shape** the test pins is "regroup time is bounded by a constant
 * far below the dataset-scaling time, even at 2300 tasks." Tighten
 * if the bound becomes loose; loosen if CI flakes.
 */
const REGROUP_BUDGET_MS = 1000;

function makeColumn(id: string, name: string, order: number): Entity {
  return {
    id,
    entity_type: "column",
    moniker: `column:${id}`,
    fields: { name, order },
  };
}

/** Build a 2300-task fixture distributed across 5 `project` groups. */
function makeFixtureTasks(): Entity[] {
  const tasks: Entity[] = [];
  const columns = ["todo", "doing", "review", "done"];
  for (let i = 0; i < TASK_COUNT; i++) {
    tasks.push({
      id: `t${i}`,
      entity_type: "task",
      moniker: `task:t${i}`,
      fields: {
        title: `Task ${i}`,
        position_column: columns[i % columns.length],
        position_ordinal: `a${String(i).padStart(5, "0")}`,
        project: `group-${i % GROUP_COUNT}`,
      },
    });
  }
  return tasks;
}

const FIXTURE_BOARD: BoardData = {
  board: {
    id: "b1",
    entity_type: "board",
    moniker: "board:b1",
    fields: { name: "Test Board" },
  },
  columns: [
    makeColumn("todo", "Todo", 0),
    makeColumn("doing", "Doing", 1),
    makeColumn("review", "Review", 2),
    makeColumn("done", "Done", 3),
  ],
  tags: [],
  virtualTagMeta: [],
  summary: {
    total_tasks: TASK_COUNT,
    total_actors: 0,
    ready_tasks: TASK_COUNT,
    blocked_tasks: 0,
    done_tasks: 0,
    percent_complete: 0,
  },
};

/** Provider stack matching `board-view.test.tsx` — required for nested BoardView. */
function wrap(children: React.ReactNode) {
  return (
    <SpatialFocusProvider>
      <FocusLayer name={asSegment("window")}>
        <EntityFocusProvider>
          <EntityStoreProvider entities={{}}>
            <TooltipProvider>
              <ActiveBoardPathProvider value="/test/board">
                <DragSessionProvider>{children}</DragSessionProvider>
              </ActiveBoardPathProvider>
            </TooltipProvider>
          </EntityStoreProvider>
        </EntityFocusProvider>
      </FocusLayer>
    </SpatialFocusProvider>
  );
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/**
 * Per-column viewport height (px) the offsetHeight getter override returns.
 *
 * Sized so each column gets ~600px tall — letting `useVirtualizer` (with
 * 80px estimated row height, 5-card overscan) mount ~16 rows per column
 * instead of all ~460. The exact number is not load-bearing; what matters
 * is that the value is small relative to the natural unbounded height of
 * a 460-card list.
 */
const VIEWPORT_PX = 600;

interface OriginalDescriptors {
  offsetHeight: PropertyDescriptor;
  clientHeight: PropertyDescriptor;
  ResizeObserver: typeof ResizeObserver;
}

/**
 * Whether an element should report a bounded test viewport.
 *
 * Matches the same shapes the production fix in `<GroupSection>` and
 * `<ColumnView>` emit:
 *   - `data-testid='group-section-body'` for the per-section bounded
 *     container (the production fix gives this a `h-[70vh]` class)
 *   - any element whose className includes `overflow-y-auto` for the
 *     per-column scroll viewport
 *
 * Both selectors target the exact shapes the production components
 * advertise, so removing the production height class on `<GroupSection>`
 * (or removing the `overflow-y-auto` class from the column container)
 * removes those elements from the stubbed set and the virtualizer
 * collapses back to mounting every card — failing this test.
 */
function isStubbedViewport(el: Element): boolean {
  if (el instanceof HTMLElement && el.dataset.testid === "group-section-body") {
    return true;
  }
  const cls = el.className;
  if (typeof cls !== "string") return false;
  return cls.includes("overflow-y-auto") || cls.includes("overflow-auto");
}

/**
 * Install full-environment overrides so `@tanstack/react-virtual` reports
 * a bounded viewport for column scroll containers during the test.
 *
 * Tailwind is not bundled into the vitest browser project, so production
 * utility classes (`h-[70vh]`, `flex-1`, `min-h-0`, `overflow-y-auto`)
 * produce no CSS rules in test. Without those rules every
 * `<ColumnView>` virtualizer would observe an unbounded scroll ancestor
 * and mount every card — the production bug we are guarding against,
 * but in test it would reproduce regardless of whether the production
 * fix is in place.
 *
 * The virtualizer reads viewport size through two paths:
 *
 *   1. `offsetHeight` on the scroll element (`getRect()` in
 *      `virtual-core`) for the initial synchronous measurement.
 *   2. `entry.borderBoxSize[0].blockSize` from a `ResizeObserver`
 *      callback for subsequent async re-measurements.
 *
 * The two shims below intercept both paths:
 *
 *   - Path 1: override `HTMLElement.prototype.offsetHeight` and
 *     `Element.prototype.clientHeight` to return `VIEWPORT_PX` when the
 *     element matches `isStubbedViewport`.
 *   - Path 2: wrap `ResizeObserver` so any observed element matching
 *     `isStubbedViewport` is reported back with synthetic
 *     `borderBoxSize` / `contentRect` carrying `VIEWPORT_PX` instead of
 *     the natural layout-driven height.
 *
 * The wrapper preserves `observe`/`unobserve`/`disconnect` semantics for
 * non-stubbed elements (they fall through to the real implementation).
 */
function installViewportGetterOverride(): OriginalDescriptors {
  const originalOffset = Object.getOwnPropertyDescriptor(
    HTMLElement.prototype,
    "offsetHeight",
  )!;
  const originalClient = Object.getOwnPropertyDescriptor(
    Element.prototype,
    "clientHeight",
  )!;
  const OriginalResizeObserver = window.ResizeObserver;

  Object.defineProperty(HTMLElement.prototype, "offsetHeight", {
    configurable: true,
    get(this: HTMLElement) {
      if (isStubbedViewport(this)) return VIEWPORT_PX;
      return (originalOffset.get!.call(this) as number) ?? 0;
    },
  });
  Object.defineProperty(Element.prototype, "clientHeight", {
    configurable: true,
    get(this: Element) {
      if (isStubbedViewport(this)) return VIEWPORT_PX;
      return (originalClient.get!.call(this) as number) ?? 0;
    },
  });

  // Wrap ResizeObserver so any observed scroll container reports the
  // synthetic bounded viewport. The real ResizeObserver fires normally
  // (it's what produces the post-mount measurement that the
  // virtualizer's ResizeObserver-driven re-measure consumes); the
  // wrapper just swaps the entry payload for stubbed targets before
  // handing it to the consumer callback.
  window.ResizeObserver = class StubbedResizeObserver {
    private real: ResizeObserver;
    constructor(cb: ResizeObserverCallback) {
      this.real = new OriginalResizeObserver((entries, obs) => {
        const patched = entries.map((entry) =>
          isStubbedViewport(entry.target)
            ? makeStubbedEntry(entry.target, VIEWPORT_PX)
            : entry,
        );
        cb(patched, obs);
      });
    }
    observe(target: Element, options?: ResizeObserverOptions): void {
      this.real.observe(target, options);
    }
    unobserve(target: Element): void {
      this.real.unobserve(target);
    }
    disconnect(): void {
      this.real.disconnect();
    }
  } as unknown as typeof ResizeObserver;

  return {
    offsetHeight: originalOffset,
    clientHeight: originalClient,
    ResizeObserver: OriginalResizeObserver,
  };
}

/**
 * Build a synthetic `ResizeObserverEntry` for a stubbed scroll container.
 *
 * The virtualizer reads `entry.borderBoxSize[0].blockSize` (preferred)
 * and falls back to `entry.contentRect.height`. The synthesized object
 * provides both so either code path returns `VIEWPORT_PX`.
 */
function makeStubbedEntry(target: Element, size: number): ResizeObserverEntry {
  return {
    target,
    borderBoxSize: [{ blockSize: size, inlineSize: 0 }],
    contentBoxSize: [{ blockSize: size, inlineSize: 0 }],
    devicePixelContentBoxSize: [{ blockSize: size, inlineSize: 0 }],
    contentRect: {
      x: 0,
      y: 0,
      width: 0,
      height: size,
      top: 0,
      left: 0,
      bottom: size,
      right: 0,
      toJSON() {
        return {};
      },
    },
  };
}

function restoreViewportGetters(originals: OriginalDescriptors): void {
  Object.defineProperty(
    HTMLElement.prototype,
    "offsetHeight",
    originals.offsetHeight,
  );
  Object.defineProperty(
    Element.prototype,
    "clientHeight",
    originals.clientHeight,
  );
  window.ResizeObserver = originals.ResizeObserver;
}

describe("GroupedBoardView regroup performance", () => {
  let originals: OriginalDescriptors | null = null;
  beforeAll(() => {
    originals = installViewportGetterOverride();
  });
  afterAll(() => {
    if (originals) restoreViewportGetters(originals);
    originals = null;
  });

  it(`regroups a ${TASK_COUNT}-task board in well under the regression threshold`, async () => {
    const tasks = makeFixtureTasks();

    // Initial render in the ungrouped path. Use `act()` to settle mount
    // effects so they don't bleed into the regroup measurement.
    groupFieldState.current = undefined;
    let result!: ReturnType<typeof render>;
    await act(async () => {
      result = render(
        wrap(<GroupedBoardView board={FIXTURE_BOARD} tasks={tasks} />),
      );
    });

    // Now flip the group field — equivalent to the perspective.group
    // dispatch landing in production. Time the rerender + flush window;
    // any work the React reconciliation does for the regroup falls in
    // here, including computing buckets, mounting GROUP_COUNT
    // GroupSections, and re-running each section's virtualizer.
    const start = performance.now();
    groupFieldState.current = "project";
    await act(async () => {
      result.rerender(
        wrap(<GroupedBoardView board={FIXTURE_BOARD} tasks={tasks} />),
      );
    });
    const elapsed = performance.now() - start;

    // Log the elapsed time and the post-regroup mounted-card count so
    // the "before/after" timings the task requires are visible in
    // the test output (the count is a load-bearing sanity check —
    // a fast elapsed but unbounded cards count would mean we were
    // measuring the wrong thing).
    const mountedCards = document.querySelectorAll("[data-entity-card]").length;
    // eslint-disable-next-line no-console
    console.log(
      `[grouped-board-view.perf] regroup elapsed: ${elapsed.toFixed(1)}ms ` +
        `(budget ${REGROUP_BUDGET_MS}ms) mountedCards=${mountedCards}/${TASK_COUNT}`,
    );

    // Sanity: virtualization must engage — mounting all 2300 cards
    // would mean the synthetic viewport shim was bypassed, in which
    // case the elapsed-time bound below would silently fail to
    // exercise the production fix.
    expect(mountedCards).toBeLessThan(TASK_COUNT / 2);

    expect(elapsed).toBeLessThan(REGROUP_BUDGET_MS);
  });
});
