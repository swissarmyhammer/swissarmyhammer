/**
 * Grid view: `data-cell-cursor` suppression when focus is outside `ui:grid`,
 * plus the single-focus-visual guarantee on a focused cell.
 *
 * Asserts the contract from kanban task `01KQ333G7N2Q6N0504H9NXEF10`:
 *
 *   1. When the focused moniker is NOT a `grid_cell:R:K` (e.g. it's
 *      `ui:navbar`, `ui:sidebar`, an entity moniker, or `null`), no cell
 *      stamps the `data-cell-cursor` attribute.
 *   2. When the focused moniker IS a grid cell, exactly one cell carries
 *      the `data-cell-cursor` attribute -- the one identified by the
 *      moniker.
 *
 * Without this suppression the grid falls back to its internal default
 * cursor of `{0, 0}` whenever focus moves elsewhere, leaving a stale
 * marker on the top-left cell.
 *
 * Plus the architectural rule from `01KQ573XBT0GFQWVY6QEZQ74R6`:
 *
 *   3. A focused grid cell renders EXACTLY ONE visible focus decoration —
 *      the `<FocusIndicator>` rendered by the cell's `<Focusable>`. The
 *      previous implementation also painted a `ring-2 ring-primary
 *      ring-inset` border on the same cell driven by `isCursor`, which
 *      double-decorated the same focus state. Removed in favour of the
 *      bar so "one decorator, one place" holds across the whole app.
 *
 * The test mounts the real provider stack (schema, entity store, focus,
 * spatial-nav) and the real `useGrid` + real `DataTable` so the
 * focus-moniker -> cursor derivation path is exercised end-to-end.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, act, fireEvent } from "@testing-library/react";
import { useEffect } from "react";

// ---------------------------------------------------------------------------
// Tauri API mocks -- must come before component imports.
// ---------------------------------------------------------------------------

const mockInvoke = vi.hoisted(() =>
  vi.fn(async (_cmd: string, _args?: unknown): Promise<unknown> => undefined),
);
const listenHandlers = vi.hoisted(
  () => ({}) as Record<string, (event: { payload: unknown }) => void>,
);
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...a: unknown[]) => mockInvoke(...(a as [string, unknown?])),
}));
// Capture handlers so tests that need to drive `focus-changed` events
// (e.g. the single-focus-visual guard) can invoke them directly. Other
// tests in this file don't touch the handler map and just rely on the
// `listen()` -> Promise<unsubscribe> contract.
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn((event: string, handler: (e: { payload: unknown }) => void) => {
    listenHandlers[event] = handler;
    return Promise.resolve(() => {
      delete listenHandlers[event];
    });
  }),
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

// Mock the perspective container so the grid gets a stable activePerspective
// without dragging in the heavier PerspectivesContainer.
vi.mock("@/components/perspective-container", () => ({
  useActivePerspective: () => ({
    activePerspective: null,
    applySort: (entities: unknown[]) => entities,
    groupField: undefined,
  }),
}));

// ---------------------------------------------------------------------------
// Imports after mocks.
// ---------------------------------------------------------------------------

import { GridView } from "./grid-view";
import { SchemaProvider } from "@/lib/schema-context";
import { EntityStoreProvider } from "@/lib/entity-store-context";
import {
  EntityFocusProvider,
  useFocusActions,
} from "@/lib/entity-focus-context";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { FocusLayer } from "@/components/focus-layer";
import { FieldUpdateProvider } from "@/lib/field-update-context";
import { UIStateProvider } from "@/lib/ui-state-context";
import { TooltipProvider } from "@/components/ui/tooltip";
import { CommandBusyProvider } from "@/lib/command-scope";
import {
  asLayerName,
  asMoniker,
  type FocusChangedPayload,
  type SpatialKey,
} from "@/types/spatial";
import { gridCellMoniker } from "@/lib/moniker";
import type { Entity, EntitySchema } from "@/types/kanban";

// ---------------------------------------------------------------------------
// Task schema -- two columns so we can prove the column-key path through
// the moniker resolves correctly.
// ---------------------------------------------------------------------------

const TASK_SCHEMA = {
  entity: {
    name: "task",
    entity_type: "task",
    search_display_field: "title",
  },
  fields: [
    { name: "title", type: "string", section: "header", display: "text" },
    { name: "status", type: "string", section: "header", display: "text" },
  ],
} as unknown as EntitySchema;

// ---------------------------------------------------------------------------
// Test helpers.
// ---------------------------------------------------------------------------

/** Build a task Entity seed. */
function seedTask(id: string, title: string, status: string): Entity {
  return {
    entity_type: "task",
    id,
    moniker: `task:${id}`,
    fields: { title, status },
  };
}

/** Three tasks -- enough to assert "exactly one cell" with ample siblings. */
function threeTasks(): Entity[] {
  return [
    seedTask("t1", "Alpha", "todo"),
    seedTask("t2", "Beta", "doing"),
    seedTask("t3", "Gamma", "done"),
  ];
}

/**
 * Probe that captures `setFocus` from the entity-focus context into a
 * test-owned ref. Tests use this to drive focus to specific monikers
 * without simulating the full keybinding pipeline -- the same `setFocus`
 * a click handler would call.
 */
interface FocusRef {
  setFocus: ((moniker: string | null) => void) | null;
}

function FocusProbe({ focusRef }: { focusRef: FocusRef }) {
  const { setFocus } = useFocusActions();
  useEffect(() => {
    focusRef.setFocus = setFocus;
    return () => {
      focusRef.setFocus = null;
    };
  }, [setFocus, focusRef]);
  return null;
}

/**
 * Mount `GridView` inside the production-shaped provider stack. Mirrors
 * `grid-view.nav-is-eventdriven.test.tsx`'s `GridHarness` -- the providers
 * are the same ones that sit under `RustEngineContainer` in `App.tsx`.
 */
function GridHarness({
  entities,
  focusRef,
}: {
  entities: Record<string, Entity[]>;
  focusRef: FocusRef;
}) {
  return (
    <CommandBusyProvider>
      <SpatialFocusProvider>
        <FocusLayer name={asLayerName("window")}>
          <TooltipProvider>
            <SchemaProvider>
              <EntityStoreProvider entities={entities}>
                <EntityFocusProvider>
                  <FieldUpdateProvider>
                    <UIStateProvider>
                      <FocusProbe focusRef={focusRef} />
                      <GridView
                        view={{
                          id: "v-cursor",
                          name: "Tasks",
                          kind: "grid",
                          entity_type: "task",
                        }}
                      />
                    </UIStateProvider>
                  </FieldUpdateProvider>
                </EntityFocusProvider>
              </EntityStoreProvider>
            </SchemaProvider>
          </TooltipProvider>
        </FocusLayer>
      </SpatialFocusProvider>
    </CommandBusyProvider>
  );
}

/** Default invoke responses for the mount-time IPCs the providers fire. */
async function defaultInvokeImpl(
  cmd: string,
  _args?: unknown,
): Promise<unknown> {
  if (cmd === "list_entity_types") return ["task"];
  if (cmd === "get_entity_schema") return TASK_SCHEMA;
  if (cmd === "get_ui_state")
    return {
      palette_open: false,
      palette_mode: "command",
      keymap_mode: "cua",
      scope_chain: [],
      open_boards: [],
      windows: {},
      recent_boards: [],
    };
  if (cmd === "get_undo_state") return { can_undo: false, can_redo: false };
  if (cmd === "dispatch_command") return undefined;
  if (cmd === "list_commands_for_scope") return [];
  return undefined;
}

// ---------------------------------------------------------------------------
// Tests.
// ---------------------------------------------------------------------------

describe("GridView -- cursor-ring suppression outside ui:grid", () => {
  beforeEach(() => {
    mockInvoke.mockClear();
    mockInvoke.mockImplementation(defaultInvokeImpl);
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it("renders no [data-cell-cursor] when focus is on a non-grid moniker", async () => {
    const focusRef: FocusRef = { setFocus: null };
    const entities = { task: threeTasks() };

    let result!: ReturnType<typeof render>;
    await act(async () => {
      result = render(<GridHarness entities={entities} focusRef={focusRef} />);
    });

    // Let initial mount-effects (including the grid's
    // `useInitialCellFocus` seed) settle.
    await act(async () => {
      await new Promise((r) => setTimeout(r, 50));
    });

    // Move focus OUT of the grid. `ui:navbar` is a non-grid_cell moniker
    // shape that mirrors the production navbar focus -- exactly the case
    // the task description calls out as problematic.
    expect(focusRef.setFocus).toBeTruthy();
    await act(async () => {
      focusRef.setFocus?.("ui:navbar");
    });

    // After focus leaves the grid, no cell should carry the cursor-ring
    // marker. The previous behaviour fell back to the internal `{0, 0}`
    // cursor and painted the ring on the top-left cell.
    const { container } = result;
    const ringedCells = container.querySelectorAll("[data-cell-cursor]");
    expect(ringedCells.length).toBe(0);
  });

  it("renders exactly one [data-cell-cursor] when focus is on a grid_cell moniker", async () => {
    const focusRef: FocusRef = { setFocus: null };
    const entities = { task: threeTasks() };

    let result!: ReturnType<typeof render>;
    await act(async () => {
      result = render(<GridHarness entities={entities} focusRef={focusRef} />);
    });

    await act(async () => {
      await new Promise((r) => setTimeout(r, 50));
    });

    expect(focusRef.setFocus).toBeTruthy();

    // Focus the cell at row 1 (second row), `title` column.
    const targetMoniker = gridCellMoniker(1, "title");
    await act(async () => {
      focusRef.setFocus?.(targetMoniker);
    });

    const { container } = result;
    const ringedCells = container.querySelectorAll("[data-cell-cursor]");
    expect(ringedCells.length).toBe(1);

    // The ringed cell should be inside the row whose `data-moniker` is
    // `task:t2` (the second seeded task). The cell's own
    // `data-cell-cursor` attribute encodes the row/col it claims to be.
    const ringedCell = ringedCells[0] as HTMLElement;
    expect(ringedCell.getAttribute("data-cell-cursor")).toBe("1:title");
  });

  it("renders no [data-cell-cursor] when the focused moniker is null", async () => {
    const focusRef: FocusRef = { setFocus: null };
    const entities = { task: threeTasks() };

    let result!: ReturnType<typeof render>;
    await act(async () => {
      result = render(<GridHarness entities={entities} focusRef={focusRef} />);
    });

    await act(async () => {
      await new Promise((r) => setTimeout(r, 50));
    });

    // Explicitly clear focus -- a fresh app-mount before any cell is
    // clicked, or after a defocus action.
    expect(focusRef.setFocus).toBeTruthy();
    await act(async () => {
      focusRef.setFocus?.(null);
    });

    const { container } = result;
    const ringedCells = container.querySelectorAll("[data-cell-cursor]");
    expect(ringedCells.length).toBe(0);
  });
});

/**
 * Click-to-cursor regression: a left-click on a cell must update the
 * entity-focus moniker in the spatial-stack-mounted production harness.
 *
 * This pins the warning from the review of
 * `01KNQXZZ9VQBHFX091P0K4F4YC`: `Focusable.onClick` calls
 * `e.stopPropagation()` (per the long-standing FocusScope convention so
 * leaf clicks don't re-fire on enclosing zones). Before the fix, the
 * click handler attached to the surrounding `<TableCell>` was swallowed
 * before it could run `setFocus(gridCellMoniker(...))`, so the cursor
 * ring (derived from entity-focus) regressed click-to-move-cursor in
 * production. The fix mounts the click handler INSIDE the `<Focusable>`
 * via a thin wrapper `<div>`, so the inner handler fires before the
 * primitive's `stopPropagation()` — both spatial focus AND entity
 * focus update on a single click.
 *
 * The assertion lifts the cursor-ring as the observable proxy for
 * entity-focus: a `data-cell-cursor` attribute appears on exactly the
 * clicked cell when the entity-focus moniker matches.
 */
describe("GridView -- click-to-cursor regression (spatial path)", () => {
  beforeEach(() => {
    mockInvoke.mockClear();
    mockInvoke.mockImplementation(defaultInvokeImpl);
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it("clicking a cell sets entity-focus and lights the cursor ring on that cell", async () => {
    const focusRef: FocusRef = { setFocus: null };
    const entities = { task: threeTasks() };

    let result!: ReturnType<typeof render>;
    await act(async () => {
      result = render(<GridHarness entities={entities} focusRef={focusRef} />);
    });

    // Let mount-effects (including `useInitialCellFocus`) settle so the
    // initial focus has landed somewhere predictable before we click.
    await act(async () => {
      await new Promise((r) => setTimeout(r, 50));
    });

    // The seed lands on `grid_cell:0:title`. Move focus elsewhere first
    // so the click has something to change — that way "ring lights up
    // on the clicked cell" cannot be a false pass from the seed already
    // matching the click target.
    expect(focusRef.setFocus).toBeTruthy();
    await act(async () => {
      focusRef.setFocus?.("ui:navbar");
    });

    // No ring while focus is outside the grid.
    expect(result.container.querySelectorAll("[data-cell-cursor]").length).toBe(
      0,
    );

    // Find the cell at (row=1, col=`status`) — the second row's status
    // column. We target the inner `<Focusable>` wrapper (where the new
    // click handler lives in the spatial path). The data-table renders
    // `<Focusable>` with `data-moniker="grid_cell:1:status"`.
    const targetMoniker = "grid_cell:1:status";
    const focusableEl = result.container.querySelector(
      `[data-moniker="${targetMoniker}"]`,
    ) as HTMLElement | null;
    expect(focusableEl).not.toBeNull();

    // Click an element INSIDE the focusable's subtree — that's where the
    // inner click wrapper is. Bubble order: target → inner div onClick
    // (legacy entity-focus) → Focusable's outer onClick (spatial focus
    // + stopPropagation). Both fire on one click.
    await act(async () => {
      fireEvent.click(focusableEl!.firstElementChild ?? focusableEl!);
    });

    // The cursor ring (driven by entity-focus → moniker → grid-cell
    // cursor → `data-cell-cursor`) must now mark exactly the clicked
    // cell. If `Focusable.onClick`'s `stopPropagation()` regressed back
    // to swallowing the entity-focus update, this assertion catches it
    // — zero rings would render instead.
    const ringedCells = result.container.querySelectorAll("[data-cell-cursor]");
    expect(ringedCells.length).toBe(1);
    expect(
      (ringedCells[0] as HTMLElement).getAttribute("data-cell-cursor"),
    ).toBe("1:status");
  });
});

/**
 * Architectural guard for "one decorator, one place" on a focused grid cell.
 *
 * The previous shape painted two simultaneous focus decorations on the
 * focused cell:
 *
 *   - `<FocusIndicator>` — bar to the left of the cell, rendered by the
 *     cell's `<Focusable>` from its `useFocusClaim` React state.
 *   - `ring-2 ring-primary ring-inset` — cell-spanning border, applied to
 *     the surrounding `<TableCell>` via `cellClasses` whenever
 *     `isCursor === true` (i.e. the same focus state the bar already
 *     reflects).
 *
 * Both were driven by the same Rust-side spatial focus state — the focused
 * moniker — so they always lit together. That violates architectural rule
 * 3 from `01KQ573XBT0GFQWVY6QEZQ74R6` ("the visible focus indicator
 * renders in exactly one component"). The cell ring was removed; the bar
 * is now the sole focus decoration on grid cells, identical to every
 * other `<Focusable>` in the app.
 *
 * This test asserts:
 *
 *   1. A focused grid cell renders exactly one `<FocusIndicator>` (its
 *      own — the focused leaf's), not multiple stacked decorators.
 *   2. No element rendered by the grid carries the removed cursor-ring
 *      classes (`ring-2 ring-primary ring-inset`). A regression that
 *      reintroduces the ring on `<TableCell>` would trip this guard.
 */
describe("GridView -- single-focus-visual on a focused cell", () => {
  beforeEach(() => {
    mockInvoke.mockClear();
    mockInvoke.mockImplementation(defaultInvokeImpl);
    // Clear any captured listeners from prior tests so this suite drives
    // its own `focus-changed` payload without picking up a stale handler.
    for (const key of Object.keys(listenHandlers)) {
      delete listenHandlers[key];
    }
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it("a focused grid cell renders exactly one <FocusIndicator> and no cell-spanning ring", async () => {
    const focusRef: FocusRef = { setFocus: null };
    const entities = { task: threeTasks() };

    let result!: ReturnType<typeof render>;
    await act(async () => {
      result = render(<GridHarness entities={entities} focusRef={focusRef} />);
    });

    // Wait for the spatial-nav stack and `useInitialCellFocus` to settle —
    // by this point every grid cell has registered via
    // `spatial_register_focusable` and we can recover its `SpatialKey`
    // from the mocked invoke history.
    await act(async () => {
      await new Promise((r) => setTimeout(r, 50));
    });

    // Set entity focus first — this drives the `data-cell-cursor` debug
    // attribute on the matching cell (legacy entity-focus → moniker →
    // grid-cell cursor pipeline). The single-focus-visual contract holds
    // even when both focus signals point at the same cell.
    const targetMoniker = gridCellMoniker(1, "status");
    expect(focusRef.setFocus).toBeTruthy();
    await act(async () => {
      focusRef.setFocus?.(targetMoniker);
    });

    // Dispatch a `focus-changed` event for the targeted cell's
    // `SpatialKey`. In production the Rust spatial layer fires this in
    // response to the click → `spatial_focus` → kernel update path; in
    // the test we drive it directly off the `spatial_register_focusable`
    // call recorded for the targeted moniker. Without this, the cell's
    // `useFocusClaim` callback never flips and the bar is never rendered.
    const registerCalls = mockInvoke.mock.calls.filter(
      (c) => c[0] === "spatial_register_focusable",
    );
    const targetRegistration = registerCalls.find(
      (c) => (c[1] as { moniker?: string })?.moniker === targetMoniker,
    );
    expect(targetRegistration).toBeTruthy();
    const targetKey = (targetRegistration![1] as { key: SpatialKey }).key;

    await act(async () => {
      const payload: FocusChangedPayload = {
        window_label: "main" as FocusChangedPayload["window_label"],
        prev_key: null,
        next_key: targetKey,
        next_moniker: asMoniker(targetMoniker),
      };
      listenHandlers["focus-changed"]?.({ payload });
    });

    const { container } = result;

    // 1. Exactly one `<FocusIndicator>` is rendered as a descendant of
    //    the focused cell's `<Focusable>` — and that's the only one in
    //    the grid (no second decorator on a sibling element).
    const indicators = container.querySelectorAll(
      "[data-testid='focus-indicator']",
    );
    expect(indicators.length).toBe(1);
    const focusedFocusable = container.querySelector(
      `[data-moniker="${targetMoniker}"]`,
    ) as HTMLElement | null;
    expect(focusedFocusable).not.toBeNull();
    expect(focusedFocusable!.contains(indicators[0])).toBe(true);

    // 2. The cell-spanning ring (`ring-2 ring-primary ring-inset`) was
    //    removed in `01KQ573XBT0GFQWVY6QEZQ74R6` to enforce "one
    //    decorator, one place". A regression that reintroduces the ring
    //    on the focused `<TableCell>` would trip this — the focus bar is
    //    the canonical decoration; the cell ring would be a duplicate.
    const ringed = container.querySelectorAll(
      ".ring-2.ring-primary.ring-inset",
    );
    expect(ringed.length).toBe(0);
  });
});
