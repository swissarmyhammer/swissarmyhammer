/**
 * Browser-mode integration test for the scroll-on-edge fall-through that
 * lets cardinal nav cross the boundary of a virtualized scroll container.
 *
 * Pins kanban task `01KQQV1FDQXGBJ70ZRMP7AG66J` (Spatial-nav #5).
 *
 * The kernel-side cardinal pick (#1, geometric pick) only knows about
 * registered scope rectangles. Off-viewport cards in a virtualized column
 * are unmounted and therefore not registered, so when the user is on the
 * last visible card and presses ArrowDown the kernel returns stay-put —
 * focus cannot cross the virtual boundary. The React-side glue in
 * `lib/scroll-on-edge.ts` (`runNavWithScrollOnEdge`) detects the stay-put
 * outcome, scrolls the column body by one item-height, and re-dispatches
 * nav so the freshly-mounted next row picks up focus.
 *
 * Test cases:
 *
 * 1. ArrowDown on last visible card scrolls the column AND re-dispatches
 *    nav (two `spatial_navigate` IPCs, scrollTop > before).
 * 2. When the column is already scrolled to the bottom, the fall-through
 *    does NOT fire — only one `spatial_navigate` IPC, no extra scroll.
 * 3. The retry depth is capped at 1: even if the second navigate also
 *    returns stay-put, no third dispatch fires (no infinite loop).
 * 4. ArrowRight from a card in the rightmost visible column of an
 *    overflowing column strip scrolls the strip horizontally AND
 *    re-dispatches nav (two `spatial_navigate` IPCs, scrollLeft >
 *    before). This pins acceptance criterion #2 — the same
 *    `runNavWithScrollOnEdge` plumbing that makes vertical work must
 *    cross a horizontal virtualization / overflow boundary.
 *
 * Companion test in `column-view.scroll-rects.browser.test.tsx` verifies
 * rect-tracking on scroll (the input the kernel uses to pick a target);
 * this test pins the React-side fall-through that runs when the kernel
 * still cannot find a candidate.
 */

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, act } from "@testing-library/react";
import type { Entity } from "@/types/kanban";

// ---------------------------------------------------------------------------
// Tauri API mocks — must come before component imports.
// ---------------------------------------------------------------------------

type ListenCallback = (event: { payload: unknown }) => void;

const { mockInvoke, mockListen, listeners } = vi.hoisted(() => {
  const listeners = new Map<string, ListenCallback[]>();
  const mockInvoke = vi.fn(
    async (_cmd: string, _args?: unknown): Promise<unknown> => undefined,
  );
  const mockListen = vi.fn(
    (eventName: string, cb: ListenCallback): Promise<() => void> => {
      const cbs = listeners.get(eventName) ?? [];
      cbs.push(cb);
      listeners.set(eventName, cbs);
      return Promise.resolve(() => {
        const arr = listeners.get(eventName);
        if (arr) {
          const idx = arr.indexOf(cb);
          if (idx >= 0) arr.splice(idx, 1);
        }
      });
    },
  );
  return { mockInvoke, mockListen, listeners };
});

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...a: unknown[]) => mockInvoke(...(a as [string, unknown?])),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: (...a: Parameters<typeof mockListen>) => mockListen(...a),
  emit: vi.fn(() => Promise.resolve()),
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

import "@/components/fields/registrations";
import { ColumnView } from "./column-view";
import { FocusLayer } from "./focus-layer";
import { FocusZone } from "./focus-zone";
import {
  SpatialFocusProvider,
  useSpatialFocusActions,
  type SpatialFocusActions,
} from "@/lib/spatial-focus-context";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { SchemaProvider } from "@/lib/schema-context";
import { EntityStoreProvider } from "@/lib/entity-store-context";
import { FieldUpdateProvider } from "@/lib/field-update-context";
import { UIStateProvider } from "@/lib/ui-state-context";
import { ActiveBoardPathProvider } from "@/lib/command-scope";
import { TooltipProvider } from "@/components/ui/tooltip";
import { asSegment, type FullyQualifiedMoniker } from "@/types/spatial";
import { runNavWithScrollOnEdge } from "@/lib/scroll-on-edge";
import { installKernelSimulator } from "@/test-helpers/kernel-simulator";
import { useEffect } from "react";

// ---------------------------------------------------------------------------
// Test fixtures
// ---------------------------------------------------------------------------

/** Identity-stable column id used by every fixture in this file. */
const COLUMN_ID = "01ABCDEFGHJKMNPQRSTVWXYZ01";

/** Build a column entity (defaults to the single-column fixture id). */
function makeColumn(id: string = COLUMN_ID, name: string = "To Do"): Entity {
  return {
    entity_type: "column",
    id,
    moniker: `column:${id}`,
    fields: { name },
  };
}

/**
 * Build a deterministic task entity. The id is left-padded so the moniker
 * matches the production format and the kernel-simulator's IPC trace stays
 * readable. `columnId` defaults to the single-column fixture id; the
 * horizontal-strip test passes its own column ids in.
 */
function makeTask(
  index: number,
  columnId: string = COLUMN_ID,
  idPrefix: string = "01TASK",
): Entity {
  const id = `${idPrefix}${String(index).padStart(26 - idPrefix.length, "0")}`;
  return {
    entity_type: "task",
    id,
    moniker: `task:${id}`,
    fields: {
      title: `Task ${index}`,
      position_column: columnId,
      position_ordinal: `a${String(index).padStart(4, "0")}`,
    },
  };
}

/**
 * Minimal task schema. Only `title` is needed because the cards render
 * their `title` field in the `header` section.
 */
const TASK_SCHEMA = {
  entity: {
    name: "task",
    entity_type: "task",
    fields: ["title"],
    sections: [{ id: "header", on_card: true }],
  },
  fields: [
    {
      id: "f-title",
      name: "title",
      type: { kind: "markdown", single_line: true },
      editor: "markdown",
      display: "text",
      section: "header",
    },
  ],
} as unknown as import("@/types/kanban").EntitySchema;

// ---------------------------------------------------------------------------
// Default fallback for non-spatial IPCs
// ---------------------------------------------------------------------------

async function fallbackInvoke(cmd: string, args?: unknown): Promise<unknown> {
  if (cmd === "list_entity_types") return ["task"];
  if (cmd === "get_entity_schema") {
    const entityType = (args as { entityType?: string })?.entityType;
    if (entityType === "task") return TASK_SCHEMA;
    return null;
  }
  if (cmd === "get_ui_state") {
    return {
      palette_open: false,
      palette_mode: "command",
      keymap_mode: "cua",
      scope_chain: [],
      open_boards: [],
      windows: {},
      recent_boards: [],
    };
  }
  if (cmd === "get_undo_state") return { can_undo: false, can_redo: false };
  if (cmd === "list_commands_for_scope") return [];
  if (cmd === "show_context_menu") return undefined;
  if (cmd === "dispatch_command") return undefined;
  return undefined;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Two-tick microtask flush so register effects settle. */
async function flushSetup() {
  await act(async () => {
    await Promise.resolve();
  });
  await act(async () => {
    await Promise.resolve();
  });
}

/** Wait for one animation frame plus a microtask flush. */
async function flushFrame() {
  await act(async () => {
    await new Promise<void>((resolve) =>
      requestAnimationFrame(() => resolve()),
    );
    await Promise.resolve();
  });
}

/** Pull every `spatial_navigate` invocation argument bag, in order. */
function spatialNavigateCalls(): Array<{
  focusedFq: FullyQualifiedMoniker;
  direction: string;
}> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "spatial_navigate")
    .map(
      (c) =>
        c[1] as {
          focusedFq: FullyQualifiedMoniker;
          direction: string;
        },
    );
}

/** Find the test wrapper that owns the outer scroll. */
function findOuterScroller(container: HTMLElement): HTMLElement {
  const node = container.querySelector(
    "[data-testid='board-shell']",
  ) as HTMLElement | null;
  if (!node) {
    throw new Error(
      "expected to find the test wrapper [data-testid='board-shell']",
    );
  }
  return node;
}

// ---------------------------------------------------------------------------
// Render helpers
// ---------------------------------------------------------------------------

/**
 * `<ActionsCapture>` exposes the live `SpatialFocusActions` to the test by
 * writing them into the supplied ref on mount. Tests use the captured
 * actions to drive `runNavWithScrollOnEdge` directly — exactly what the
 * `nav.down` command's `execute` closure does in production.
 */
function ActionsCapture({
  actionsRef,
}: {
  actionsRef: { current: SpatialFocusActions | null };
}) {
  const actions = useSpatialFocusActions();
  useEffect(() => {
    actionsRef.current = actions;
  }, [actions, actionsRef]);
  return null;
}

/**
 * Render a `<ColumnView>` inside a fixed-height ancestor that forces the
 * column body to scroll. Mirrors the fixture in
 * `column-view.scroll-rects.browser.test.tsx`. The vitest browser project
 * does not bundle Tailwind, so the column-view's inner scroller produces
 * no CSS rules — drive scroll from the outer wrapper instead.
 */
function renderColumn(
  column: Entity,
  tasks: Entity[],
  actionsRef: { current: SpatialFocusActions | null },
) {
  return render(
    <SpatialFocusProvider>
      <FocusLayer name={asSegment("window")}>
        <EntityFocusProvider>
          <SchemaProvider>
            <EntityStoreProvider entities={{ task: tasks }}>
              <FieldUpdateProvider>
                <UIStateProvider>
                  <TooltipProvider>
                    <ActiveBoardPathProvider value="/test/board">
                      <ActionsCapture actionsRef={actionsRef} />
                      <div
                        data-testid="board-shell"
                        style={{
                          height: "400px",
                          width: "400px",
                          overflowY: "auto",
                          overflowX: "hidden",
                        }}
                      >
                        <FocusZone moniker={asSegment("ui:board")}>
                          <ColumnView column={column} tasks={tasks} />
                        </FocusZone>
                      </div>
                    </ActiveBoardPathProvider>
                  </TooltipProvider>
                </UIStateProvider>
              </FieldUpdateProvider>
            </EntityStoreProvider>
          </SchemaProvider>
        </EntityFocusProvider>
      </FocusLayer>
    </SpatialFocusProvider>,
  );
}

/**
 * Render `<ColumnView>` instances for a subset of columns inside a
 * horizontally-scrolling wrapper. Mirrors the production board-strip
 * layout (`overflow-x: auto`, fixed-width column items) but leaves a
 * trailing intrinsic-width spacer so the wrapper accumulates horizontal
 * scroll travel even when the right-side columns are *not* mounted —
 * the same shape essential virtualization produces in production.
 *
 * `mountedColumns` are the only columns that actually render their
 * `<ColumnView>` (and therefore the only ones whose cards register
 * spatial-nav scopes). `totalStripWidthPx` controls how wide the
 * intrinsic strip is — used to force the wrapper into overflow even
 * when only a couple of columns are mounted.
 *
 * Used by the column-strip horizontal e2e test: the rightmost mounted
 * column's last card sits at the wrapper's right edge with nothing
 * registered to its right, exactly mirroring the virtualization
 * boundary scroll-on-edge is designed to cross.
 */
function renderColumnStrip(opts: {
  mountedColumns: Entity[];
  tasksByColumn: Map<string, Entity[]>;
  totalStripWidthPx: number;
  actionsRef: { current: SpatialFocusActions | null };
}) {
  const { mountedColumns, tasksByColumn, totalStripWidthPx, actionsRef } = opts;
  const allTasks = mountedColumns.flatMap(
    (c) => tasksByColumn.get(c.id) ?? [],
  );
  return render(
    <SpatialFocusProvider>
      <FocusLayer name={asSegment("window")}>
        <EntityFocusProvider>
          <SchemaProvider>
            <EntityStoreProvider entities={{ task: allTasks }}>
              <FieldUpdateProvider>
                <UIStateProvider>
                  <TooltipProvider>
                    <ActiveBoardPathProvider value="/test/board">
                      <ActionsCapture actionsRef={actionsRef} />
                      <div
                        data-testid="board-shell"
                        style={{
                          // Scrolls horizontally; vertical is clipped so
                          // scroll-on-edge can only latch onto this
                          // wrapper on the X axis.
                          height: "400px",
                          width: "600px",
                          overflowX: "auto",
                          overflowY: "hidden",
                          display: "flex",
                          flexDirection: "row",
                        }}
                      >
                        <FocusZone moniker={asSegment("ui:board")}>
                          <div
                            style={{
                              display: "flex",
                              flexDirection: "row",
                              gap: "8px",
                              // Force the strip's intrinsic width past
                              // the wrapper's so horizontal overflow
                              // exists regardless of how many columns
                              // are actually mounted.
                              width: `${totalStripWidthPx}px`,
                              minWidth: `${totalStripWidthPx}px`,
                              flexShrink: 0,
                            }}
                          >
                            {mountedColumns.map((col) => (
                              <div
                                key={col.id}
                                style={{
                                  width: "280px",
                                  flexShrink: 0,
                                }}
                              >
                                <ColumnView
                                  column={col}
                                  tasks={tasksByColumn.get(col.id) ?? []}
                                />
                              </div>
                            ))}
                          </div>
                        </FocusZone>
                      </div>
                    </ActiveBoardPathProvider>
                  </TooltipProvider>
                </UIStateProvider>
              </FieldUpdateProvider>
            </EntityStoreProvider>
          </SchemaProvider>
        </EntityFocusProvider>
      </FocusLayer>
    </SpatialFocusProvider>,
  );
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("<ColumnView> — scroll-on-edge fall-through for virtualized nav", () => {
  beforeEach(() => {
    mockInvoke.mockClear();
    mockListen.mockClear();
    listeners.clear();
    // Install the kernel simulator so spatial_navigate emits realistic
    // focus-changed events (including stay-put echoes).
    installKernelSimulator(mockInvoke, listeners, fallbackInvoke);
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  // -------------------------------------------------------------------------
  // 1. ArrowDown on the last visible card scrolls the column AND re-dispatches.
  // -------------------------------------------------------------------------

  it("scrolls the column and re-dispatches nav when the kernel returns stay-put", async () => {
    const column = makeColumn();
    const tasks = Array.from({ length: 50 }, (_, i) => makeTask(i));
    const actionsRef: { current: SpatialFocusActions | null } = {
      current: null,
    };
    const { container, unmount } = renderColumn(column, tasks, actionsRef);
    await flushSetup();
    await flushFrame();

    const scroller = findOuterScroller(container);
    expect(scroller.scrollHeight).toBeGreaterThan(scroller.clientHeight);

    // Pick the last currently-visible card. Its bounding rect is at the
    // bottom edge of the visible viewport, so ArrowDown will not find a
    // registered peer below it (the off-viewport cards are unmounted).
    const visibleCards = Array.from(
      container.querySelectorAll("[data-segment^='task:']"),
    ) as HTMLElement[];
    expect(visibleCards.length).toBeGreaterThan(0);
    const lastVisible = visibleCards[visibleCards.length - 1];
    const focusedFq = lastVisible.getAttribute(
      "data-moniker",
    ) as FullyQualifiedMoniker;
    expect(focusedFq).toBeTruthy();

    // Drive focus through the kernel so `actions.focusedFq()` reads as
    // the last-visible card's FQM.
    const actions = actionsRef.current!;
    expect(actions).not.toBeNull();
    await act(async () => {
      await actions.focus(focusedFq);
    });
    await flushFrame();

    expect(actions.focusedFq()).toBe(focusedFq);

    const beforeNavCount = spatialNavigateCalls().length;
    const beforeScrollTop = scroller.scrollTop;

    await act(async () => {
      await runNavWithScrollOnEdge(actions, "down");
    });
    await flushFrame();

    const navCalls = spatialNavigateCalls();
    expect(navCalls.length - beforeNavCount).toBe(2);
    expect(scroller.scrollTop).toBeGreaterThan(beforeScrollTop);

    unmount();
  });

  // -------------------------------------------------------------------------
  // 2. Negative test — fully-scrolled column does NOT fire the fall-through.
  // -------------------------------------------------------------------------

  it("does not re-dispatch when the column is already at the bottom edge", async () => {
    const column = makeColumn();
    const tasks = Array.from({ length: 50 }, (_, i) => makeTask(i));
    const actionsRef: { current: SpatialFocusActions | null } = {
      current: null,
    };
    const { container, unmount } = renderColumn(column, tasks, actionsRef);
    await flushSetup();
    await flushFrame();

    const scroller = findOuterScroller(container);

    // Scroll to the very bottom — no remaining travel down.
    await act(async () => {
      scroller.scrollTop = scroller.scrollHeight - scroller.clientHeight;
      scroller.dispatchEvent(new Event("scroll"));
    });
    await flushFrame();
    await flushFrame();

    const visibleCards = Array.from(
      container.querySelectorAll("[data-segment^='task:']"),
    ) as HTMLElement[];
    expect(visibleCards.length).toBeGreaterThan(0);
    const lastVisible = visibleCards[visibleCards.length - 1];
    const focusedFq = lastVisible.getAttribute(
      "data-moniker",
    ) as FullyQualifiedMoniker;

    const actions = actionsRef.current!;
    await act(async () => {
      await actions.focus(focusedFq);
    });
    await flushFrame();

    const beforeNavCount = spatialNavigateCalls().length;
    const beforeScrollTop = scroller.scrollTop;

    await act(async () => {
      await runNavWithScrollOnEdge(actions, "down");
    });
    await flushFrame();

    // Exactly one navigate IPC — the initial dispatch. No retry, no scroll.
    const navCalls = spatialNavigateCalls();
    expect(navCalls.length - beforeNavCount).toBe(1);
    expect(scroller.scrollTop).toBe(beforeScrollTop);

    unmount();
  });

  // -------------------------------------------------------------------------
  // 3. Retry depth is capped at 1 — no infinite loop.
  // -------------------------------------------------------------------------

  it("retries at most once even when the second nav also returns stay-put", async () => {
    const column = makeColumn();
    const tasks = Array.from({ length: 50 }, (_, i) => makeTask(i));
    const actionsRef: { current: SpatialFocusActions | null } = {
      current: null,
    };
    const { container, unmount } = renderColumn(column, tasks, actionsRef);
    await flushSetup();
    await flushFrame();

    // Pick a card and drive focus to it. We do NOT scroll the outer
    // wrapper to the edge here, so the helper finds an ancestor that
    // can scroll further. The kernel-simulator's `spatial_navigate`
    // returns stay-put for cards whose registered rect is at the
    // viewport edge with no peer below — which is the case for the
    // last visible card.
    const visibleCards = Array.from(
      container.querySelectorAll("[data-segment^='task:']"),
    ) as HTMLElement[];
    const lastVisible = visibleCards[visibleCards.length - 1];
    const focusedFq = lastVisible.getAttribute(
      "data-moniker",
    ) as FullyQualifiedMoniker;

    const actions = actionsRef.current!;
    await act(async () => {
      await actions.focus(focusedFq);
    });
    await flushFrame();

    const beforeNavCount = spatialNavigateCalls().length;

    await act(async () => {
      await runNavWithScrollOnEdge(actions, "down");
    });
    // Multiple frames to give any hypothetical infinite-retry loop time
    // to manifest. The retry-depth-1 cap means the navigate count tops
    // out at 2 regardless of how long we wait.
    await flushFrame();
    await flushFrame();
    await flushFrame();

    const navCalls = spatialNavigateCalls();
    const dispatched = navCalls.length - beforeNavCount;
    // Initial dispatch + at most one retry. Two is the maximum.
    expect(dispatched).toBeLessThanOrEqual(2);
    expect(dispatched).toBeGreaterThanOrEqual(1);

    unmount();
  });

  // -------------------------------------------------------------------------
  // 4. Horizontal column-strip e2e — ArrowRight at the right edge of the
  //    visible column strip scrolls horizontally AND re-dispatches nav so
  //    a previously-off-screen column gets focus. Pins acceptance
  //    criterion #2 of `01KQQV1FDQXGBJ70ZRMP7AG66J`.
  // -------------------------------------------------------------------------

  it("scrolls the column strip horizontally and re-dispatches nav from a card in the rightmost visible column", async () => {
    // Mount one column (280px) inside a 600px-wide wrapper, but
    // force the strip's intrinsic width to 1120px so the wrapper has
    // ~520px of unscrolled horizontal travel to its right. The
    // off-screen columns are not mounted — mirroring how essential
    // virtualization in production drops off-viewport columns from
    // the registry.
    //
    // The kernel-simulator's stay-put surface depends on registry
    // population details that are sensitive to virtualizer churn (an
    // off-screen card unregistered between focus and navigate causes
    // the cascade to bottom out at "from not in registry"). To pin
    // the column-strip wiring deterministically, this test installs
    // a custom `spatial_navigate` handler that *always* returns
    // stay-put for the `right` direction — exactly the kernel
    // behavior that scroll-on-edge is designed to handle. The helper
    // either re-dispatches once after scrolling (the contract under
    // test) or doesn't (which would be a regression).
    const mountedColumns = [makeColumn("colA", "Alpha")];
    const tasksByColumn = new Map<string, Entity[]>();
    tasksByColumn.set(
      "colA",
      Array.from({ length: 5 }, (_, i) => makeTask(i, "colA", "01TSKcolA")),
    );

    const actionsRef: { current: SpatialFocusActions | null } = {
      current: null,
    };
    const { container, unmount } = renderColumnStrip({
      mountedColumns,
      tasksByColumn,
      totalStripWidthPx: 1120,
      actionsRef,
    });
    await flushSetup();
    await flushFrame();

    const scroller = findOuterScroller(container);
    expect(scroller.scrollWidth).toBeGreaterThan(scroller.clientWidth);

    // Pick the last DOM-mounted card as the focused FQM. Position is
    // not critical here because the navigate handler is hard-coded
    // to return stay-put — what matters is that scroll-on-edge looks
    // up the right wrapper and advances its `scrollLeft`.
    const visibleCards = Array.from(
      container.querySelectorAll("[data-segment^='task:']"),
    ) as HTMLElement[];
    expect(visibleCards.length).toBeGreaterThan(0);
    const rightmostCard = visibleCards[visibleCards.length - 1];
    const focusedFq = rightmostCard.getAttribute(
      "data-moniker",
    ) as FullyQualifiedMoniker;
    expect(focusedFq).toBeTruthy();

    const actions = actionsRef.current!;
    expect(actions).not.toBeNull();
    await act(async () => {
      await actions.focus(focusedFq);
    });
    await flushFrame();
    expect(actions.focusedFq()).toBe(focusedFq);

    // Override `spatial_navigate` for this test only. The simulator's
    // earlier handler stays in place for every other IPC; we just
    // intercept navigate and emit a deterministic stay-put echo.
    // This pins the React-side wiring — the kernel's exact cascade
    // is exercised by the dedicated Rust tests.
    const navHandler = vi.fn(async (_cmd: string, args?: unknown) => {
      const a = (args ?? {}) as Record<string, unknown>;
      const fromFq = a.focusedFq as FullyQualifiedMoniker;
      // Stay-put echo — mirrors the kernel's no-silent-dropout emit.
      const handlers = listeners.get("focus-changed") ?? [];
      const segment = rightmostCard.getAttribute("data-segment") ?? null;
      queueMicrotask(() => {
        for (const h of handlers) {
          h({
            payload: {
              window_label: "main",
              prev_fq: fromFq,
              next_fq: fromFq,
              next_segment: segment,
            },
          });
        }
      });
      return undefined;
    });
    const originalImpl = mockInvoke.getMockImplementation();
    mockInvoke.mockImplementation(async (cmd: string, args?: unknown) => {
      if (cmd === "spatial_navigate") return navHandler(cmd, args);
      return originalImpl ? originalImpl(cmd, args) : undefined;
    });

    const beforeNavCount = navHandler.mock.calls.length;
    const beforeScrollLeft = scroller.scrollLeft;

    await act(async () => {
      await runNavWithScrollOnEdge(actions, "right");
    });
    await flushFrame();

    // Two `spatial_navigate` IPCs (initial stay-put + post-scroll
    // retry), and the wrapper advanced its horizontal scroll offset.
    expect(navHandler.mock.calls.length - beforeNavCount).toBe(2);
    expect(scroller.scrollLeft).toBeGreaterThan(beforeScrollLeft);
    // Both IPCs went out as `right` — the helper re-dispatches the
    // same direction, so the trace is unambiguous about what the
    // user asked for.
    const directions = navHandler.mock.calls
      .slice(beforeNavCount)
      .map((c) => (c[1] as { direction: string }).direction);
    expect(directions).toEqual(["right", "right"]);

    unmount();
  });
});
