/**
 * DataTable row structure tests.
 *
 * Asserts that each data row has exactly (1 selector + N field columns) <td>
 * elements — no extra cells from scope wrappers, focus highlights, or
 * context providers.
 */

import { describe, it, expect, vi } from "vitest";
import { render } from "@testing-library/react";

// ---------------------------------------------------------------------------
// jsdom stubs
// ---------------------------------------------------------------------------
Element.prototype.scrollIntoView = vi.fn();

// ---------------------------------------------------------------------------
// Mocks — before component imports
// ---------------------------------------------------------------------------

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(() => Promise.resolve(null)),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));
vi.mock("@tauri-apps/api/webviewWindow", () => ({
  getCurrentWebviewWindow: () => ({
    label: "main",
    listen: vi.fn(() => Promise.resolve(() => {})),
  }),
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
import type { CommandDef } from "@/lib/command-scope";

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

/** Stub entity commands factory — returns one inspect command per entity. */
function stubRowCommands(entity: Entity): CommandDef[] {
  return [
    {
      id: "ui.inspect",
      name: `Inspect ${entity.entity_type}`,
      target: entity.moniker,
      contextMenu: true,
    },
  ];
}

function renderTable(
  props: Partial<React.ComponentProps<typeof DataTable>> = {},
) {
  return render(
    <EntityFocusProvider>
      <DataTable
        columns={COLUMNS}
        rows={ENTITIES}
        grid={makeGrid()}
        showRowSelector={true}
        rowEntityCommands={stubRowCommands}
        {...props}
      />
    </EntityFocusProvider>,
  );
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("DataTable row structure", () => {
  it("each data row has exactly selector + field columns <td> elements", () => {
    const { container } = renderTable();
    const tbody = container.querySelector("tbody")!;
    const rows = tbody.querySelectorAll("tr");
    expect(rows.length).toBe(ENTITIES.length);

    for (const row of rows) {
      const cells = row.querySelectorAll("td");
      // 1 selector + 2 field columns = 3
      expect(cells.length).toBe(1 + COLUMNS.length);
    }
  });

  it("selector cell shows row number", () => {
    const { container } = renderTable();
    const selectors = container.querySelectorAll(
      "[data-testid='row-selector']",
    );
    expect(selectors.length).toBe(ENTITIES.length);
    expect(selectors[0].textContent).toBe("1");
    expect(selectors[1].textContent).toBe("2");
    expect(selectors[2].textContent).toBe("3");
  });

  it("no <div> between <tbody> and <tr>", () => {
    const { container } = renderTable();
    const tbody = container.querySelector("tbody")!;
    // Every direct child of tbody should be a <tr>
    for (const child of tbody.children) {
      expect(child.tagName).toBe("TR");
    }
  });

  it("row has data-moniker attribute with entity moniker", () => {
    const { container } = renderTable();
    const tbody = container.querySelector("tbody")!;
    const rows = tbody.querySelectorAll("tr");
    expect(rows[0].getAttribute("data-moniker")).toBe("task:t1");
    expect(rows[1].getAttribute("data-moniker")).toBe("task:t2");
  });

  it("column count matches with showRowSelector=false", () => {
    const { container } = renderTable({ showRowSelector: false });
    const tbody = container.querySelector("tbody")!;
    const rows = tbody.querySelectorAll("tr");
    for (const row of rows) {
      const cells = row.querySelectorAll("td");
      expect(cells.length).toBe(COLUMNS.length);
    }
  });

  it("column count matches without rowEntityCommands", () => {
    const { container } = renderTable({ rowEntityCommands: undefined });
    const tbody = container.querySelector("tbody")!;
    const rows = tbody.querySelectorAll("tr");
    for (const row of rows) {
      const cells = row.querySelectorAll("td");
      expect(cells.length).toBe(1 + COLUMNS.length);
    }
  });
});

describe("DataTable grouping sync", () => {
  it("clearing grouping prop returns to flat layout", () => {
    // Render grouped by status — should show group header rows
    const { container, rerender } = render(
      <EntityFocusProvider>
        <DataTable
          columns={COLUMNS}
          rows={ENTITIES}
          grid={makeGrid()}
          showRowSelector={true}
          rowEntityCommands={stubRowCommands}
          grouping={["status"]}
        />
      </EntityFocusProvider>,
    );

    // With grouping active, re-render with grouping cleared
    // to verify the table returns to a flat layout.
    rerender(
      <EntityFocusProvider>
        <DataTable
          columns={COLUMNS}
          rows={ENTITIES}
          grid={makeGrid()}
          showRowSelector={true}
          rowEntityCommands={stubRowCommands}
          grouping={undefined}
        />
      </EntityFocusProvider>,
    );

    // After clearing, all rows should be flat data rows with entity monikers
    const flatRows = container.querySelectorAll("tbody tr[data-moniker]");
    expect(flatRows.length).toBe(ENTITIES.length);
  });

  it("renders flat layout when no grouping prop is provided", () => {
    const { container } = renderTable();
    const rows = container.querySelectorAll("tbody tr[data-moniker]");
    expect(rows.length).toBe(ENTITIES.length);
  });
});

describe("DataTable header focus wiring", () => {
  // Regression guards for the column-header spatial-nav target.
  //
  // Each `<th>` is wrapped in a `FocusScope` with moniker
  // `column-header:<fieldName>` so pressing `k` (up) from a body cell
  // lands on the header above instead of skipping past to the
  // perspective bar. The `data-table-header-focus` class repositions
  // the left-edge focus bar inside the `<th>` — cells sit inside a
  // `<tr>` whose enclosing `<table>` clips the default negative-left
  // offset.

  it("each <th> carries data-moniker=`column-header:<fieldName>`", () => {
    const { container } = renderTable();
    const thead = container.querySelector("thead")!;
    const headers = thead.querySelectorAll("th[data-moniker]");
    // One header per field column; the row-selector <th> is unmonikered.
    expect(headers.length).toBe(COLUMNS.length);
    const monikers = Array.from(headers).map((th) =>
      th.getAttribute("data-moniker"),
    );
    expect(monikers).toEqual(["column-header:title", "column-header:status"]);
  });

  it("each data column <th> carries `data-table-header-focus`", () => {
    const { container } = renderTable();
    const headers = container.querySelectorAll("thead th[data-moniker]");
    expect(headers.length).toBe(COLUMNS.length);
    for (const th of headers) {
      expect(th.className).toContain("data-table-header-focus");
    }
  });
});

describe("DataTable focus class wiring", () => {
  // Regression guards for the focus-bar override classes.
  //
  // With the global `[data-focused]` ring removed, the left-edge bar
  // is the only focus indicator — and it lives on `::before` at
  // `left: -0.5rem` by default. A `<td>` sits inside a `<tr>` whose
  // parent `<table>` and view container clip horizontal overflow, so
  // the negative-left bar never shows. The `cell-focus` class
  // repositions the bar inside the cell via a CSS override (see
  // `index.css`). These tests lock the class in on both `<td>` types
  // that participate in the grid cursor's focus.

  it("data cells carry `cell-focus` so the focus bar renders inside the <td>", () => {
    const { container } = renderTable();
    // Skip the row selector <td> (data-testid='row-selector') and
    // look at the field cells — they carry the data-moniker for the
    // (entity, field) pair produced by `fieldMoniker`.
    const fieldCells = container.querySelectorAll(
      "tbody td[data-moniker]:not([data-testid='row-selector'])",
    );
    expect(fieldCells.length).toBeGreaterThan(0);
    for (const cell of fieldCells) {
      expect(cell.className).toContain("cell-focus");
    }
  });

  it("row selector <td> carries `cell-focus` for the same reason", () => {
    const { container } = renderTable();
    const selectors = container.querySelectorAll(
      "[data-testid='row-selector']",
    );
    expect(selectors.length).toBe(ENTITIES.length);
    for (const sel of selectors) {
      expect(sel.className).toContain("cell-focus");
    }
  });
});
