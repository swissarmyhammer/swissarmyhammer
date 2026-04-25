/**
 * DataTable row structure tests.
 *
 * Asserts that each data row has exactly (1 selector + N field columns) <td>
 * elements — no extra cells from scope wrappers, focus highlights, or
 * context providers.
 */

import { describe, it, expect, vi } from "vitest";
import { render, fireEvent, act } from "@testing-library/react";

// ---------------------------------------------------------------------------
// jsdom stubs
// ---------------------------------------------------------------------------
Element.prototype.scrollIntoView = vi.fn();

// ---------------------------------------------------------------------------
// Mocks — before component imports
// ---------------------------------------------------------------------------

vi.mock("@tauri-apps/api/core", () => ({
  // Return `[]` for list-like invoke calls (e.g. `list_commands_for_scope`
  // fired by `useContextMenu`). Returning null tripped a TypeError inside
  // `useContextMenu` when a test right-clicked a row; the empty array
  // short-circuits cleanly without hiding real failures.
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
// Imports — after mocks
// ---------------------------------------------------------------------------

import { DataTable, type DataTableColumn } from "./data-table";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
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
  {
    entity_type: "task",
    id: "t3",
    moniker: "task:t3",
    fields: { title: "Task 3", status: "todo" },
  },
];

/** Minimal grid state — no editing, cursor at 0,0. */
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

/**
 * Render `<DataTable>` and flush all post-mount effects inside an
 * `act` scope.
 *
 * Both the legacy `setVisibleRowCount` `useEffect` in `DataTable` and
 * `useVirtualizer`'s `ResizeObserver`-driven `rerender` fire async
 * post-mount and would otherwise emit "update not wrapped in act(...)"
 * warnings in tests. Wrapping the render in `await act(async () => {})`
 * is the standard React Testing Library pattern for silencing these
 * legitimate-but-noisy warnings.
 */
async function renderTable(
  props: Partial<React.ComponentProps<typeof DataTable>> = {},
) {
  let result!: ReturnType<typeof render>;
  await act(async () => {
    result = render(
      <EntityFocusProvider>
        <DataTable
          columns={COLUMNS}
          rows={ENTITIES}
          grid={makeGrid()}
          showRowSelector={true}
          {...props}
        />
      </EntityFocusProvider>,
    );
  });
  return result;
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("DataTable row structure", () => {
  it("each data row has exactly selector + field columns <td> elements", async () => {
    const { container } = await renderTable();
    const tbody = container.querySelector("tbody")!;
    const rows = tbody.querySelectorAll("tr");
    expect(rows.length).toBe(ENTITIES.length);

    for (const row of rows) {
      const cells = row.querySelectorAll("td");
      // 1 selector + 2 field columns = 3
      expect(cells.length).toBe(1 + COLUMNS.length);
    }
  });

  it("selector cell shows row number", async () => {
    const { container } = await renderTable();
    const selectors = container.querySelectorAll(
      "[data-testid='row-selector']",
    );
    expect(selectors.length).toBe(ENTITIES.length);
    expect(selectors[0].textContent).toBe("1");
    expect(selectors[1].textContent).toBe("2");
    expect(selectors[2].textContent).toBe("3");
  });

  it("no <div> between <tbody> and <tr>", async () => {
    const { container } = await renderTable();
    const tbody = container.querySelector("tbody")!;
    // Every direct child of tbody should be a <tr>
    for (const child of tbody.children) {
      expect(child.tagName).toBe("TR");
    }
  });

  it("row has data-moniker attribute with entity moniker", async () => {
    const { container } = await renderTable();
    const tbody = container.querySelector("tbody")!;
    const rows = tbody.querySelectorAll("tr");
    expect(rows[0].getAttribute("data-moniker")).toBe("task:t1");
    expect(rows[1].getAttribute("data-moniker")).toBe("task:t2");
  });

  it("column count matches with showRowSelector=false", async () => {
    const { container } = await renderTable({ showRowSelector: false });
    const tbody = container.querySelector("tbody")!;
    const rows = tbody.querySelectorAll("tr");
    for (const row of rows) {
      const cells = row.querySelectorAll("td");
      expect(cells.length).toBe(COLUMNS.length);
    }
  });
});

describe("DataTable grouping sync", () => {
  it("clearing grouping prop returns to flat layout", async () => {
    // Render grouped by status — should show group header rows
    let result!: ReturnType<typeof render>;
    await act(async () => {
      result = render(
        <EntityFocusProvider>
          <DataTable
            columns={COLUMNS}
            rows={ENTITIES}
            grid={makeGrid()}
            showRowSelector={true}
            grouping={["status"]}
          />
        </EntityFocusProvider>,
      );
    });
    const { container, rerender } = result;

    // With grouping active, re-render with grouping cleared
    // to verify the table returns to a flat layout.
    await act(async () => {
      rerender(
        <EntityFocusProvider>
          <DataTable
            columns={COLUMNS}
            rows={ENTITIES}
            grid={makeGrid()}
            showRowSelector={true}
            grouping={undefined}
          />
        </EntityFocusProvider>,
      );
    });

    // After clearing, all rows should be flat data rows with entity monikers
    const flatRows = container.querySelectorAll("tbody tr[data-moniker]");
    expect(flatRows.length).toBe(ENTITIES.length);
  });

  it("renders flat layout when no grouping prop is provided", async () => {
    const { container } = await renderTable();
    const rows = container.querySelectorAll("tbody tr[data-moniker]");
    expect(rows.length).toBe(ENTITIES.length);
  });
});

describe("DataTable container context menu", () => {
  it("invokes onContainerContextMenu when whitespace below the last row is right-clicked", async () => {
    const handler = vi.fn();
    let result!: ReturnType<typeof render>;
    await act(async () => {
      result = render(
        <EntityFocusProvider>
          <DataTable
            columns={COLUMNS}
            rows={ENTITIES}
            grid={makeGrid()}
            showRowSelector={true}
            onContainerContextMenu={handler}
          />
        </EntityFocusProvider>,
      );
    });
    const { container } = result;

    // Fire contextmenu on the `<table>` element itself — it lives inside
    // the scroll container but is NOT inside any `<tr>`, so it simulates
    // a right-click on the whitespace region between/below rows. The
    // event must bubble up to `onContainerContextMenu` via React synthetic
    // event bubbling; firing on the container directly would pass even if
    // bubbling were broken, which is the test gap this replaces.
    const scrollContainer = container.querySelector("div.flex-1.overflow-auto");
    expect(scrollContainer).not.toBeNull();
    const table = scrollContainer!.querySelector("table");
    expect(table).not.toBeNull();
    fireEvent.contextMenu(table!);
    expect(handler).toHaveBeenCalledTimes(1);
  });

  it("does not fire onContainerContextMenu when a column header is right-clicked", async () => {
    // Right-clicking a `<TableHead>` must NOT bubble to the container
    // handler — otherwise the header's grouping toggle would fire
    // alongside the view-scoped native context menu. The header
    // handler stops propagation explicitly.
    const containerHandler = vi.fn();
    let result!: ReturnType<typeof render>;
    await act(async () => {
      result = render(
        <EntityFocusProvider>
          <DataTable
            columns={COLUMNS}
            rows={ENTITIES}
            grid={makeGrid()}
            showRowSelector={true}
            onContainerContextMenu={containerHandler}
          />
        </EntityFocusProvider>,
      );
    });
    const { container } = result;

    const header = container.querySelector(
      "[data-testid='column-header-title']",
    ) as HTMLElement;
    expect(header).not.toBeNull();

    // Right-click on a header dispatches `column.toggleGrouping()` --
    // a TanStack table state update -- so wrap the event in `act`.
    await act(async () => {
      fireEvent.contextMenu(header);
    });
    expect(containerHandler).not.toHaveBeenCalled();
  });

  it("does not fire onContainerContextMenu when a row's own context menu stops propagation", async () => {
    // `EntityRow.onContextMenu` calls `useContextMenu()` which in turn
    // calls `e.stopPropagation()`. That means even though the row is
    // inside the scroll container, a right-click on the row itself must
    // NOT bubble up to fire `onContainerContextMenu`. Simulate that by
    // calling `stopPropagation()` on the row event before the contextmenu
    // bubbles — the container handler should receive zero calls.
    const containerHandler = vi.fn();
    let result!: ReturnType<typeof render>;
    await act(async () => {
      result = render(
        <EntityFocusProvider>
          <DataTable
            columns={COLUMNS}
            rows={ENTITIES}
            grid={makeGrid()}
            showRowSelector={true}
            onContainerContextMenu={containerHandler}
          />
        </EntityFocusProvider>,
      );
    });
    const { container } = result;

    const firstRow = container.querySelector(
      "tbody tr[data-moniker]",
    ) as HTMLElement;
    expect(firstRow).not.toBeNull();

    // React's onContextMenu handler on EntityRow calls useContextMenu,
    // which is wired to real Tauri invoke(). In this jsdom test,
    // @tauri-apps/api/core is mocked to resolve `null`, so the handler
    // runs through its preventDefault/stopPropagation logic before any
    // real work. That's exactly what the container-vs-row dispatch
    // separation relies on.
    fireEvent.contextMenu(firstRow);
    expect(containerHandler).not.toHaveBeenCalled();
  });
});
