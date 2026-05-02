/**
 * Row-label focus + entity-scope dispatch contract for the data-table grid.
 *
 * Source of truth for kanban task `01KQJH8N2CVHQPZ6HCN662W1CA`:
 *
 *   1. The row's leftmost cell (`<RowSelector>` / `data-testid="row-selector"`)
 *      mounts a `<FocusScope moniker={asSegment("row_label:{di}")}>` leaf in
 *      the spatial-nav graph when the production provider stack is mounted —
 *      one leaf per row, with the data-row index baked into the segment so
 *      siblings under the row's `renderContainer={false}` `<FocusScope>`
 *      don't collide on identical composed FQMs.
 *   2. Driving focus to a row label leaf flips its `data-focused`
 *      attribute to `"true"` (the visible `<FocusIndicator>` paints
 *      around the row label cell). This pins the leaf-is-focusable
 *      property the global `nav.left` / `nav.right` commands rely on
 *      to land focus there from the first data cell.
 *   3. With focus on the row label leaf, dispatching `entity.archive`
 *      through the production `useDispatchCommand` hook reaches the
 *      backend with the row entity's moniker present in the
 *      `scopeChain` — proving the leaf inherits the row's entity
 *      command-scope frame and that entity-level commands target the
 *      whole row, not a per-cell field.
 *   4. Clicking the row selector still fires the legacy move-cursor
 *      `onCellClick` handler. The spatial path attaches the handler to
 *      a wrapper `<div>` INSIDE the leaf so it runs before
 *      `FocusScope.onClick` calls `e.stopPropagation()`. If that
 *      wrapper were dropped (or the handler attached to the outer
 *      `<TableCell>` instead), the leaf would swallow the click and
 *      cursor-move would never reach the consumer. (Truly omitting the
 *      spatial providers is impossible — `<EntityRow>` calls the
 *      strict `useFullyQualifiedMoniker()` which throws without a
 *      `<FocusLayer>` ancestor — so the test mounts the same
 *      `<SpatialFocusProvider>` + `<FocusLayer>` harness
 *      `data-table.test.tsx` uses and asserts the click reaches
 *      `onCellClick`.)
 *
 * Mock pattern follows `grid-view.cursor-ring.test.tsx`: a single
 * `mockInvoke` that records every IPC and a `listenHandlers` map that
 * lets the test drive `focus-changed` events directly. The
 * `defaultInvokeImpl` mirrors the kernel just enough that
 * `setFocus`-style dispatches flow through to the React-side
 * entity-focus store.
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
import { DataTable, type DataTableColumn } from "./data-table";
import { SchemaProvider } from "@/lib/schema-context";
import { EntityStoreProvider } from "@/lib/entity-store-context";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { FocusLayer } from "@/components/focus-layer";
import { FieldUpdateProvider } from "@/lib/field-update-context";
import { UIStateProvider } from "@/lib/ui-state-context";
import { TooltipProvider } from "@/components/ui/tooltip";
import { CommandBusyProvider, useDispatchCommand } from "@/lib/command-scope";
import { asSegment } from "@/types/spatial";
import type { Entity, EntitySchema, FieldDef } from "@/types/kanban";
import type { UseGridReturn } from "@/hooks/use-grid";

// ---------------------------------------------------------------------------
// Task schema -- two columns so the grid renders cells the leaf can
// neighbor in the spatial graph.
// ---------------------------------------------------------------------------

const TASK_SCHEMA = {
  entity: {
    name: "task",
    entity_type: "task",
    search_display_field: "title",
  },
  fields: [
    // `id` is required on FieldDef and is used as the React key for body
    // cells in `data-table.tsx` (`key={col.field.id}`). Omitting it makes
    // every cell key `undefined`, which logs a "unique key" warning under
    // `<tr>` even though the suite still passes — and warnings count as
    // failures under the project's test-skill rules.
    {
      id: "f-title",
      name: "title",
      type: "string",
      section: "header",
      display: "text",
    },
    {
      id: "f-status",
      name: "status",
      type: "string",
      section: "header",
      display: "text",
    },
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

/** Two tasks per the task spec — `task:a` and `task:b`. */
function twoTasks(): Entity[] {
  return [
    seedTask("a", "Alpha", "todo"),
    seedTask("b", "Beta", "doing"),
  ];
}

/**
 * Probe that captures the production `useDispatchCommand` callable into
 * a test-owned ref. The test invokes it from outside the React tree so
 * we exercise the same dispatch path the keybinding handler uses, not a
 * separate `mockInvoke` call constructed by hand.
 *
 * Lives at the grid level (not inside a row) and relies on the focused-
 * scope override in `useDispatchCommand` (it prefers `FocusedScopeContext`
 * over the tree scope when focus is set). When the row label leaf has
 * been focused via a kernel `focus-changed` event, the dispatch picks up
 * the leaf's full scope chain at click time.
 */
interface DispatchRef {
  dispatch: ((cmd: string) => Promise<unknown>) | null;
}

function DispatchProbe({ dispatchRef }: { dispatchRef: DispatchRef }) {
  const dispatch = useDispatchCommand();
  useEffect(() => {
    dispatchRef.dispatch = dispatch;
    return () => {
      dispatchRef.dispatch = null;
    };
  }, [dispatch, dispatchRef]);
  return null;
}

/**
 * Mount `GridView` inside the production-shaped provider stack. Mirrors
 * `grid-view.cursor-ring.test.tsx`'s `GridHarness` exactly — the
 * providers are the same ones that sit under `RustEngineContainer` in
 * `App.tsx`.
 */
function GridHarness({
  entities,
  dispatchRef,
}: {
  entities: Record<string, Entity[]>;
  dispatchRef?: DispatchRef;
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
                      {dispatchRef && (
                        <DispatchProbe dispatchRef={dispatchRef} />
                      )}
                      <GridView
                        view={{
                          id: "v-row-label",
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

/**
 * Tracks the FQM → segment mapping so the focus-changed simulator can
 * carry both fields when emitting events. Same shape as the simulator
 * in `grid-view.cursor-ring.test.tsx`.
 */
const fqToSegment = new Map<string, string>();
const currentFocusKey: { key: string | null } = { key: null };

/**
 * Emit a `focus-changed` event onto the captured listener so the
 * spatial-focus React bridge sees the kernel's update. Queued via
 * `queueMicrotask` so the timing matches real Tauri events — a
 * synchronous emit would hide regressions where `setFocus` writes the
 * store synchronously.
 */
function emitFocusChanged(nextFq: string | null, nextSegment: string | null) {
  const prev = currentFocusKey.key;
  currentFocusKey.key = nextFq;
  queueMicrotask(() => {
    const handler = listenHandlers["focus-changed"];
    if (handler) {
      handler({
        payload: {
          window_label: "main",
          prev_fq: prev,
          next_fq: nextFq,
          next_segment: nextSegment,
        },
      });
    }
  });
}

/** Default invoke responses for the mount-time IPCs the providers fire. */
async function defaultInvokeImpl(
  cmd: string,
  args?: unknown,
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
  if (cmd === "spatial_register_scope" || cmd === "spatial_register_zone") {
    const a = (args ?? {}) as { fq?: string; segment?: string };
    if (a.fq && a.segment) fqToSegment.set(a.fq, a.segment);
    return undefined;
  }
  if (cmd === "spatial_unregister_scope") {
    const a = (args ?? {}) as { fq?: string };
    if (a.fq) fqToSegment.delete(a.fq);
    return undefined;
  }
  if (cmd === "spatial_focus") {
    const a = (args ?? {}) as { fq?: string };
    const fq = a.fq ?? null;
    const segment = fq ? fqToSegment.get(fq) ?? null : null;
    if (fq) emitFocusChanged(fq, segment);
    return undefined;
  }
  if (cmd === "spatial_clear_focus") {
    emitFocusChanged(null, null);
    return undefined;
  }
  return undefined;
}

// ---------------------------------------------------------------------------
// Tests — spatial-stack path
// ---------------------------------------------------------------------------

describe("RowSelector — row-label focus leaf (spatial path)", () => {
  beforeEach(() => {
    mockInvoke.mockClear();
    fqToSegment.clear();
    currentFocusKey.key = null;
    for (const key of Object.keys(listenHandlers)) {
      delete listenHandlers[key];
    }
    mockInvoke.mockImplementation(defaultInvokeImpl);
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it("registers a `row_label` FocusScope leaf for every data row", async () => {
    const entities = { task: twoTasks() };

    let result!: ReturnType<typeof render>;
    await act(async () => {
      result = render(<GridHarness entities={entities} />);
    });
    await act(async () => {
      await new Promise((r) => setTimeout(r, 100));
    });

    // Sanity check — both rows must actually be in the DOM. The
    // virtualizer's natural height + overscan in the browser-mode test
    // harness mounts every row when the viewport isn't height-constrained
    // (see `data-table.virtualized.test.tsx` for the same observation).
    const rowSelectors = result.container.querySelectorAll(
      "[data-testid='row-selector']",
    );
    expect(rowSelectors.length).toBe(2);

    // Every visible row registers a `row_label:{di}` leaf via
    // `spatial_register_scope`. With two tasks, that's exactly two
    // registrations — no more (each row mounts one leaf), no fewer
    // (the spatial path was taken in both rows). The `{di}` suffix is
    // load-bearing: the row's outer `<FocusScope renderContainer={false}>`
    // doesn't push a new FQM context, so siblings under it share a
    // parent FQM and would collide on identical composed FQMs without
    // the per-row index in the segment (same disambiguation convention
    // as `grid_cell:{di}:{colKey}`).
    const registerCalls = mockInvoke.mock.calls.filter(
      (c) => c[0] === "spatial_register_scope",
    );
    const rowLabelRegs = registerCalls.filter((c) =>
      (c[1] as { segment?: string })?.segment?.startsWith("row_label:"),
    );
    expect(rowLabelRegs.length).toBe(2);

    const rowLabelSegments = rowLabelRegs.map(
      (c) => (c[1] as { segment: string }).segment,
    );
    expect(rowLabelSegments).toContain("row_label:0");
    expect(rowLabelSegments).toContain("row_label:1");

    // Each registration carries a distinct FQM (one per row). That's
    // what the spatial kernel uses to differentiate the two leaves
    // when arrow-key beam-search picks a target.
    const rowLabelFqs = rowLabelRegs.map(
      (c) => (c[1] as { fq: string }).fq,
    );
    expect(new Set(rowLabelFqs).size).toBe(2);
  });

  it("driving focus to a row label leaf flips its `data-focused` attribute", async () => {
    const entities = { task: twoTasks() };

    let result!: ReturnType<typeof render>;
    await act(async () => {
      result = render(<GridHarness entities={entities} />);
    });
    await act(async () => {
      await new Promise((r) => setTimeout(r, 100));
    });

    // Pick the row 0 label leaf. Drive focus to its FQM through the
    // kernel simulator.
    const row0LabelEntry = Array.from(fqToSegment.entries()).find(
      ([, seg]) => seg === "row_label:0",
    );
    expect(row0LabelEntry).toBeTruthy();
    const targetFq = row0LabelEntry![0];

    await act(async () => {
      emitFocusChanged(targetFq, "row_label:0");
    });
    await act(async () => {
      await new Promise((r) => setTimeout(r, 0));
    });

    // The focused leaf carries `data-focused="true"`. The two
    // selectors render side-by-side; the focused one is identified by
    // its FQM (not by row index — the assertion is on the FQM
    // directly to avoid a fragile coupling).
    const focusedLeaf = result.container.querySelector(
      `[data-moniker="${targetFq}"]`,
    ) as HTMLElement | null;
    expect(focusedLeaf).not.toBeNull();
    expect(focusedLeaf!.getAttribute("data-focused")).toBe("true");
    expect(focusedLeaf!.getAttribute("data-segment")).toBe("row_label:0");

    // The leaf is wrapped by the row selector cell — that's where the
    // visible focus indicator paints around (the cell stays a `<td>`
    // and the leaf div sits inside it).
    const rowSelector = focusedLeaf!.closest(
      "[data-testid='row-selector']",
    );
    expect(rowSelector).not.toBeNull();
    expect(rowSelector!.tagName).toBe("TD");
  });

  it("dispatching `entity.archive` from the row label leaf carries the row entity in scopeChain", async () => {
    const dispatchRef: DispatchRef = { dispatch: null };
    const entities = { task: twoTasks() };

    await act(async () => {
      render(
        <GridHarness entities={entities} dispatchRef={dispatchRef} />,
      );
    });
    await act(async () => {
      await new Promise((r) => setTimeout(r, 50));
    });

    // Pick the row 0 label leaf — that's the row for `task:a` (the
    // first seeded task).
    const row0LabelEntry = Array.from(fqToSegment.entries()).find(
      ([, seg]) => seg === "row_label:0",
    );
    expect(row0LabelEntry).toBeTruthy();
    const row0LabelFq = row0LabelEntry![0];

    // Drive focus directly to the row label leaf's FQM. The
    // EntityFocusProvider bridge mirrors `focus-changed.next_fq` into
    // the entity-focus store, which `useDispatchCommand` reads through
    // `FocusedScopeContext`.
    await act(async () => {
      emitFocusChanged(row0LabelFq, "row_label:0");
    });
    await act(async () => {
      await new Promise((r) => setTimeout(r, 0));
    });

    mockInvoke.mockClear();

    // Dispatch `entity.archive` through the production hook the
    // keybinding handler uses (`useDispatchCommand`).
    expect(dispatchRef.dispatch).toBeTruthy();
    await act(async () => {
      await dispatchRef.dispatch?.("entity.archive");
    });

    const dispatchCalls = mockInvoke.mock.calls.filter(
      (c) => c[0] === "dispatch_command",
    );
    expect(dispatchCalls.length).toBe(1);
    const dispatchArgs = dispatchCalls[0][1] as {
      cmd?: string;
      scopeChain?: string[];
    };
    expect(dispatchArgs.cmd).toBe("entity.archive");
    // The scope chain must include `task:a` — that's the row's entity
    // moniker, contributed by the row's outer
    // `<FocusScope moniker={asSegment("task:a")} renderContainer={false}>`
    // frame the row label leaf is mounted inside (the wrapper pushes
    // `task:a` into the React command-scope chain even though it
    // doesn't itself register with the kernel). This is the
    // load-bearing assertion: it proves entity-level commands resolve
    // against the whole row entity, not a per-cell field.
    expect(dispatchArgs.scopeChain).toBeTruthy();
    expect(dispatchArgs.scopeChain).toContain("task:a");
    // The `row_label:0` segment itself is also in the chain (the leaf
    // pushes its own moniker frame). We don't pin exact ordering of
    // higher frames (window/grid/view) — those are independent
    // contracts.
    expect(dispatchArgs.scopeChain).toContain("row_label:0");
  });
});

// ---------------------------------------------------------------------------
// Tests — bare-harness path (no GridView providers, just SpatialFocus + Layer)
// ---------------------------------------------------------------------------

const FALLBACK_FIELD_DEFS: FieldDef[] = [
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

const FALLBACK_COLUMNS: DataTableColumn[] = FALLBACK_FIELD_DEFS.map((f) => ({
  field: f,
}));

const FALLBACK_ENTITIES: Entity[] = [
  {
    entity_type: "task",
    id: "t1",
    moniker: "task:t1",
    fields: { title: "Task 1", status: "todo" },
  },
];

/** Minimal grid state for the fallback harness — no editing, cursor at 0,0. */
function makeFallbackGrid(): UseGridReturn {
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

describe("RowSelector — click-to-cursor regression", () => {
  beforeEach(() => {
    mockInvoke.mockClear();
    mockInvoke.mockImplementation(async () => undefined);
    fqToSegment.clear();
    currentFocusKey.key = null;
    for (const key of Object.keys(listenHandlers)) {
      delete listenHandlers[key];
    }
    // jsdom doesn't implement `scrollIntoView`; the cursor-row scroll
    // effect would otherwise throw on mount.
    Element.prototype.scrollIntoView = vi.fn();
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it("clicking the row selector still fires the legacy move-cursor onCellClick handler", async () => {
    // Pins the click-handler split documented on `GridCellFocusable`
    // and mirrored in `RowSelector`'s spatial path: the inner click
    // wrapper's `onClick` runs BEFORE `FocusScope.onClick` calls
    // `e.stopPropagation()`. If the wrapper were dropped (or the
    // legacy handler attached to the outer `<TableCell>` instead),
    // the leaf would swallow the click and the cursor-move would
    // never reach the consumer's `onCellClick`. This test pins both
    // halves of that contract by mounting the spatial provider stack
    // and clicking the leaf's inner div.
    //
    // (We deliberately mount with `<SpatialFocusProvider>` +
    // `<FocusLayer>` rather than truly bare. `<EntityRow>` calls the
    // strict `useFullyQualifiedMoniker()` which throws without an FQM
    // ancestor, so a "no spatial providers" mount can't render
    // `<DataTable>` at all — the spatial path is the only path
    // production exercises.)
    const handleClick = vi.fn();
    let result!: ReturnType<typeof render>;
    await act(async () => {
      result = render(
        <SpatialFocusProvider>
          <FocusLayer name={asSegment("window")}>
            <EntityFocusProvider>
              <DataTable
                columns={FALLBACK_COLUMNS}
                rows={FALLBACK_ENTITIES}
                grid={makeFallbackGrid()}
                showRowSelector={true}
                onCellClick={handleClick}
              />
            </EntityFocusProvider>
          </FocusLayer>
        </SpatialFocusProvider>,
      );
    });

    const selector = result.container.querySelector(
      "[data-testid='row-selector']",
    ) as HTMLElement | null;
    expect(selector).not.toBeNull();
    expect(selector!.tagName).toBe("TD");
    expect(selector!.textContent).toBe("1");

    // Click the inner click wrapper — the spatial path mounts
    // `<FocusScope>` inside the cell, whose root `<div>` wraps an
    // inner `<div>` carrying the `onClick`. Mirrors the click-target
    // lookup pattern in `grid-view.cursor-ring.test.tsx::clicking a
    // cell sets entity-focus and lights the cursor ring on that
    // cell` — the outer `<td>` no longer carries the click handler in
    // the spatial branch.
    const focusableEl = selector!.querySelector(
      "[data-segment^='row_label:']",
    ) as HTMLElement | null;
    expect(focusableEl).not.toBeNull();
    const innerClickTarget =
      (focusableEl!.firstElementChild as HTMLElement | null) ?? focusableEl!;
    await act(async () => {
      fireEvent.click(innerClickTarget);
    });
    // The click reached `handleCellClick(di, col)` → `onCellClick(di, col)`.
    expect(handleClick).toHaveBeenCalled();
  });
});
