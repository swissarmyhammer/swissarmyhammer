/**
 * Grid cell click-region contract.
 *
 * Pins the bug fix from the kanban task "Fix grid cell click region smaller
 * than visible cell": in the spatial path, the focusable element inside each
 * grid `<td>` must fill the visible cell rect exactly, so a left-click
 * anywhere inside the visible cell border lands on the focusable (and
 * triggers `spatial_focus`). Previously the `<TableCell>` carried the
 * padding (`px-3 py-1.5`) and the `<FocusScope>`'s rendered `<div>` lived
 * inside that padding — clicks on the cell edge landed on the `<td>` but
 * missed the focusable.
 *
 * The fix moves padding off the `<td>` and onto a wrapper child that lives
 * inside the `<FocusScope>` (`block h-full w-full px-3 py-1.5`). Both the
 * `<FocusScope>`'s rendered `<div>` and the inner click wrapper are sized
 * `block h-full w-full` so their rects equal the `<td>`'s rect. The same
 * shape applies to `RowSelector` — its row-label leaf must also fill its
 * `<td>`.
 *
 * Why class-string assertions rather than rect comparisons:
 *
 *   The vitest browser project does not bundle Tailwind, so utility
 *   classes like `p-0` / `h-full` / `w-full` produce no CSS rules at test
 *   time and a `getBoundingClientRect()` comparison would be skewed by the
 *   browser's UA default `<td>` padding (≈1 px) plus the absence of the
 *   `h-full` rule. The contract this fix establishes is purely in the
 *   className composition — the runtime browser (Tauri WebView) loads
 *   `index.css` and the rules apply normally. The class-string assertions
 *   below pin the implementation contract directly, which is the layer
 *   the regression would re-break.
 *
 * Test cases:
 *
 *   1. body cell `<td>` carries `p-0` — no padding lives on the visible
 *      cell wrapper in the spatial path.
 *   2. body cell focusable (the `<FocusScope>`'s `<div>`) carries
 *      `block h-full w-full` — the spatial-nav-registered rect fills the
 *      `<td>`.
 *   3. body cell inner click wrapper carries `block h-full w-full px-3
 *      py-1.5` — the click region fills the `<td>` and the previously-on-
 *      `<td>` padding now lives inside the focusable.
 *   4. row label `<td>` carries `p-0` — same property for the row
 *      selector cell.
 *   5. row label focusable + inner wrapper carry the fill classes.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, act } from "@testing-library/react";

// ---------------------------------------------------------------------------
// Tauri API mocks — must come before component imports.
// ---------------------------------------------------------------------------

const mockInvoke = vi.hoisted(() =>
  vi.fn(async (_cmd: string, _args?: unknown): Promise<unknown> => undefined),
);
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...a: unknown[]) => mockInvoke(...(a as [string, unknown?])),
}));
vi.mock("@tauri-apps/api/event", () => ({
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

import { DataTable, type DataTableColumn } from "./data-table";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { FocusLayer } from "./focus-layer";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { asSegment } from "@/types/spatial";
import type { Entity, FieldDef } from "@/types/kanban";
import type { UseGridReturn } from "@/hooks/use-grid";

// ---------------------------------------------------------------------------
// Fixtures — keep deliberately small so the table mounts quickly.
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

const ENTITIES: Entity[] = [
  {
    entity_type: "task",
    id: "t1",
    moniker: "task:t1",
    fields: { title: "Task 1", status: "todo" },
  },
  {
    entity_type: "task",
    id: "t2",
    moniker: "task:t2",
    fields: { title: "Task 2", status: "done" },
  },
];

/** Minimal grid state — no editing, cursor at 0,0. */
function makeGrid(): UseGridReturn {
  return {
    cursor: { row: 0, col: 0 },
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

/**
 * Render `<DataTable>` inside the spatial-nav provider stack so the spatial
 * path of `GridCellFocusable` and `RowSelector` is exercised — that's the
 * branch where the click-region bug manifests.
 */
async function renderTable() {
  let result!: ReturnType<typeof render>;
  await act(async () => {
    result = render(
      <SpatialFocusProvider>
        <FocusLayer name={asSegment("window")}>
          <EntityFocusProvider>
            <DataTable
              columns={COLUMNS}
              rows={ENTITIES}
              grid={makeGrid()}
              showRowSelector={true}
            />
          </EntityFocusProvider>
        </FocusLayer>
      </SpatialFocusProvider>,
    );
  });
  return result;
}

/**
 * Resolve the focusable `<div>`, the inner click wrapper, and the
 * enclosing `<td>` for a given segment moniker.
 *
 * `<FocusScope>` mounts a `<div data-segment="...">` (the focusable that
 * the spatial-nav kernel registers) and the call sites in `data-table.tsx`
 * place the click handler on a child `<div>` inside the `<FocusScope>`.
 * The focusable's first element child is the click wrapper.
 */
function findCellShape(
  container: HTMLElement,
  segment: string,
): {
  td: HTMLElement;
  focusable: HTMLElement;
  innerWrapper: HTMLElement;
} {
  const focusable = container.querySelector(
    `[data-segment="${segment}"]`,
  ) as HTMLElement | null;
  if (!focusable) {
    throw new Error(`focusable for ${segment} not found`);
  }
  const td = focusable.closest("td") as HTMLElement | null;
  if (!td) {
    throw new Error(`<td> ancestor for ${segment} not found`);
  }
  const innerWrapper = focusable.firstElementChild as HTMLElement | null;
  if (!innerWrapper) {
    throw new Error(`inner click wrapper for ${segment} not found`);
  }
  return { td, focusable, innerWrapper };
}

/** Assert every class in `expected` is present on `el`'s `class` attribute. */
function expectClasses(el: HTMLElement, expected: string[]): void {
  const classList = (el.getAttribute("class") ?? "").split(/\s+/);
  for (const cls of expected) {
    expect(
      classList,
      `expected class "${cls}" on element ${el.tagName.toLowerCase()}`,
    ).toContain(cls);
  }
}

/** Assert no class in `forbidden` is present on `el`'s `class` attribute. */
function expectNoClasses(el: HTMLElement, forbidden: string[]): void {
  const classList = (el.getAttribute("class") ?? "").split(/\s+/);
  for (const cls of forbidden) {
    expect(
      classList,
      `expected class "${cls}" NOT on element ${el.tagName.toLowerCase()}`,
    ).not.toContain(cls);
  }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("DataTable cell click region — body cells", () => {
  beforeEach(() => {
    mockInvoke.mockClear();
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it("body cell <td> carries `p-0` and no padding utilities", async () => {
    const { container } = await renderTable();
    const { td } = findCellShape(container, "grid_cell:0:title");

    // Padding lives on the inner wrapper now, not on the `<td>`. The
    // `p-0` is what overrides `<TableCell>`'s default `p-2`. Any
    // padding utility on the `<td>` would re-introduce the dead-zone
    // bug — every variant is forbidden.
    expectClasses(td, ["p-0"]);
    expectNoClasses(td, [
      "p-1",
      "p-1.5",
      "p-2",
      "p-3",
      "p-4",
      "px-1",
      "px-2",
      "px-3",
      "px-4",
      "py-1",
      "py-1.5",
      "py-2",
      "py-3",
      "pl-2",
      "pl-3",
      "pl-4",
    ]);
  });

  it("body cell focusable carries `block h-full w-full` so it fills the <td>", async () => {
    const { container } = await renderTable();
    const { focusable } = findCellShape(container, "grid_cell:0:title");

    // The `<FocusScope>`'s rendered `<div>` is what the spatial-nav
    // kernel registers (its `getBoundingClientRect()` is the cell's
    // hit rect). Sizing it `block h-full w-full` makes its rect equal
    // the `<td>`'s rect.
    expectClasses(focusable, ["block", "h-full", "w-full"]);
  });

  it("body cell inner click wrapper carries `block h-full w-full px-3 py-1.5`", async () => {
    const { container } = await renderTable();
    const { innerWrapper } = findCellShape(container, "grid_cell:0:title");

    // The inner wrapper is the legacy click target (`onClick` /
    // `onDoubleClick` are attached here so they fire before
    // `FocusScope.onClick` calls `e.stopPropagation()`). It carries
    // the cell padding that previously lived on the `<td>`, plus the
    // fill sizing so the click region equals the visible cell.
    expectClasses(innerWrapper, [
      "block",
      "h-full",
      "w-full",
      "px-3",
      "py-1.5",
    ]);
  });

  it("first-column body cell inner wrapper carries the extra `pl-4`", async () => {
    const { container } = await renderTable();
    const { innerWrapper } = findCellShape(container, "grid_cell:0:title");

    // `pl-4` is the leftmost-column extra-left-padding rule. After the
    // fix, it lives on the inner wrapper alongside `px-3` (tailwind-merge
    // resolves to `pl-4 pr-3 py-1.5` at runtime). A regression that
    // moved `pl-4` back to the `<td>` would re-introduce a 16-px-wide
    // dead zone on the leftmost cell of every row — pin it here.
    expectClasses(innerWrapper, ["pl-4"]);
  });
});

describe("DataTable cell click region — row label (RowSelector)", () => {
  beforeEach(() => {
    mockInvoke.mockClear();
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it("row label <td> carries `p-0` and no padding utilities", async () => {
    const { container } = await renderTable();
    const { td } = findCellShape(container, "row_label:0");
    expect(td.getAttribute("data-testid")).toBe("row-selector");

    expectClasses(td, ["p-0"]);
    expectNoClasses(td, ["p-1", "p-2", "py-1", "py-1.5", "py-2"]);
  });

  it("row label focusable carries `block h-full w-full` so it fills the <td>", async () => {
    const { container } = await renderTable();
    const { focusable } = findCellShape(container, "row_label:0");

    expectClasses(focusable, ["block", "h-full", "w-full"]);
  });

  it("row label inner click wrapper carries `block h-full w-full py-1.5`", async () => {
    const { container } = await renderTable();
    const { innerWrapper } = findCellShape(container, "row_label:0");

    // Row label's vertical padding (`py-1.5`) used to live on the
    // `<td>` — same dead-zone bug as body cells, just smaller. After
    // the fix it lives on the inner wrapper. `px-0` is documented
    // intent (no horizontal padding in the row-label cell, the
    // `text-center` on the `<td>` handles horizontal positioning).
    expectClasses(innerWrapper, [
      "block",
      "h-full",
      "w-full",
      "py-1.5",
      "px-0",
    ]);
  });
});
