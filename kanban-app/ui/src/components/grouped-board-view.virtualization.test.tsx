/**
 * Regression: `<GroupedBoardView>` must virtualize cards per group.
 *
 * Acceptance for kanban task `01KREWAXSXWY95SJCZTD03J0AJ` — switching the
 * perspective group field on a 2300-task board used to take ~3 minutes
 * because every `<GroupSection>` mounted its inner `<BoardView>` against
 * an unbounded scroll region. The column-level `useVirtualizer` then saw
 * its scroll element's `clientHeight` collapse to the natural height of
 * the full task list and concluded "the viewport already shows
 * everything" — mounting all N cards.
 *
 * The fix gives each expanded `<GroupSection>` body a bounded
 * viewport-relative height so the column's scroll element has a finite
 * ancestor the virtualizer can window against. This file pins that
 * contract: a 2300-task fixture distributed across 5 groups must mount
 * far fewer DOM `[data-entity-card]` nodes than the dataset.
 *
 * Test environment notes:
 *
 * - Runs under the vitest browser project (real Chromium via Playwright);
 *   `useVirtualizer`'s ResizeObserver fires for real and reads real
 *   element heights via `offsetHeight`.
 * - Tailwind utilities are not bundled into the browser project, so the
 *   production `h-[70vh]` class produces no CSS rule. We stub a finite
 *   height inline on every `data-testid="group-section-body"` element
 *   AND on every column scroll container (`[class*='overflow-y-auto']`)
 *   after mount — the same pattern `data-table.virtualized.test.tsx` and
 *   `column-view.test.tsx` document.
 */

import { describe, it, expect, vi } from "vitest";
import { act, fireEvent, waitFor } from "@testing-library/react";
import { renderInAct } from "@/test/act-render";

/**
 * Click every group-section header to expand it.
 *
 * `<GroupedBoardView>` starts every bucket collapsed by default (see
 * the production component's file header for the jumpiness rationale).
 * These tests check the inner card virtualizer + the section body's
 * bounded-height CSS contract — both of which only exist when a
 * section is expanded. Expand them all before asserting.
 */
async function expandAll(container: HTMLElement): Promise<void> {
  const headers = container.querySelectorAll("[data-group-section] button");
  await act(async () => {
    for (const btn of headers) {
      fireEvent.click(btn);
    }
  });
}

// ---------------------------------------------------------------------------
// Tauri API mocks — must come before component imports.
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

// `useActivePerspective` is used by both `<GroupedBoardView>` (to read the
// `groupField`) and by inner `<BoardView>` instances (to compute task ordering).
// Both call sites get the same stub. `vi.hoisted` keeps the field-defs
// constant visible to both the perspective mock and the schema mock factories
// — which are themselves hoisted above the module body.
const { fieldDefs } = vi.hoisted(() => ({
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
    groupField: "project",
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

/** Number of groups the perf fixture distributes tasks across. */
const GROUP_COUNT = 5;
/** Total task count — matches the production scenario from the task report. */
const TASK_COUNT = 2300;
/** Per-group viewport pixel height stubbed into the section body in tests. */
const SECTION_VIEWPORT_HEIGHT = 600;

function makeColumn(id: string, name: string, order: number): Entity {
  return {
    id,
    entity_type: "column",
    moniker: `column:${id}`,
    fields: { name, order },
  };
}

/**
 * Build a 2300-task fixture distributed across 5 `project` group values.
 *
 * Each task carries a `position_column` (one of 4 columns) and a `project`
 * group label. Distribution is round-robin across groups so every group
 * gets ~460 tasks — well above `VIRTUALIZE_THRESHOLD = 25` so each
 * column inside each group activates the virtualizer.
 */
function makeFixtureTasks(): Entity[] {
  const tasks: Entity[] = [];
  const columns = ["todo", "doing", "review", "done"];
  for (let i = 0; i < TASK_COUNT; i++) {
    const groupIdx = i % GROUP_COUNT;
    const colIdx = i % columns.length;
    tasks.push({
      id: `t${i}`,
      entity_type: "task",
      moniker: `task:t${i}`,
      fields: {
        title: `Task ${i}`,
        position_column: columns[colIdx],
        position_ordinal: `a${String(i).padStart(5, "0")}`,
        project: `group-${groupIdx}`,
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

/**
 * Wrap `<GroupedBoardView>` in the full provider stack the real BoardView
 * descendants require.
 *
 * Mirrors the wrapper used by `board-view.test.tsx` — anything less and the
 * inner `<BoardSpatialBody>` throws from `useFullyQualifiedMoniker()`.
 */
async function renderGroupedBoard(board: BoardData, tasks: Entity[]) {
  return await renderInAct(
    <SpatialFocusProvider>
      <FocusLayer name={asSegment("window")}>
        <EntityFocusProvider>
          <EntityStoreProvider entities={{}}>
            <TooltipProvider>
              <ActiveBoardPathProvider value="/test/board">
                <DragSessionProvider>
                  <GroupedBoardView board={board} tasks={tasks} />
                </DragSessionProvider>
              </ActiveBoardPathProvider>
            </TooltipProvider>
          </EntityStoreProvider>
        </EntityFocusProvider>
      </FocusLayer>
    </SpatialFocusProvider>,
  );
}

/**
 * Stub each group section body to a fixed viewport height AND each column
 * scroll container to that same height.
 *
 * Tailwind utilities (`h-[70vh]`, `flex-1`, `overflow-y-auto`, `min-h-0`)
 * are not bundled into the vitest browser project, so the production
 * classes produce no CSS rule. Without these inline stubs the
 * `<ColumnView>` virtualizer would observe an unbounded scroll element
 * and mount every card — masking the regression we are guarding against.
 *
 * `@tanstack/react-virtual` reads viewport size via `element.offsetHeight`
 * (initial measurement) and via `ResizeObserver.borderBoxSize` (subsequent
 * updates). Setting inline `height` + `overflow: auto` satisfies both
 * paths — the same canonical pattern `data-table.virtualized.test.tsx`
 * documents.
 */
function stubViewportHeights(container: HTMLElement): void {
  const sections = container.querySelectorAll<HTMLDivElement>(
    "[data-testid='group-section-body']",
  );
  for (const section of sections) {
    section.style.height = `${SECTION_VIEWPORT_HEIGHT}px`;
    section.style.maxHeight = `${SECTION_VIEWPORT_HEIGHT}px`;
    section.style.overflow = "hidden";
  }
  const scrollEls = container.querySelectorAll<HTMLDivElement>(
    "[class*='overflow-y-auto']",
  );
  for (const el of scrollEls) {
    el.style.height = `${SECTION_VIEWPORT_HEIGHT}px`;
    el.style.maxHeight = `${SECTION_VIEWPORT_HEIGHT}px`;
    el.style.overflow = "auto";
  }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("GroupedBoardView virtualization", () => {
  // Default 15s timeout is tight when this test runs as part of the full
  // suite: expanding 5 sections each holding ~460 tasks fires 20 inner
  // column virtualizers measuring + mounting in parallel, and the
  // ResizeObserver-driven calibration burns wall time. The test passes
  // in isolation at ~5s but flaked at the boundary under load — 45s
  // keeps it stable without changing what's being measured.
  it("mounts only viewport-bounded card windows across all sections (NOT every task)", async () => {
    const tasks = makeFixtureTasks();
    const { container } = await renderGroupedBoard(FIXTURE_BOARD, tasks);

    // Sections start collapsed by default — expand them so the inner
    // card virtualizer fires.
    await expandAll(container);

    // First confirm the grouped layout actually rendered the expected
    // number of section bodies — if grouping silently fell back to the
    // ungrouped path, the rest of the assertion would be vacuous.
    const sectionBodies = container.querySelectorAll(
      "[data-testid='group-section-body']",
    );
    expect(sectionBodies.length).toBe(GROUP_COUNT);

    stubViewportHeights(container);

    // Bound proof. Each section's columns are independently virtualized;
    // even in the worst case where every column in every section mounts
    // its full viewport-visible window (plus 5-card overscan on each
    // side, plus a trailing drop zone), the DOM card count is bounded
    // by `GROUP_COUNT * columns_per_group * (visible + 2*overscan + 1)`.
    //
    // With 600px section height, 80px estimated row height, 5-card
    // overscan, 4 columns, 5 groups: ceil(600/80) = 8 visible per
    // column + 10 overscan + 1 trailing zone = 19 cards per column. So
    // 5 * 4 * 19 = 380 cards is the theoretical upper bound. Allow
    // generous headroom (1000) — the assertion is "far less than 2300",
    // not "exactly some predicted constant", because the virtualizer's
    // dynamic size-measurement may temporarily mount extra rows during
    // the initial calibration pass.
    await waitFor(
      () => {
        const cards = container.querySelectorAll("[data-entity-card]");
        expect(cards.length).toBeGreaterThan(0);
        expect(cards.length).toBeLessThan(1000);
      },
      { timeout: 5000 },
    );

    // Hard absolute upper bound — well below TASK_COUNT.
    const cards = container.querySelectorAll("[data-entity-card]");
    expect(cards.length).toBeLessThan(TASK_COUNT / 2);
  }, 45_000);

  it("each group section body has a bounded height class so the inner virtualizer can window", async () => {
    // Pin the structural contract: expanded section bodies carry a
    // viewport-relative height class. Without a definite-height
    // ancestor, the column's `flex-1 overflow-y-auto` scroll container
    // would expand to its content height and the virtualizer would
    // collapse to mounting every card. This regression test fires when
    // the height class is silently removed by a future refactor — the
    // matching virtualization test above only fires when both the
    // height class AND the inline test-stub coincide, so this one
    // guards the production CSS contract independently.
    const tasks = makeFixtureTasks().slice(0, 50); // small enough to render quickly
    const { container } = await renderGroupedBoard(FIXTURE_BOARD, tasks);

    // Section bodies only render when expanded.
    await expandAll(container);

    const sectionBodies = container.querySelectorAll<HTMLDivElement>(
      "[data-testid='group-section-body']",
    );
    expect(sectionBodies.length).toBeGreaterThan(0);
    for (const body of sectionBodies) {
      // The bounded-height class is what gives the inner virtualizer a
      // finite ancestor to measure against. Spelling matches the
      // production class set in `group-section.tsx`.
      expect(body.className).toMatch(/h-\[70vh\]/);
      expect(body.className).toContain("min-h-0");
      expect(body.className).toContain("flex");
      expect(body.className).toContain("flex-col");
    }
  });
});
