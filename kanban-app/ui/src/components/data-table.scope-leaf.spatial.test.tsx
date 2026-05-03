/**
 * Browser-mode regression test pinning the **scope-is-leaf** invariant
 * for the data-table row.
 *
 * Source of truth for kanban task `01KQM6VWQTK6KCQMQNKS0BX5V3` (audit
 * remaining scope-not-leaf offenders surfaced by path-prefix
 * enforcement). Mirrors `entity-card.scope-leaf.spatial.test.tsx` for
 * the table-row equivalent: the row's outer focus primitive must be a
 * `<FocusZone renderContainer={false}>`, never a `<FocusScope>`, so
 * the per-cell `<FocusScope>` leaves inside the row do not register
 * as path-prefix descendants of an outer Scope.
 *
 * The kernel's three peers are:
 *
 *   - `<FocusLayer>` — modal boundary
 *   - `<FocusZone>` — navigable container, can have children (other zones
 *     or scopes)
 *   - `<FocusScope>` — leaf in the spatial graph
 *
 * The previous row-as-`<FocusScope renderContainer={false}>` shape did
 * not register a rect with the kernel (no DOM node — `<tr>` is
 * structural), but the React subtree composed cell `<FocusScope>`
 * leaves whose FQMs were path-descendants of the row scope FQM as soon
 * as the row's wrapper started publishing its FQM through
 * `FullyQualifiedMonikerContext.Provider`. Promoting the row to a
 * `<FocusZone renderContainer={false}>` keeps the entity moniker frame
 * in the React command-scope chain (so right-click resolves
 * entity-level commands against the row), while the Zone-vs-Scope
 * change lifts the scope-is-leaf restriction on cell descendants.
 *
 * This file pins:
 *
 *   1. The row's entity moniker (`task:{id}`) NEVER appears as a
 *      `spatial_register_scope` payload — the row uses a
 *      `<FocusZone renderContainer={false}>` which does not register
 *      with the kernel at all (no rect), but more importantly, the
 *      outer wrapper is not a Scope.
 *   2. Per-cell `grid_cell:{di}:{colKey}` `<FocusScope>` leaves DO
 *      register, and their composed FQM is a path-descendant of the
 *      row's composed FQM. This is the load-bearing assertion: it
 *      proves the row's outer wrapper publishes its FQM through
 *      `FullyQualifiedMonikerContext.Provider` (a Zone behavior;
 *      `<FocusScope renderContainer={false}>` does not push that
 *      provider).
 *   3. The row label leaf (`row_label:{di}`) registers as a scope and
 *      its `parentZone` resolves to the row's FQM — proving the row
 *      Zone publishes `FocusZoneContext.Provider` so descendants'
 *      `useParentZoneFq()` lands on the row entity, not the
 *      surrounding `ui:grid` zone.
 *
 * Mock pattern matches `data-table.row-label-focus.spatial.test.tsx`
 * exactly — same `mockInvoke`, same `defaultInvokeImpl`, same
 * `GridHarness` shape — so the assertions here ride on top of the
 * already-pinned production-stack invariants.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, act } from "@testing-library/react";

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

// Mock the perspective container so the grid gets a stable
// activePerspective without dragging in the heavier
// PerspectivesContainer (mirrors the row-label-focus test).
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
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { FocusLayer } from "@/components/focus-layer";
import { FieldUpdateProvider } from "@/lib/field-update-context";
import { UIStateProvider } from "@/lib/ui-state-context";
import { TooltipProvider } from "@/components/ui/tooltip";
import { CommandBusyProvider } from "@/lib/command-scope";
import { asSegment } from "@/types/spatial";
import type { Entity, EntitySchema } from "@/types/kanban";

// ---------------------------------------------------------------------------
// Task schema -- two columns so the grid renders cells the leaf can
// neighbor in the spatial graph. Mirrors the row-label-focus test
// schema exactly.
// ---------------------------------------------------------------------------

const TASK_SCHEMA = {
  entity: {
    name: "task",
    entity_type: "task",
    search_display_field: "title",
  },
  fields: [
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

/** Two tasks for the spatial-nav assertions. */
function twoTasks(): Entity[] {
  return [
    seedTask("a", "Alpha", "todo"),
    seedTask("b", "Beta", "doing"),
  ];
}

/**
 * Tracks the FQM → segment mapping so an unsolicited focus emit can
 * carry both fields. The default invoke implementation populates this
 * on `spatial_register_*` and the helper flushes it on
 * `spatial_unregister_scope`.
 */
const fqToSegment = new Map<string, string>();

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
  return undefined;
}

/**
 * Mount `GridView` inside the production-shaped provider stack. Same
 * harness as `data-table.row-label-focus.spatial.test.tsx`.
 */
function GridHarness({ entities }: { entities: Record<string, Entity[]> }) {
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
                      <GridView
                        view={{
                          id: "v-scope-leaf",
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

/** Calls collected by `mockInvoke` for the named command. */
function callsFor(cmd: string): Array<Record<string, unknown>> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === cmd)
    .map((c) => (c[1] ?? {}) as Record<string, unknown>);
}

function registerScopeArgs(): Array<Record<string, unknown>> {
  return callsFor("spatial_register_scope");
}

function registerZoneArgs(): Array<Record<string, unknown>> {
  return callsFor("spatial_register_zone");
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("DataTable row — scope-is-leaf invariant", () => {
  beforeEach(() => {
    mockInvoke.mockClear();
    fqToSegment.clear();
    for (const key of Object.keys(listenHandlers)) {
      delete listenHandlers[key];
    }
    mockInvoke.mockImplementation(defaultInvokeImpl);
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it("the row's `task:{id}` segment is NEVER passed to spatial_register_scope", async () => {
    // The row mounts a `<FocusZone renderContainer={false}>` whose
    // `moniker` is the row entity's segment (`task:a`, `task:b`, …).
    // `renderContainer={false}` means the zone does NOT call
    // `spatial_register_zone` either (no DOM rect to register), but the
    // critical scope-is-leaf assertion is the negative one: the row
    // entity's segment must NEVER be registered as a Scope, because
    // doing so would make every per-cell `<FocusScope>` leaf inside the
    // row a path-prefix scope-not-leaf offender.
    const entities = { task: twoTasks() };

    let result!: ReturnType<typeof render>;
    await act(async () => {
      result = render(<GridHarness entities={entities} />);
    });
    await act(async () => {
      await new Promise((r) => setTimeout(r, 100));
    });

    const rowEntitySegments = new Set(["task:a", "task:b"]);
    const rowAsScope = registerScopeArgs().find((a) =>
      typeof a.segment === "string" &&
      rowEntitySegments.has(a.segment as string),
    );
    expect(
      rowAsScope,
      "the data-table row's task:{id} segment must NEVER be registered as a Scope (scope-is-leaf invariant)",
    ).toBeUndefined();

    // Belt-and-braces: the row Zone uses `renderContainer={false}`, so
    // it should not register as a Zone either (no DOM rect). This pins
    // the renderContainer={false} contract — promoting the row to a
    // Zone is purely a context-publishing change.
    const rowAsZone = registerZoneArgs().find((a) =>
      typeof a.segment === "string" &&
      rowEntitySegments.has(a.segment as string),
    );
    expect(
      rowAsZone,
      "the data-table row uses renderContainer={false} — it must NOT register a rect with the kernel either",
    ).toBeUndefined();

    result.unmount();
  });

  it("per-cell `grid_cell:{di}:{colKey}` leaves nest under the row's FQM", async () => {
    // The load-bearing test. With the row promoted to a `<FocusZone
    // renderContainer={false}>`, the wrapper publishes its composed FQM
    // through `FullyQualifiedMonikerContext.Provider` even though it
    // renders no DOM. Cell `<FocusScope>` leaves inside the row read
    // that FQM as their parent and compose their own FQM as
    // `<rowFq>/grid_cell:{di}:{colKey}`. This assertion proves the row
    // wrapper IS a Zone (which pushes the FQM) and not a Scope with
    // `renderContainer={false}` (which would NOT push it — see
    // `<FocusScope>` short-circuit).
    const entities = { task: twoTasks() };

    let result!: ReturnType<typeof render>;
    await act(async () => {
      result = render(<GridHarness entities={entities} />);
    });
    await act(async () => {
      await new Promise((r) => setTimeout(r, 100));
    });

    // Find the cells for row 0 (di=0). The `grid_cell:{di}:{colKey}`
    // segment encodes the data-row index and column key — see
    // `gridCellMoniker` in data-table.tsx.
    const row0CellRegs = registerScopeArgs().filter((a) => {
      const seg = a.segment;
      return typeof seg === "string" && seg.startsWith("grid_cell:0:");
    });
    expect(
      row0CellRegs.length,
      "row 0 should register one cell leaf per column (title + status)",
    ).toBeGreaterThanOrEqual(2);

    // Every row-0 cell FQM must contain `/task:a/` somewhere — that's
    // the row entity's segment. If the row wrapper failed to publish
    // its FQM (i.e. it was still a `<FocusScope renderContainer={false}>`),
    // cell FQMs would compose directly under the surrounding `ui:grid`
    // zone and the `task:a` segment would be missing from the path.
    for (const reg of row0CellRegs) {
      const fq = reg.fq as string;
      expect(
        fq,
        `row-0 cell FQM must include the row entity segment "task:a": ${fq}`,
      ).toMatch(/\/task:a\//);
    }

    result.unmount();
  });

  it("the row_label leaf's parentZone resolves to the row's FQM", async () => {
    // `<RowSelector>` renders a `<FocusScope moniker="row_label:{di}">`
    // leaf inside the cell. When the row is a `<FocusZone>` (even with
    // `renderContainer={false}`), it pushes `FocusZoneContext.Provider`
    // with its own FQM, so the row label leaf's `useParentZoneFq()`
    // resolves to the row entity's FQM — not the surrounding `ui:grid`
    // zone. This pins the FocusZoneContext provider in the
    // renderContainer={false} short-circuit.
    const entities = { task: twoTasks() };

    let result!: ReturnType<typeof render>;
    await act(async () => {
      result = render(<GridHarness entities={entities} />);
    });
    await act(async () => {
      await new Promise((r) => setTimeout(r, 100));
    });

    const row0LabelReg = registerScopeArgs().find(
      (a) => a.segment === "row_label:0",
    );
    expect(
      row0LabelReg,
      "row 0 must register a row_label:0 leaf",
    ).toBeDefined();

    const parentZone = row0LabelReg!.parentZone as string | null;
    expect(
      parentZone,
      "row_label:0 must have a parentZone (the row Zone, not null)",
    ).toBeTruthy();
    // The row label's parent zone segment must be the row entity's
    // moniker — `task:a` for row 0. We resolve the FQM back to the
    // segment via the registration map populated in
    // `defaultInvokeImpl`.
    const parentSegment = fqToSegment.get(parentZone!);
    // The row Zone uses `renderContainer={false}` and so does NOT call
    // `spatial_register_zone`, so it does NOT appear in `fqToSegment`
    // (the map is populated on register, and there's no register call
    // for the row). Pin the FQM-tail shape directly: the row's FQM ends
    // in `/task:a` for row 0 because that's the entity moniker the
    // wrapper composes.
    expect(
      parentSegment,
      "the row Zone uses renderContainer={false} and does not register, so the FQM is not in the map",
    ).toBeUndefined();
    expect(
      parentZone!.endsWith("/task:a"),
      `row_label:0 parentZone must point at the row entity (composed FQM ending in /task:a): ${parentZone}`,
    ).toBe(true);

    result.unmount();
  });
});
