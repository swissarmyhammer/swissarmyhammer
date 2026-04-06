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
    fields: { title: "Task 1", status: "todo" },
  },
  {
    entity_type: "task",
    id: "t2",
    fields: { title: "Task 2", status: "done" },
  },
  {
    entity_type: "task",
    id: "t3",
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
      target: `${entity.entity_type}:${entity.id}`,
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

    // With grouping, there should be group header rows (fewer data rows visible at top level)
    const groupedRows = container.querySelectorAll("tbody tr");
    const hasGroupHeaders = Array.from(groupedRows).some(
      (row) => row.querySelector("[data-group-header]") !== null,
    );

    // Re-render with grouping cleared
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
