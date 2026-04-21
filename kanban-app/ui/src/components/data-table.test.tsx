/**
 * DataTable row structure tests.
 *
 * Asserts that each data row has exactly (1 selector + N field columns) <td>
 * elements — no extra cells from scope wrappers, focus highlights, or
 * context providers.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render } from "@testing-library/react";
import { userEvent } from "vitest/browser";

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
import {
  EntityFocusProvider,
  useEntityFocus,
} from "@/lib/entity-focus-context";
import { CommandScopeProvider, type CommandDef } from "@/lib/command-scope";
import { FixtureShell } from "@/test/spatial-fixture-shell";
import { fieldMoniker, ROW_SELECTOR_FIELD } from "@/lib/moniker";
import { invoke } from "@tauri-apps/api/core";
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

/**
 * Minimal grid state.
 *
 * `cursor` mirrors the production contract — it is `null` when no data
 * cell is focused (the default for these structural tests, which mount
 * the grid without an active spatial focus). Pass an explicit cursor to
 * simulate a focused cell.
 */
function makeGrid(
  cursor: { row: number; col: number } | null = null,
): UseGridReturn {
  return {
    cursor,
    mode: "normal" as const,
    enterEdit: vi.fn(),
    exitEdit: vi.fn(),
    enterVisual: vi.fn(),
    exitVisual: vi.fn(),
    selection: null,
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

// ---------------------------------------------------------------------------
// Single-visual-focus invariant
// ---------------------------------------------------------------------------

/**
 * The grid cursor is a derived view of spatial focus, not an
 * independent source of truth. The only visual that paints "where the
 * user is" is the `data-focused` attribute written by each cell's
 * `FocusScope` — no cursor-driven row background, no `data-active`
 * attribute, no row-selector background override.
 *
 * These tests assert the redundant cursor-driven visuals are gone:
 *
 * - With a cursor at `(1, 1)` (simulating spatial focus on cell 1,1),
 *   no `<tr>` paints `bg-accent/30`, and the row selector for row 1
 *   does not paint `bg-muted`/`data-active=true`.
 * - With a null cursor (the default when spatial focus is on a
 *   non-cell target or nothing), none of the cursor-driven visuals
 *   appear on any row.
 * - When the cursor moves, no previously-cursor'd row retains its
 *   background.
 *
 * These lock in the companion invariant to the always-focused rule:
 * at most one visual focus at any moment. (The "≥ 1" side is enforced
 * by `FocusScope`'s `data-focused`, which these tests don't exercise
 * because rendering DataTable without an `EntityFocusProvider` focus
 * doesn't set it — that's a separate concern.)
 */
describe("DataTable cursor-driven visuals collapse to spatial focus", () => {
  /** Extract trimmed row background class from the given `<tr>`. */
  function rowClassFor(container: HTMLElement, rowIndex: number): string {
    const rows = container.querySelectorAll("tbody tr[data-moniker]");
    return rows[rowIndex]?.className ?? "";
  }

  /** Extract className for the row-selector cell at the given row. */
  function selectorClassFor(container: HTMLElement, rowIndex: number): string {
    const selectors = container.querySelectorAll(
      "[data-testid='row-selector']",
    );
    return selectors[rowIndex]?.className ?? "";
  }

  it("no row paints bg-accent/30 when cursor is null", () => {
    const { container } = renderTable({ grid: makeGrid(null) });
    for (let i = 0; i < ENTITIES.length; i++) {
      expect(rowClassFor(container, i)).not.toContain("bg-accent");
    }
  });

  it("no row paints bg-accent/30 even when cursor is set", () => {
    // With grid.cursor driven by spatial focus, the cursor row no
    // longer paints a background — the `<td>`'s `data-focused` bar is
    // the single visual. Setting cursor={1,1} simulates spatial focus
    // on cell (1,1); no `<tr>` should pick up `bg-accent/30`.
    const { container } = renderTable({ grid: makeGrid({ row: 1, col: 1 }) });
    for (let i = 0; i < ENTITIES.length; i++) {
      expect(rowClassFor(container, i)).not.toContain("bg-accent");
    }
  });

  it("row selector never carries data-active attribute", () => {
    // `data-active` was redundant with `data-focused` written by the
    // enclosing FocusScope. Removed — asserts it never reappears.
    const { container } = renderTable({ grid: makeGrid({ row: 1, col: 1 }) });
    const selectors = container.querySelectorAll(
      "[data-testid='row-selector']",
    );
    expect(selectors.length).toBe(ENTITIES.length);
    for (const sel of selectors) {
      expect(sel.hasAttribute("data-active")).toBe(false);
    }
  });

  it("row selector never paints bg-muted/text-foreground cursor highlight", () => {
    // Pre-refactor, the cursor row's selector painted
    // `bg-muted text-foreground`. With the derived cursor, that's
    // redundant with `data-focused` — removed.
    const { container } = renderTable({ grid: makeGrid({ row: 1, col: 1 }) });
    for (let i = 0; i < ENTITIES.length; i++) {
      const cls = selectorClassFor(container, i);
      // `bg-muted/50` (with slash opacity) is the base selector
      // background — the cursor-driven `bg-muted` (no opacity) was
      // the removed override. Asserting the exact token with a word
      // boundary ensures we don't false-match `bg-muted/50`.
      expect(cls).not.toMatch(/\bbg-muted\b(?!\/)/);
      expect(cls).not.toMatch(/\btext-foreground\b/);
    }
  });

  it("data cells paint neither bg-primary/10 nor data-active when not selected", () => {
    const { container } = renderTable({ grid: makeGrid({ row: 1, col: 1 }) });
    const fieldCells = container.querySelectorAll(
      "tbody td[data-moniker]:not([data-testid='row-selector'])",
    );
    for (const cell of fieldCells) {
      expect(cell.className).not.toContain("bg-primary/10");
      expect(cell.hasAttribute("data-active")).toBe(false);
    }
  });

  it("moving the cursor does not leave a stale background on the prior row", () => {
    // Re-render with a different cursor; the previous row must not
    // retain any cursor-driven visual. With the collapse-to-spatial-
    // focus refactor, this is vacuously true — no row ever paints a
    // cursor background — but the test guards future regressions if
    // someone reintroduces row-level state.
    const { container, rerender } = render(
      <EntityFocusProvider>
        <DataTable
          columns={COLUMNS}
          rows={ENTITIES}
          grid={makeGrid({ row: 0, col: 0 })}
          showRowSelector={true}
          rowEntityCommands={stubRowCommands}
        />
      </EntityFocusProvider>,
    );
    expect(rowClassFor(container, 0)).not.toContain("bg-accent");

    rerender(
      <EntityFocusProvider>
        <DataTable
          columns={COLUMNS}
          rows={ENTITIES}
          grid={makeGrid({ row: 2, col: 0 })}
          showRowSelector={true}
          rowEntityCommands={stubRowCommands}
        />
      </EntityFocusProvider>,
    );
    // Neither the old cursor row (0) nor the new cursor row (2) should
    // have a cursor-driven background.
    expect(rowClassFor(container, 0)).not.toContain("bg-accent");
    expect(rowClassFor(container, 2)).not.toContain("bg-accent");
    // And the old-cursor selector must not retain `bg-muted` either.
    expect(selectorClassFor(container, 0)).not.toMatch(/\bbg-muted\b(?!\/)/);
  });
});

// ---------------------------------------------------------------------------
// Row selector Enter → ui.inspect (scope shadowing)
// ---------------------------------------------------------------------------

/**
 * Harness that mirrors production's grid-view wiring: the DataTable's
 * `onCellClick` dispatches `setFocus(cellMoniker)` so clicking a data
 * cell moves spatial focus to that cell. Needed for tests that assert
 * keyboard behavior on focused cells, because `DataTableCell` uses
 * `renderContainer={false}` and does not attach its own click handler.
 */
function DataTableWithCellFocus() {
  const { setFocus } = useEntityFocus();
  const onCellClick = (row: number, col: number) => {
    const entity = ENTITIES[row];
    const field = COLUMNS[col].field;
    setFocus(fieldMoniker(entity.entity_type, entity.id, field.name));
  };
  return (
    <DataTable
      columns={COLUMNS}
      rows={ENTITIES}
      grid={makeGrid()}
      showRowSelector={true}
      rowEntityCommands={stubRowCommands}
      onCellClick={onCellClick}
    />
  );
}

/**
 * When the row selector cell is spatially focused and the user presses
 * Enter, the binding must dispatch `ui.inspect` with the row's entity
 * moniker as the explicit target. It must NOT fall through to the
 * grid-level `grid.editEnter` binding (which would drop the grid into
 * edit mode on cell (0, 0)).
 *
 * The tests mount `DataTable` inside a `FixtureShell` which provides the
 * same keybinding wiring as production's `AppShell` — `createKeyHandler`
 * listens on `document` and routes keys through the focused scope's
 * commands. A sibling `CommandScopeProvider` contributes a parent-level
 * `grid.editEnter` binding so the shadow-key resolution is exercised:
 * without the row selector's Enter binding, `grid.editEnter` would fire.
 */
describe("DataTable row selector Enter opens inspector", () => {
  const gridEditEnter = vi.fn();

  /** Parent scope's `grid.editEnter`/`grid.edit` — mirrors `grid-view.tsx`. */
  const gridCommands: CommandDef[] = [
    {
      id: "grid.editEnter",
      name: "Edit Cell (Enter)",
      keys: { vim: "Enter" },
      execute: gridEditEnter,
    },
    {
      id: "grid.edit",
      name: "Edit Cell",
      keys: { vim: "i", cua: "Enter" },
      execute: gridEditEnter,
    },
  ];

  function renderTableInShell() {
    return render(
      <EntityFocusProvider>
        <FixtureShell>
          <CommandScopeProvider commands={gridCommands}>
            <DataTable
              columns={COLUMNS}
              rows={ENTITIES}
              grid={makeGrid()}
              showRowSelector={true}
              rowEntityCommands={stubRowCommands}
            />
          </CommandScopeProvider>
        </FixtureShell>
      </EntityFocusProvider>,
    );
  }

  /**
   * Find all `dispatch_command` invoke calls for the given cmd id.
   *
   * Returns the args object(s) passed as the second argument to `invoke`.
   * Used to assert both that the command fired and with what target.
   */
  function dispatchCallsFor(cmd: string): Record<string, unknown>[] {
    return vi
      .mocked(invoke)
      .mock.calls.filter(
        (c) =>
          c[0] === "dispatch_command" &&
          (c[1] as Record<string, unknown>)?.cmd === cmd,
      )
      .map((c) => c[1] as Record<string, unknown>);
  }

  beforeEach(() => {
    gridEditEnter.mockClear();
    vi.mocked(invoke).mockClear();
    vi.mocked(invoke).mockImplementation(() => Promise.resolve(null));
  });

  it("Enter on row selector dispatches ui.inspect with target=row.moniker", async () => {
    const { container } = renderTableInShell();

    // Click the row-2 selector to focus it (index 1 → ENTITIES[1] = t2).
    const selectors = container.querySelectorAll<HTMLElement>(
      "[data-testid='row-selector']",
    );
    expect(selectors.length).toBe(ENTITIES.length);
    const selectorRow2 = selectors[1];
    expect(selectorRow2.getAttribute("data-moniker")).toBe(
      fieldMoniker("task", "t2", ROW_SELECTOR_FIELD),
    );
    await userEvent.click(selectorRow2);

    // Fire Enter.
    await userEvent.keyboard("{Enter}");

    // Assert ui.inspect was dispatched with explicit target for row 2.
    const inspectCalls = dispatchCallsFor("ui.inspect");
    expect(inspectCalls.length).toBe(1);
    expect(inspectCalls[0].target).toBe("task:t2");

    // Assert the parent's grid.editEnter did NOT fire — the row selector's
    // Enter binding shadows it.
    expect(gridEditEnter).not.toHaveBeenCalled();
  });

  it("Enter on the first-row selector targets that row, not row 0's cell", async () => {
    // Guards the specific bug the task identifies: the grid cursor at
    // init is (0, 0), so the old behavior of `grid.editEnter` with no
    // target would edit cell (0, 0) regardless of which selector was
    // focused. Asserting the target matches the focused selector's row
    // (not row 0) locks that in.
    const { container } = renderTableInShell();

    const selectors = container.querySelectorAll<HTMLElement>(
      "[data-testid='row-selector']",
    );
    const selectorRow3 = selectors[2];
    await userEvent.click(selectorRow3);
    await userEvent.keyboard("{Enter}");

    const inspectCalls = dispatchCallsFor("ui.inspect");
    expect(inspectCalls.length).toBe(1);
    expect(inspectCalls[0].target).toBe("task:t3");
    expect(gridEditEnter).not.toHaveBeenCalled();
  });

  it("Enter on a regular data cell still falls through to grid.editEnter", async () => {
    // Regression guard: the row selector's shadow must be scoped to the
    // selector cell only. Focusing a normal data cell should resolve
    // Enter via the parent's grid binding — same behavior as before.
    //
    // The data cell FocusScope uses `renderContainer={false}` and only
    // sets focus through the DataTable's `onCellClick` prop (production
    // wires this from `grid-view.tsx`). Mirror that wiring here via a
    // harness component that sets focus on click using the same
    // `fieldMoniker` shape.
    const { container } = render(
      <EntityFocusProvider>
        <FixtureShell>
          <CommandScopeProvider commands={gridCommands}>
            <DataTableWithCellFocus />
          </CommandScopeProvider>
        </FixtureShell>
      </EntityFocusProvider>,
    );

    // Click the first row's first field cell (title column).
    const fieldCells = container.querySelectorAll<HTMLElement>(
      "tbody td[data-moniker]:not([data-testid='row-selector'])",
    );
    expect(fieldCells.length).toBeGreaterThan(0);
    await userEvent.click(fieldCells[0]);

    await userEvent.keyboard("{Enter}");

    // Parent's grid.editEnter fires for normal cells in vim mode
    // (FixtureShell's default keymap). ui.inspect must NOT fire.
    expect(gridEditEnter).toHaveBeenCalledTimes(1);
    expect(dispatchCallsFor("ui.inspect").length).toBe(0);
  });
});
