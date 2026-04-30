/**
 * DataTable virtualization tests.
 *
 * Verifies that the body of `DataTable` is row-virtualized via
 * `@tanstack/react-virtual`: only the rows inside the scroll viewport
 * (plus a small overscan) are mounted, and cursor navigation past the
 * visible window scrolls the cursor row into view by translating the
 * data-row index into the corresponding flat-row index that the
 * virtualizer indexes into.
 *
 * Tests run in real Chromium via `@vitest/browser-playwright`, but the
 * vitest browser project does not bundle Tailwind, so utility classes
 * like `flex-1 overflow-auto min-h-0` produce no CSS rules. The
 * `<DataTable>` outer scroll container therefore renders at its
 * natural content height (~25k px for 1000 rows), so the virtualizer
 * sees the entire list as "visible" and renders every row.
 *
 * To force a finite, deterministic scroll viewport we override the
 * scroll container's `clientHeight` and `getBoundingClientRect` with
 * `Object.defineProperty` after mount. This mirrors the mock pattern
 * the task description prescribes, and matches `data-table.test.tsx`
 * setup hygiene.
 */

import { describe, it, expect, vi } from "vitest";
import { render, waitFor } from "@testing-library/react";

// ---------------------------------------------------------------------------
// jsdom / browser-mode shims (kept in sync with data-table.test.tsx).
// ---------------------------------------------------------------------------
Element.prototype.scrollIntoView = vi.fn();

// ---------------------------------------------------------------------------
// Mocks -- before component imports.
// ---------------------------------------------------------------------------

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(() => Promise.resolve([])),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ label: "main" }),
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
// Imports -- after mocks.
// ---------------------------------------------------------------------------

import { DataTable, type DataTableColumn } from "./data-table";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { EntityStoreProvider } from "@/lib/entity-store-context";
import { SchemaProvider } from "@/lib/schema-context";
import { TooltipProvider } from "@/components/ui/tooltip";
import { FocusLayer } from "./focus-layer";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { asSegment } from "@/types/spatial";
import "@/components/fields/registrations";
import type { Entity, FieldDef } from "@/types/kanban";
import type { UseGridReturn } from "@/hooks/use-grid";

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

const FIELD_DEFS: FieldDef[] = [
  {
    id: "f1",
    name: "title",
    type: { kind: "text" },
    section: "header",
    display: "text",
    editor: "text",
  },
  {
    id: "f2",
    name: "status",
    type: { kind: "text" },
    section: "header",
    display: "text",
    editor: "text",
  },
];

const COLUMNS: DataTableColumn[] = FIELD_DEFS.map((f) => ({ field: f }));

/** Synthesize N row entities for virtualization stress tests. */
function makeEntities(count: number): Entity[] {
  const out: Entity[] = [];
  for (let i = 0; i < count; i++) {
    out.push({
      entity_type: "task",
      id: `t${i}`,
      moniker: `task:t${i}`,
      fields: { title: `Task ${i}`, status: i % 2 === 0 ? "todo" : "done" },
    });
  }
  return out;
}

/** Minimal grid state -- no editing, cursor at the supplied position. */
function makeGrid(cursor = { row: 0, col: 0 }): UseGridReturn {
  return {
    cursor,
    mode: "normal" as const,
    enterEdit: vi.fn(),
    exitEdit: vi.fn(),
    enterVisual: vi.fn(),
    exitVisual: vi.fn(),
    selection: null,
    setCursor: vi.fn(),
    expandSelection: vi.fn(),
    getSelectedRange: () => null,
  };
}

/** Fixed viewport height used for all virtualization tests. */
const VIEWPORT_HEIGHT = 400;

/**
 * Force the scroll container to a fixed pixel height so the virtualizer
 * has a finite, deterministic viewport.
 *
 * Without this stub the scroll container's `flex-1 overflow-auto
 * min-h-0` Tailwind classes do nothing in the test environment
 * (Tailwind is not bundled into the vitest browser project) and the
 * container renders at its natural content height -- which makes the
 * virtualizer treat the full row list as "visible" and the test
 * becomes meaningless.
 *
 * `@tanstack/react-virtual` reads viewport size via
 * `element.offsetHeight` (initial measurement) and via
 * `ResizeObserver.borderBoxSize` (subsequent updates). Setting inline
 * `height` + `overflow: auto` styles is the only thing both code paths
 * agree on -- a `clientHeight` defineProperty stub would be ignored by
 * the live ResizeObserver.
 */
function stubScrollViewport(
  container: HTMLElement,
  height = VIEWPORT_HEIGHT,
): void {
  const scrollEl = container.querySelector(
    "div.flex-1.overflow-auto",
  ) as HTMLDivElement | null;
  if (!scrollEl) throw new Error("scroll container not found");
  scrollEl.style.height = `${height}px`;
  scrollEl.style.maxHeight = `${height}px`;
  scrollEl.style.overflow = "auto";
}

/**
 * Render `<DataTable>` inside a `<EntityFocusProvider>`.
 *
 * The caller is expected to invoke `stubScrollViewport(container)`
 * after rendering to give the virtualizer a finite viewport to work
 * with -- the stubbing has to happen after mount because the scroll
 * container is created by `<DataTable>`, not by the test wrapper.
 */
function renderTable(
  props: Partial<React.ComponentProps<typeof DataTable>> = {},
) {
  return render(
    <SpatialFocusProvider>
      <FocusLayer name={asSegment("window")}>
        <EntityFocusProvider>
          <DataTable
            columns={COLUMNS}
            rows={props.rows ?? []}
            grid={props.grid ?? makeGrid()}
            showRowSelector={true}
            {...props}
          />
        </EntityFocusProvider>
      </FocusLayer>
    </SpatialFocusProvider>,
  );
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("DataTable row virtualization", () => {
  it("mounts only the visible window of rows for a 1000-row grid", async () => {
    const entities = makeEntities(1000);
    const { container } = renderTable({ rows: entities });
    stubScrollViewport(container);

    // Wait for the post-mount ResizeObserver tick to re-measure the
    // (now-stubbed) viewport and the virtualizer to re-render with
    // the correct visible window.
    await waitFor(
      () => {
        const tbody = container.querySelector("tbody")!;
        const dataRows = tbody.querySelectorAll("tr[data-segment]");
        // 400px viewport / 36px per row = ~11 visible + overscan 5
        // each side -- well under 100.
        expect(dataRows.length).toBeGreaterThan(0);
        expect(dataRows.length).toBeLessThan(100);
      },
      { timeout: 2000 },
    );

    const tbody = container.querySelector("tbody")!;
    const dataRows = tbody.querySelectorAll("tr[data-segment]");
    // Hard upper bound proves we are virtualized.
    expect(dataRows.length).toBeLessThanOrEqual(50);
  });

  it("scrolls the cursor row into view when cursor moves past the visible window", async () => {
    const entities = makeEntities(1000);
    const grid0 = makeGrid({ row: 0, col: 0 });

    const { container, rerender } = renderTable({
      rows: entities,
      grid: grid0,
    });
    stubScrollViewport(container);

    const scrollEl = container.querySelector(
      "div.flex-1.overflow-auto",
    ) as HTMLDivElement;
    expect(scrollEl).not.toBeNull();

    // Wait for the initial virtualizer mount + measurement to settle.
    await waitFor(
      () => {
        const dataRows = container.querySelectorAll("tbody tr[data-segment]");
        expect(dataRows.length).toBeLessThan(100);
      },
      { timeout: 2000 },
    );

    // Initial scroll position: at or very near the top (sub-pixel
    // adjustments from `scrollToIndex(0, { align: "auto" })` may
    // leave a few pixels of offset, but it must be far below the
    // post-scroll target).
    const initialScrollTop = scrollEl.scrollTop;
    expect(initialScrollTop).toBeLessThan(50);

    // Move cursor to row 500. The virtualization layer must call
    // `virtualizer.scrollToIndex(500)` (translated through the
    // dataRow -> flatRow inverse map; with no grouping the two
    // indices coincide) which mutates the scroll container's
    // `scrollTop`. We assert the side-effect: the scroll position
    // is significantly past the initial viewport.
    const grid500 = makeGrid({ row: 500, col: 0 });
    rerender(
      <EntityFocusProvider>
        <DataTable
          columns={COLUMNS}
          rows={entities}
          grid={grid500}
          showRowSelector={true}
        />
      </EntityFocusProvider>,
    );

    await waitFor(
      () => {
        expect(scrollEl.scrollTop).toBeGreaterThan(initialScrollTop + 1000);
      },
      { timeout: 2000 },
    );
  });

  it("compact-mode rows render at the same structural height regardless of population", async () => {
    // The virtualizer reserves a fixed `ROW_HEIGHT` per row. If a populated
    // cell renders taller than its empty counterpart, rows visibly jitter
    // and the absolute scroll position drifts from the rendered positions.
    //
    // Tailwind utilities are not bundled into the vitest browser project,
    // so we cannot assert on `getBoundingClientRect().height`. Instead we
    // assert the structural invariant that drives the visible-height
    // contract: every compact-mode display output is wrapped in a
    // `data-compact-cell="true"` element with the same className. The
    // wrapper's `h-6 flex items-center` Tailwind classes are what
    // produce the uniform pixel height when CSS is loaded; matching
    // wrapper class names is the test-friendly proxy.

    // Build an actor pool the populated rows reference, plus two field
    // defs (`assignees` and `tags`) prone to height divergence. Each
    // exercises a different empty-state offender:
    // - `assignees` (avatar display) — populated renders <Avatar>, empty
    //   renders a placeholder/dash span.
    // - `tags` (badge-list display) — populated renders CM6 mention pills,
    //   empty renders a placeholder/dash span.
    const actors: Entity[] = [
      {
        entity_type: "actor",
        id: "alice",
        moniker: "actor:alice",
        fields: { name: "Alice Smith" },
      },
    ];
    const heightFields: FieldDef[] = [
      {
        id: "f-assignees",
        name: "assignees",
        type: { kind: "reference", entity: "actor", multiple: true },
        section: "body",
        display: "avatar",
        editor: "multi-select",
        placeholder: "Assign",
      } as unknown as FieldDef,
      {
        id: "f-tags",
        name: "tags",
        type: {
          kind: "computed",
          derive: "parse-body-tags",
          entity: "tag",
          commit_display_names: true,
        },
        section: "header",
        display: "badge-list",
        editor: "multi-select",
        placeholder: "Add tags",
      } as unknown as FieldDef,
    ];
    const heightColumns: DataTableColumn[] = heightFields.map((f) => ({
      field: f,
    }));

    function renderHeightTable(rows: Entity[]) {
      return render(
        <TooltipProvider>
          <SchemaProvider>
            <EntityStoreProvider entities={{ actor: actors, task: rows }}>
              <EntityFocusProvider>
                <DataTable
                  columns={heightColumns}
                  rows={rows}
                  grid={makeGrid()}
                  showRowSelector={true}
                />
              </EntityFocusProvider>
            </EntityStoreProvider>
          </SchemaProvider>
        </TooltipProvider>,
      );
    }

    // All-populated grid: every row carries an assignee + a tag.
    const populatedRows: Entity[] = Array.from({ length: 8 }, (_, i) => ({
      entity_type: "task",
      id: `pt${i}`,
      moniker: `task:pt${i}`,
      fields: { assignees: ["alice"], tags: ["bugfix"] },
    }));
    const populated = renderHeightTable(populatedRows);
    let populatedClassNames: string[] = [];
    await waitFor(
      () => {
        const wrappers = populated.container.querySelectorAll(
          "tbody tr[data-segment] [data-compact-cell='true']",
        );
        expect(wrappers.length).toBe(
          populatedRows.length * heightFields.length,
        );
        populatedClassNames = Array.from(wrappers).map((w) => w.className);
      },
      { timeout: 2000 },
    );
    populated.unmount();

    // All-empty grid: same shape, no assignees, no tags.
    const emptyRows: Entity[] = Array.from({ length: 8 }, (_, i) => ({
      entity_type: "task",
      id: `et${i}`,
      moniker: `task:et${i}`,
      fields: { assignees: [], tags: [] },
    }));
    const empty = renderHeightTable(emptyRows);
    let emptyClassNames: string[] = [];
    await waitFor(
      () => {
        const wrappers = empty.container.querySelectorAll(
          "tbody tr[data-segment] [data-compact-cell='true']",
        );
        expect(wrappers.length).toBe(emptyRows.length * heightFields.length);
        emptyClassNames = Array.from(wrappers).map((w) => w.className);
      },
      { timeout: 2000 },
    );
    empty.unmount();

    // Populated and empty wrappers must share identical class names —
    // that's what guarantees the same rendered height once Tailwind
    // applies the encoded `h-6 flex items-center` rules.
    expect(emptyClassNames).toEqual(populatedClassNames);
  });

  it("reserves padding-spacer height proportional to total row count", async () => {
    // Render a small grid first so we can compare against a large grid
    // and assert that scrollHeight scales linearly with row count.
    // This proves the padding-row pattern reserves vertical space for
    // unmounted rows -- if it didn't, scrollHeight would only reflect
    // the rendered rows and the user-visible scrollbar would be wrong.
    const small = renderTable({ rows: makeEntities(50) });
    stubScrollViewport(small.container);
    const smallTable = small.container.querySelector("table")!;
    let smallHeight = 0;
    await waitFor(
      () => {
        smallHeight = smallTable.scrollHeight;
        expect(smallHeight).toBeGreaterThan(0);
      },
      { timeout: 2000 },
    );
    small.unmount();

    const big = renderTable({ rows: makeEntities(2000) });
    stubScrollViewport(big.container);
    const bigTable = big.container.querySelector("table")!;
    await waitFor(
      () => {
        // Big grid (2000 rows) should reserve roughly 40x the height of
        // the small grid (50 rows). We use a loose lower bound (10x)
        // that distinguishes "padding reserved" from "only visible rows
        // contribute" -- without padding spacers, scrollHeight would be
        // bounded by the viewport (~400px + overscan) regardless of
        // row count.
        expect(bigTable.scrollHeight).toBeGreaterThanOrEqual(smallHeight * 10);
      },
      { timeout: 2000 },
    );
    big.unmount();
  });
});
