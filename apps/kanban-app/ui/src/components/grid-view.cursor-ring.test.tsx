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
 *      the `<FocusIndicator>` rendered by the cell's `<FocusScope>`. The
 *      previous implementation also painted a `ring-2 ring-primary
 *      ring-inset` border on the same cell driven by `isCursor`, which
 *      double-decorated the same focus state. Removed in favour of the
 *      bar so "one decorator, one place" holds across the whole app.
 *
 * # Harness — shared spatial kernel simulator
 *
 * The test mounts the real provider stack (schema, entity store, focus,
 * spatial-nav) and the real `useGrid` + real `DataTable` so the
 * focus-moniker -> cursor derivation path is exercised end-to-end.
 *
 * Focus mutations reach the kernel over the in-process MCP transport
 * (`focus-mcp.ts::setFocus` → `invoke("command_tool_call", { tool:
 * "focus", op: "set focus", params: { fq, snapshot, window } })`), and
 * the kernel answers by emitting a `focus-changed` Tauri event that the
 * `SpatialFocusProvider` listener fans out to the `EntityFocusProvider`
 * bridge (the sole upstream of the entity-focus store the cursor ring
 * derives from). The shared harness at
 * `@/test/spatial-shadow-registry` models that whole loop — including
 * the kernel's silent-drop conditions (no snapshot / unknown layer /
 * fq missing from the snapshot) — so a focus claim on an UNREGISTERED
 * moniker is correctly dropped, exactly like production. The harness's
 * shadow registry is fed by the global `LayerScopeRegistry` hook that
 * `src/test/setup.ts` installs (mirroring every scope registration as a
 * legacy `spatial_register_scope` invoke entry).
 *
 * Because the kernel only commits focus to registered scopes, the
 * harness mounts a real `<FocusScope moniker="ui:navbar">` sibling next
 * to the grid — the "focus moved out of the grid" cases drive focus to
 * that genuinely-registered scope instead of a synthetic key.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, act, fireEvent, waitFor } from "@testing-library/react";
import { useEffect } from "react";

// ---------------------------------------------------------------------------
// Tauri API mocks -- must come before component imports. File-scoped,
// forwarding to the spies owned by the shared spatial-nav harness module
// (`@/test/spatial-shadow-registry`); `vi.mock` is hoisted to the top of
// THIS file, so the `vi.hoisted` factory resolves the helper's exports
// before the production imports below run.
// ---------------------------------------------------------------------------

const { mockInvoke, mockListen } = await vi.hoisted(async () => {
  const helper = await import("@/test/spatial-shadow-registry");
  return {
    mockInvoke: helper.mockInvoke,
    mockListen: helper.mockListen,
  };
});

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...a: unknown[]) => mockInvoke(...(a as [string, unknown?])),
}));
vi.mock("@tauri-apps/api/event", () => ({
  emit: vi.fn(() => Promise.resolve()),
  listen: (...a: Parameters<typeof mockListen>) => mockListen(...a),
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

import {
  setupSpatialHarness,
  type SpatialHarness,
} from "@/test/spatial-shadow-registry";
import { commandToolCall } from "@/test/mock-command-list";
import { GridView } from "./grid-view";
import { FocusScope } from "./focus-scope";
import { SchemaProvider } from "@/lib/schema-context";
import { EntityStoreProvider } from "@/lib/entity-store-context";
import {
  EntityFocusProvider,
  useFocusActions,
} from "@/lib/entity-focus-context";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { resetWebviewCommandBusForTest } from "@/lib/webview-command-bus";
import { FocusLayer } from "@/components/focus-layer";
import { FieldUpdateProvider } from "@/lib/field-update-context";
import { UIStateProvider } from "@/lib/ui-state-context";
import { TooltipProvider } from "@/components/ui/tooltip";
import { CommandBusyProvider } from "@/lib/command-scope";
import { asSegment, type FullyQualifiedMoniker } from "@/types/spatial";
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
 * primitive the `nav.focus` webview-bus handler's `actions.focus(fq)` is
 * identity-equal to in production.
 */
interface FocusRef {
  setFocus: ((fq: FullyQualifiedMoniker | null) => void) | null;
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
 * `grid-view.spatial-nav.test.tsx`'s `GridHarness` -- the providers are
 * the same ones that sit under `RustEngineContainer` in `App.tsx`.
 *
 * The `ui:navbar` `<FocusScope>` sibling stands in for the production
 * navbar: a genuinely-registered out-of-grid focus target. The kernel
 * (and the shadow harness mirroring its drop conditions) only commits
 * focus to registered scopes, so "move focus out of the grid" must
 * target a real registration.
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
        <FocusLayer name={asSegment("window")}>
          <TooltipProvider>
            <SchemaProvider>
              <EntityStoreProvider entities={entities}>
                <EntityFocusProvider>
                  <FieldUpdateProvider>
                    <UIStateProvider>
                      <FocusProbe focusRef={focusRef} />
                      <FocusScope moniker={asSegment("ui:navbar")}>
                        navbar
                      </FocusScope>
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
  args?: unknown,
): Promise<unknown> {
  // Non-focus MCP tool calls (the focus tool is translated and handled by
  // the shadow harness before falling through here). Serves the global
  // command registry mirror for `list command` / `available command`.
  if (cmd === "command_tool_call") return commandToolCall(args);
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

/** Let mount-effects (scope registration, `useInitialCellFocus`) settle. */
async function flushSetup(): Promise<void> {
  await act(async () => {
    await new Promise((r) => setTimeout(r, 50));
  });
}

/**
 * Resolve the registered FQM for a segment, failing the test loudly when
 * the segment never registered (a harness bug, not a production state).
 */
function registeredFq(
  harness: SpatialHarness,
  segment: string,
): FullyQualifiedMoniker {
  const fq = harness.getRegisteredFqBySegment(segment);
  expect(fq, `expected a registration for segment "${segment}"`).not.toBeNull();
  return fq!;
}

// ---------------------------------------------------------------------------
// Tests.
// ---------------------------------------------------------------------------

describe("GridView -- cursor-ring suppression outside ui:grid", () => {
  let harness: SpatialHarness;

  beforeEach(() => {
    resetWebviewCommandBusForTest();
    harness = setupSpatialHarness({ defaultInvokeImpl });
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
    await flushSetup();

    // Precondition: light the ring by focusing a grid cell through the
    // kernel. This keeps the suppression assertion below non-vacuous --
    // the ring must genuinely turn OFF, not merely never turn on.
    expect(focusRef.setFocus).toBeTruthy();
    const cellFq = registeredFq(harness, gridCellMoniker(0, "title"));
    await act(async () => {
      focusRef.setFocus?.(cellFq);
    });
    await waitFor(() => {
      expect(
        result.container.querySelectorAll("[data-cell-cursor]").length,
      ).toBe(1);
    });

    // Move focus OUT of the grid onto the registered `ui:navbar` scope --
    // exactly the case the task description calls out as problematic.
    const navbarFq = registeredFq(harness, "ui:navbar");
    await act(async () => {
      focusRef.setFocus?.(navbarFq);
    });

    // After focus leaves the grid, no cell should carry the cursor-ring
    // marker. The previous behaviour fell back to the internal `{0, 0}`
    // cursor and painted the ring on the top-left cell.
    await waitFor(() => {
      expect(
        result.container.querySelectorAll("[data-cell-cursor]").length,
      ).toBe(0);
    });
  });

  it("renders exactly one [data-cell-cursor] when focus is on a grid_cell moniker", async () => {
    const focusRef: FocusRef = { setFocus: null };
    const entities = { task: threeTasks() };

    let result!: ReturnType<typeof render>;
    await act(async () => {
      result = render(<GridHarness entities={entities} focusRef={focusRef} />);
    });
    await flushSetup();

    expect(focusRef.setFocus).toBeTruthy();

    // Focus the cell at row 1 (second row), `title` column, via its
    // registered FQM -- the same fully-qualified path a production
    // `nav.focus` dispatch carries.
    const targetMoniker = gridCellMoniker(1, "title");
    const targetFq = registeredFq(harness, targetMoniker);
    await act(async () => {
      focusRef.setFocus?.(targetFq);
    });

    // Exactly one ringed cell, and its `data-cell-cursor` attribute
    // encodes the row/col it claims to be.
    await waitFor(() => {
      const ringedCells =
        result.container.querySelectorAll("[data-cell-cursor]");
      expect(ringedCells.length).toBe(1);
      expect(
        (ringedCells[0] as HTMLElement).getAttribute("data-cell-cursor"),
      ).toBe("1:title");
    });
  });

  it("renders no [data-cell-cursor] when the focused moniker is null", async () => {
    const focusRef: FocusRef = { setFocus: null };
    const entities = { task: threeTasks() };

    let result!: ReturnType<typeof render>;
    await act(async () => {
      result = render(<GridHarness entities={entities} focusRef={focusRef} />);
    });
    await flushSetup();

    // Precondition: light the ring by focusing a grid cell through the
    // kernel, so the explicit clear below genuinely transitions
    // `Some(prev) -> None` (and the ring 1 -> 0).
    expect(focusRef.setFocus).toBeTruthy();
    const cellFq = registeredFq(harness, gridCellMoniker(0, "title"));
    await act(async () => {
      focusRef.setFocus?.(cellFq);
    });
    await waitFor(() => {
      expect(
        result.container.querySelectorAll("[data-cell-cursor]").length,
      ).toBe(1);
    });

    // Explicitly clear focus -- a defocus action. `setFocus(null)` routes
    // to the kernel's `clear focus` op, which emits the
    // `Some(prev) -> None` focus-changed event the bridge consumes.
    await act(async () => {
      focusRef.setFocus?.(null);
    });

    await waitFor(() => {
      expect(
        result.container.querySelectorAll("[data-cell-cursor]").length,
      ).toBe(0);
    });
  });
});

/**
 * Click-to-cursor regression: a left-click on a cell must light the
 * cursor ring on that cell in the spatial-stack-mounted production
 * harness.
 *
 * The cell's `<FocusScope>` owns the click handler: it dispatches
 * `nav.focus({ args: { fq } })` (the single auditable focus-claim
 * command from `01KR7CDEFWWVF4WH0BCHE8Y21J`), whose webview-bus leg runs
 * `actions.focus(fq)` -- the snapshot-bearing kernel commit. The kernel
 * emits `focus-changed`, the `EntityFocusProvider` bridge mirrors
 * `next_fq` into the entity-focus store, and the grid derives the
 * cursor ring from the focused moniker.
 *
 * The assertion lifts the cursor-ring as the observable proxy for
 * entity-focus: a `data-cell-cursor` attribute appears on exactly the
 * clicked cell when the focused moniker matches. A regression anywhere
 * along click -> nav.focus -> kernel commit -> focus-changed -> store
 * renders zero rings instead.
 */
describe("GridView -- click-to-cursor regression (spatial path)", () => {
  let harness: SpatialHarness;

  beforeEach(() => {
    resetWebviewCommandBusForTest();
    harness = setupSpatialHarness({ defaultInvokeImpl });
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
    await flushSetup();

    // Park focus on the registered navbar scope first so the click has
    // something to change — that way "ring lights up on the clicked
    // cell" cannot be a false pass from focus already matching the
    // click target.
    expect(focusRef.setFocus).toBeTruthy();
    const navbarFq = registeredFq(harness, "ui:navbar");
    await act(async () => {
      focusRef.setFocus?.(navbarFq);
    });

    // No ring while focus is outside the grid.
    await waitFor(() => {
      expect(
        result.container.querySelectorAll("[data-cell-cursor]").length,
      ).toBe(0);
    });

    // Find the cell at (row=1, col=`status`) — the second row's status
    // column. The data-table renders the cell's `<FocusScope>` with
    // `data-segment="grid_cell:1:status"`; its click handler dispatches
    // `nav.focus` for the cell's FQM.
    const targetMoniker = "grid_cell:1:status";
    const focusableEl = result.container.querySelector(
      `[data-segment="${targetMoniker}"]`,
    ) as HTMLElement | null;
    expect(focusableEl).not.toBeNull();

    // Click inside the focusable's subtree — the event bubbles to the
    // `<FocusScope>`'s own click handler (which stops propagation so the
    // claim stays local to the leaf).
    await act(async () => {
      fireEvent.click(focusableEl!.firstElementChild ?? focusableEl!);
    });

    // The cursor ring (driven by focused moniker → grid-cell cursor →
    // `data-cell-cursor`) must now mark exactly the clicked cell.
    await waitFor(() => {
      const ringedCells =
        result.container.querySelectorAll("[data-cell-cursor]");
      expect(ringedCells.length).toBe(1);
      expect(
        (ringedCells[0] as HTMLElement).getAttribute("data-cell-cursor"),
      ).toBe("1:status");
    });
  });
});

/**
 * Architectural guard for "one decorator, one place" on a focused grid cell.
 *
 * The previous shape painted two simultaneous focus decorations on the
 * focused cell:
 *
 *   - `<FocusIndicator>` — bar to the left of the cell, rendered by the
 *     cell's `<FocusScope>` from its `useFocusClaim` React state.
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
 * other `<FocusScope>` in the app.
 *
 * This test asserts:
 *
 *   1. A focused grid cell renders exactly one `<FocusIndicator>` (its
 *      own — the focused leaf's), not multiple stacked decorators.
 *   2. No element rendered by the grid carries the removed cursor-ring
 *      classes (`ring-2 ring-primary ring-inset`). A regression that
 *      reintroduces the ring on `<TableCell>` would trip this guard.
 *
 * A single kernel commit (`setFocus(cellFq)`) drives BOTH focus signals:
 * the emitted `focus-changed` flips the cell's `useFocusClaim` listener
 * (mounting the bar) and flows through the entity-focus bridge (stamping
 * `data-cell-cursor`). The contract holds when both signals point at the
 * same cell.
 */
describe("GridView -- single-focus-visual on a focused cell", () => {
  let harness: SpatialHarness;

  beforeEach(() => {
    resetWebviewCommandBusForTest();
    harness = setupSpatialHarness({ defaultInvokeImpl });
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
    // by this point every grid cell has registered in the layer registry
    // and we can recover its `FullyQualifiedMoniker` from the harness.
    await flushSetup();

    // Commit focus to the targeted cell through the kernel. The emitted
    // `focus-changed` drives the cell's `useFocusClaim` callback (the
    // bar) AND the entity-focus store (the `data-cell-cursor` attribute)
    // — the production single-source-of-truth loop.
    const targetMoniker = gridCellMoniker(1, "status");
    const targetFq = registeredFq(harness, targetMoniker);
    expect(focusRef.setFocus).toBeTruthy();
    await act(async () => {
      focusRef.setFocus?.(targetFq);
    });

    const { container } = result;

    // 1. Exactly one `<FocusIndicator>` is rendered as a descendant of
    //    the focused cell's `<FocusScope>` — and that's the only one in
    //    the grid (no second decorator on a sibling element).
    await waitFor(() => {
      const indicators = container.querySelectorAll(
        "[data-testid='focus-indicator']",
      );
      expect(indicators.length).toBe(1);
    });
    const indicators = container.querySelectorAll(
      "[data-testid='focus-indicator']",
    );
    const focusedFocusable = container.querySelector(
      `[data-segment="${targetMoniker}"]`,
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
