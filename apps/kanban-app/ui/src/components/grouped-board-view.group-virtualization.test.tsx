/**
 * Regression: `<GroupedBoardView>` must virtualize its outer group list.
 *
 * Acceptance for kanban task `01KRHH5WKRZGPVVMP9MBD8WRVG`. When the user
 * groups by a high-cardinality field (the trigger case is `tags` — any
 * multi-bucket field reproduces) `<GroupedBoardView>` previously mounted
 * every `<GroupSection>` at once. Each section instantiates a full
 * `<BoardView>` tree (one inner virtualizer per column). The inner
 * virtualizer from task `01KREWAXSXWY95SJCZTD03J0AJ` correctly windows
 * cards *inside* a section, but the *count* of mounted sections still
 * scaled with group count — at 100+ groups the cost of mounting that
 * many BoardView trees became the bottleneck.
 *
 * The fix wraps the group list in a second `useVirtualizer` so only
 * viewport-visible group sections are present in the DOM. The
 * per-section card virtualization is preserved unchanged.
 *
 * Test environment notes mirror `grouped-board-view.perf.test.tsx`:
 *
 *   - Tailwind utilities are not bundled into the vitest browser
 *     project, so `h-[70vh]`, `flex-1`, `min-h-0`, and `overflow-y-auto`
 *     produce no CSS rules. We install the same `offsetHeight` +
 *     `clientHeight` + `ResizeObserver` shims the perf test uses, plus
 *     a shim for the outer grouped-board-view scroll container itself
 *     so the *outer* virtualizer measures a bounded viewport.
 *   - The outer scroll container in production carries
 *     `className="flex flex-col flex-1 min-h-0 overflow-y-auto"` — the
 *     same `overflow-y-auto` selector the existing shim already
 *     matches, so no new shim shape is required. The outer container
 *     also receives a fixed pixel height via `BOARD_VIEWPORT_PX` so the
 *     virtualizer's `getScrollElement().clientHeight` returns a
 *     definite value at measurement time.
 *
 * The contracts this file pins:
 *
 *   1. mounted_group_section_count_is_bounded_by_viewport — only a
 *      handful of `<GroupSection>` roots (carrying `data-group-section`)
 *      are present in the DOM, regardless of the 200-group fixture
 *      size.
 *   2. regrouping_high_cardinality_field_completes_under_budget —
 *      flipping `groupField` from `undefined` to a 200-group field on a
 *      ~2000-task board completes within `REGROUP_BUDGET_MS`. Mounted
 *      section and card counts stay bounded.
 *   3. collapse_state_survives_outer_scroll_recycling — the regression
 *      pin for hoisting collapse state out of `<GroupSection>` into the
 *      parent. Without hoisted state, recycling the section
 *      (`useState` dies with the unmount) would drop the user's
 *      collapsed/expanded choice.
 *   4. outer_scroll_container_uses_overflow_y_auto — sanity check the
 *      production CSS contract the existing perf-test shim relies on
 *      stays in place.
 */

import {
  describe,
  it,
  expect,
  vi,
  beforeAll,
  afterAll,
  beforeEach,
} from "vitest";
import { act, render, fireEvent } from "@testing-library/react";

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

// Mutable group-field + drag-session stubs — flipping
// `groupFieldState.current` between renders simulates the
// `perspective.group` dispatch landing, and `dragSessionState.current`
// drives the `useDragSession()` mock so tests can exercise the
// drag-suspends-virtualization path. `vi.hoisted` keeps the state
// objects reachable from the `vi.mock` factories which are themselves
// hoisted to the top of the file.
const { groupFieldState, dragSessionState, fieldDefs } = vi.hoisted(() => ({
  groupFieldState: { current: undefined as string | undefined },
  // Held as a getter-style ref because the `useDragSession()` mock
  // returns a fresh value on each call — flipping `current` between
  // renders flips the drag state the next time the component reads it.
  dragSessionState: { current: null as unknown },
  fieldDefs: [
    {
      id: "tag",
      name: "tag",
      type: { kind: "string" },
    },
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

// Drag-session stub. The real `useDragSession()` reads the Tauri event
// stream — wiring that up in the test environment is heavier than we
// need. We re-expose the same shape (session + no-op control methods)
// and let `dragSessionState.current` drive the value the hook returns.
// Production paths inside `<GroupedBoardView>` only read `.session`
// today, but the methods are present so a future caller-side change
// surfaces here as a missing-property type error rather than a silent
// runtime miss.
vi.mock("@/lib/drag-session-context", async () => {
  const actual = await vi.importActual<
    typeof import("@/lib/drag-session-context")
  >("@/lib/drag-session-context");
  return {
    ...actual,
    useDragSession: () => ({
      session: dragSessionState.current,
      startSession: async () => {},
      startFileSession: async () => {},
      cancelSession: async () => {},
      completeSession: async () => {},
      completeFileSession: async () => {},
      isSource: false,
    }),
  };
});

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

/** Number of distinct group buckets in the high-cardinality fixture. */
const GROUP_COUNT = 200;
/** Tasks per group — 10 keeps the total under 2300 while exercising many groups. */
const TASKS_PER_GROUP = 10;
/** Total task count — 200 * 10 = 2000. */
const TASK_COUNT = GROUP_COUNT * TASKS_PER_GROUP;

/**
 * Upper bound on regroup wall-clock time inside the test environment.
 *
 * Matches the existing card-virtualization perf test's budget — the
 * test runtime is much slower than production (act() overhead, dev-mode
 * React, no JIT) so the test budget is a multiple of the user-facing
 * 200ms target. The shape this pins is "regroup time is bounded by a
 * constant regardless of group count" — at 200 unbounded sections the
 * broken path mounts 200 full BoardView trees and takes well past 8s.
 */
const REGROUP_BUDGET_MS = 1000;

/**
 * Upper bound on the number of `<GroupSection>` roots that may be
 * mounted simultaneously.
 *
 * Sections start collapsed by default so the steady-state estimate is
 * `COLLAPSED_HEIGHT_PX` (~40px) per row. With a ~600px outer viewport
 * that fits ~15 rows, plus 2 overscan top + 2 bottom ≈ 19 mounted in
 * the steady state. Allow headroom (<30) to absorb the virtualizer's
 * transient extra-row measurement pass and any small layout drift.
 *
 * The bound is still well below the 200-group fixture so the test
 * pins virtualization meaningfully — a broken outer virtualizer mounts
 * all 200 sections, far over the threshold.
 */
const MOUNTED_SECTION_LIMIT = 30;

/** Per-section/per-column viewport height (px) the shim returns. */
const VIEWPORT_PX = 600;

function makeColumn(id: string, name: string, order: number): Entity {
  return {
    id,
    entity_type: "column",
    moniker: `column:${id}`,
    fields: { name, order },
  };
}

/**
 * Build a 2000-task fixture distributed across 200 `tag-i` groups.
 *
 * `bucket.value = "tag-${i}"` is the high-cardinality shape the task
 * description specifies — tags-like cardinality but with the simpler
 * string-field plumbing every test path already understands.
 */
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
        tag: `tag-${i % GROUP_COUNT}`,
        // `project` shares the same `tag-N` value space on purpose: the
        // groupField-change regression test relies on collision-by-value
        // between the two fields to verify hoisted collapsed-set state
        // does NOT bleed across `groupField` changes. With matching
        // values, a buggy implementation that keeps the old collapsed
        // set after `tag` → `project` would render the project bucket
        // pre-collapsed.
        project: `tag-${i % GROUP_COUNT}`,
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

/** Provider stack required by `<BoardView>` descendants. */
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
// Test environment shims — see grouped-board-view.perf.test.tsx for the
// full rationale. The shim shape here is identical except an additional
// selector matches the outer grouped-board-view scroll container so the
// *outer* virtualizer (the one this task introduces) also reports a
// bounded viewport.
// ---------------------------------------------------------------------------

interface OriginalDescriptors {
  offsetHeight: PropertyDescriptor;
  clientHeight: PropertyDescriptor;
  ResizeObserver: typeof ResizeObserver;
}

/**
 * Whether an element should report a synthetic bounded viewport.
 *
 * - `data-testid='group-section-body'` — the per-section bounded
 *   container inside `<GroupSection>`.
 * - `[data-group-list]` — the outer group list scroll container the
 *   outer virtualizer measures.
 * - Anything with `overflow-y-auto` in its class — column scroll
 *   containers + the outer grouped-board-view root, both of which need
 *   bounded measurements for their respective virtualizers.
 */
function isStubbedViewport(el: Element): boolean {
  if (el instanceof HTMLElement) {
    if (el.dataset.testid === "group-section-body") return true;
    if (el.dataset.groupList !== undefined) return true;
  }
  const cls = el.className;
  if (typeof cls !== "string") return false;
  return cls.includes("overflow-y-auto") || cls.includes("overflow-auto");
}

/**
 * Install global overrides so `@tanstack/react-virtual` reports bounded
 * viewports for any element matching `isStubbedViewport`.
 *
 * Covers both measurement paths the virtualizer uses: synchronous
 * `offsetHeight`/`clientHeight` reads, and asynchronous
 * `ResizeObserver.borderBoxSize` callbacks. See the perf test for the
 * full mechanism — the wrapper here is a direct copy.
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("GroupedBoardView outer group virtualization", () => {
  let originals: OriginalDescriptors | null = null;
  beforeAll(() => {
    originals = installViewportGetterOverride();
  });
  afterAll(() => {
    if (originals) restoreViewportGetters(originals);
    originals = null;
  });
  beforeEach(() => {
    // Reset the drag session and groupField stubs between tests so one
    // test's mutation doesn't leak into the next.
    dragSessionState.current = null;
    groupFieldState.current = undefined;
  });

  it("mounts only viewport-visible group sections (mounted_group_section_count_is_bounded_by_viewport)", async () => {
    const tasks = makeFixtureTasks();
    groupFieldState.current = "tag";

    let result!: ReturnType<typeof render>;
    await act(async () => {
      result = render(
        wrap(<GroupedBoardView board={FIXTURE_BOARD} tasks={tasks} />),
      );
    });

    const mountedSections = result.container.querySelectorAll(
      "[data-group-section]",
    ).length;

    // eslint-disable-next-line no-console
    console.log(
      `[group-virtualization] mountedSections=${mountedSections}/${GROUP_COUNT}`,
    );

    expect(mountedSections).toBeGreaterThan(0);
    expect(mountedSections).toBeLessThan(MOUNTED_SECTION_LIMIT);
  });

  it("regrouping by a 200-group field completes under the regression budget", async () => {
    const tasks = makeFixtureTasks();

    // Initial render in the ungrouped path so the regroup measurement
    // captures the full mount cost of the grouped tree.
    groupFieldState.current = undefined;
    let result!: ReturnType<typeof render>;
    await act(async () => {
      result = render(
        wrap(<GroupedBoardView board={FIXTURE_BOARD} tasks={tasks} />),
      );
    });

    const start = performance.now();
    groupFieldState.current = "tag";
    await act(async () => {
      result.rerender(
        wrap(<GroupedBoardView board={FIXTURE_BOARD} tasks={tasks} />),
      );
    });
    const elapsed = performance.now() - start;

    const mountedSections = result.container.querySelectorAll(
      "[data-group-section]",
    ).length;
    const mountedCards =
      result.container.querySelectorAll("[data-entity-card]").length;

    // eslint-disable-next-line no-console
    console.log(
      `[group-virtualization] regroup elapsed=${elapsed.toFixed(1)}ms ` +
        `(budget ${REGROUP_BUDGET_MS}ms) mountedSections=${mountedSections}/${GROUP_COUNT} ` +
        `mountedCards=${mountedCards}/${TASK_COUNT}`,
    );

    expect(mountedSections).toBeGreaterThan(0);
    expect(mountedSections).toBeLessThan(MOUNTED_SECTION_LIMIT);
    expect(mountedCards).toBeLessThan(TASK_COUNT / 2);
    expect(elapsed).toBeLessThan(REGROUP_BUDGET_MS);
  });

  it("preserves expand state across outer scroll recycling (collapse_state_survives_outer_scroll_recycling)", async () => {
    // Sections default to collapsed (see <GroupedBoardBody>'s lazy
    // useState initializer). We exercise the hoisted-state contract in
    // the opposite polarity: expand a section, scroll it out of the
    // recycle window, scroll back, and assert it is STILL expanded. If
    // collapse state lived in `<GroupSection>`'s own `useState`, the
    // recycled remount would reset to the default-collapsed render and
    // the body would disappear.
    const tasks = makeFixtureTasks();
    groupFieldState.current = "tag";

    let result!: ReturnType<typeof render>;
    await act(async () => {
      result = render(
        wrap(<GroupedBoardView board={FIXTURE_BOARD} tasks={tasks} />),
      );
    });

    const { container } = result;

    const firstSection = container.querySelector(
      "[data-group-section]",
    ) as HTMLElement;
    expect(firstSection).toBeTruthy();
    const firstSectionLabel = firstSection.getAttribute("data-group-value");
    expect(firstSectionLabel).toBeTruthy();

    // Sanity: section starts collapsed.
    expect(
      firstSection.querySelector("[data-testid='group-section-body']"),
    ).toBeNull();

    // Click to expand.
    const toggleButton = firstSection.querySelector("button");
    expect(toggleButton).toBeTruthy();
    await act(async () => {
      fireEvent.click(toggleButton!);
    });
    expect(
      firstSection.querySelector("[data-testid='group-section-body']"),
    ).not.toBeNull();

    // Scroll far enough that the virtualizer recycles the first section
    // out of view, then scroll back.
    const outerScroll = container.querySelector(
      "[data-group-list]",
    ) as HTMLElement;
    expect(outerScroll).toBeTruthy();
    await act(async () => {
      outerScroll.scrollTop = 50_000;
      outerScroll.dispatchEvent(new Event("scroll"));
    });
    await act(async () => {
      outerScroll.scrollTop = 0;
      outerScroll.dispatchEvent(new Event("scroll"));
    });

    // Re-locate the (now-re-mounted) first section by its
    // data-group-value. It MUST still be expanded because expand/collapse
    // state is hoisted to the parent, not held inside the recycled
    // `<GroupSection>`'s `useState`.
    const refoundSection = container.querySelector(
      `[data-group-section][data-group-value="${firstSectionLabel}"]`,
    ) as HTMLElement | null;
    expect(refoundSection).toBeTruthy();
    expect(
      refoundSection!.querySelector("[data-testid='group-section-body']"),
    ).not.toBeNull();
  });

  it("outer scroll container uses overflow-y-auto (outer_scroll_container_uses_overflow_y_auto)", async () => {
    const tasks = makeFixtureTasks();
    groupFieldState.current = "tag";

    let result!: ReturnType<typeof render>;
    await act(async () => {
      result = render(
        wrap(<GroupedBoardView board={FIXTURE_BOARD} tasks={tasks} />),
      );
    });

    // The outer scroll container must keep `overflow-y-auto` so the
    // existing perf-test's shim selector continues to apply. The
    // production component carries `flex flex-col flex-1 min-h-0
    // overflow-y-auto` on its root.
    const outer = result.container.querySelector(
      "[data-group-list]",
    ) as HTMLElement | null;
    expect(outer).toBeTruthy();
    expect(outer!.className).toContain("overflow-y-auto");
  });

  it("does not carry expand state across groupField changes (collapse_state_does_not_bleed_across_group_field)", async () => {
    // Regression for the review finding that `<GroupedBoardBody>`'s
    // hoisted state would survive `groupField` flips when reconciled by
    // position. The fix wraps the body in `key={groupField}` so React
    // remounts it on every field change. This test pins that contract
    // in the default-collapsed polarity:
    //
    //   - Under `tag`, expand a bucket (changes the user-set state away
    //     from the default).
    //   - Switch to `project`. The project field shares the `tag-N` value
    //     space on purpose so a buggy implementation that kept the state
    //     would render the matching project bucket already expanded.
    //   - Assert no project bucket renders expanded — the lazy
    //     initializer has run fresh on the remount and seeded the
    //     project field's set with every project bucket collapsed.
    const tasks = makeFixtureTasks();

    groupFieldState.current = "tag";
    let result!: ReturnType<typeof render>;
    await act(async () => {
      result = render(
        wrap(<GroupedBoardView board={FIXTURE_BOARD} tasks={tasks} />),
      );
    });

    const { container } = result;

    // Expand the first bucket under `tag` grouping.
    const firstSection = container.querySelector(
      "[data-group-section]",
    ) as HTMLElement;
    expect(firstSection).toBeTruthy();
    const expandedValue = firstSection.getAttribute("data-group-value");
    expect(expandedValue).toBeTruthy();

    await act(async () => {
      fireEvent.click(firstSection.querySelector("button")!);
    });

    // Sanity: the bucket really is expanded under `tag`.
    expect(
      firstSection.querySelector("[data-testid='group-section-body']"),
    ).not.toBeNull();

    // Flip groupField to `project`. Same value space — a buggy
    // implementation that kept the prior `Set<string>` would render the
    // matching project bucket expanded.
    groupFieldState.current = "project";
    await act(async () => {
      result.rerender(
        wrap(<GroupedBoardView board={FIXTURE_BOARD} tasks={tasks} />),
      );
    });

    // Every mounted project section must render collapsed — the prior
    // field's expansion may not transfer.
    const projectSections = container.querySelectorAll(
      "[data-group-section]",
    );
    expect(projectSections.length).toBeGreaterThan(0);
    for (const section of projectSections) {
      const body = section.querySelector("[data-testid='group-section-body']");
      const value = section.getAttribute("data-group-value");
      expect(body, `project bucket "${value}" rendered expanded`).toBeNull();
    }
  });

  it("mounts every section while a drag session is active (drag_suspends_outer_virtualization)", async () => {
    // Regression for the review finding that dnd-kit's per-element
    // registrations die when a `<GroupSection>` unmounts mid-drag.
    // The fix bypasses the outer virtualizer while
    // `useDragSession().session !== null`, mounting every section so
    // sources and targets stay registered for the whole drag.
    const tasks = makeFixtureTasks();
    groupFieldState.current = "tag";

    let result!: ReturnType<typeof render>;
    await act(async () => {
      result = render(
        wrap(<GroupedBoardView board={FIXTURE_BOARD} tasks={tasks} />),
      );
    });

    // Sanity: virtualizer is windowing the section count before a drag
    // begins.
    const beforeDragMounted = result.container.querySelectorAll(
      "[data-group-section]",
    ).length;
    expect(beforeDragMounted).toBeLessThan(MOUNTED_SECTION_LIMIT);

    // Begin a drag. Any non-null shape suffices for the production
    // code's check (`session !== null`); we use the minimum bag of
    // fields the production type requires.
    dragSessionState.current = {
      session_id: "test-drag",
      source_board_path: "/test/board",
      source_window_label: "main",
      task_id: tasks[0]!.id,
      task_fields: {},
      copy_mode: false,
      from: {
        kind: "focus_chain",
        entity_type: "task",
        entity_id: tasks[0]!.id,
        fields: {},
        source_board_path: "/test/board",
        source_window_label: "main",
      },
    };

    // Force a re-render so `useDragSession()` reads the new value.
    // (`groupFieldState` is unchanged but `rerender` flushes the
    // component tree which is enough to make `useContext` pick up the
    // mock's new return value.)
    await act(async () => {
      result.rerender(
        wrap(<GroupedBoardView board={FIXTURE_BOARD} tasks={tasks} />),
      );
    });

    const duringDragMounted = result.container.querySelectorAll(
      "[data-group-section]",
    ).length;
    expect(duringDragMounted).toBe(GROUP_COUNT);

    // And the drag-bypass marker is present so future tests / dev
    // tools can tell which path is mounted.
    const outer = result.container.querySelector(
      "[data-group-list][data-drag-bypass='true']",
    );
    expect(outer).toBeTruthy();
  });
});
