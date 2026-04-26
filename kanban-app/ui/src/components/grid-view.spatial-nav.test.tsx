/**
 * Spatial-nav integration tests for `<GridView>`.
 *
 * Mounts the grid inside the production-shaped provider stack
 * (`<SpatialFocusProvider>` + `<FocusLayer name="window">`) so the conditional
 * `<GridSpatialZone>` lights up its `<FocusZone moniker={asMoniker("ui:grid")}>`
 * branch, and the per-cell `<GridCellFocusable>` lights up its `<Focusable>`
 * leaf branch. The Tauri `invoke` boundary is mocked at the module level so we
 * can inspect the `spatial_register_zone` and `spatial_register_focusable`
 * calls the components make on mount.
 *
 * Asserts the contract from kanban task `01KNQXZZ9VQBHFX091P0K4F4YC`:
 *
 *   1. The grid registers exactly one zone with moniker `"ui:grid"`.
 *   2. Every cell registers as a leaf focusable with the moniker
 *      `grid_cell:R:K` (where K is the column field name).
 *   3. Each cell focusable's `parentZone` is the zone key the grid registered.
 *   4. The `data-moniker="ui:grid"` element exists in the rendered DOM.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, act } from "@testing-library/react";

// ---------------------------------------------------------------------------
// Tauri API mocks -- must come before component imports.
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

// Stub the perspective container so the grid gets a stable activePerspective
// without dragging in heavier providers.
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
import { asLayerName } from "@/types/spatial";
import type { Entity, EntitySchema } from "@/types/kanban";

// ---------------------------------------------------------------------------
// Task schema -- two columns so we exercise the grid_cell:R:K shape with
// distinct column keys.
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

function seedTask(id: string, title: string, status: string): Entity {
  return {
    entity_type: "task",
    id,
    moniker: `task:${id}`,
    fields: { title, status },
  };
}

function threeTasks(): Entity[] {
  return [
    seedTask("t1", "Alpha", "todo"),
    seedTask("t2", "Beta", "doing"),
    seedTask("t3", "Gamma", "done"),
  ];
}

/**
 * Mount `GridView` inside the production-shaped provider stack with the
 * spatial-nav layer present so `<GridSpatialZone>` and `<GridCellFocusable>`
 * both light up.
 */
function GridHarness({ entities }: { entities: Record<string, Entity[]> }) {
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
                      <GridView
                        view={{
                          id: "v-spatial",
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

/** Collect every `spatial_register_zone` call payload. */
function registerZoneCalls(): Array<Record<string, unknown>> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "spatial_register_zone")
    .map((c) => c[1] as Record<string, unknown>);
}

/** Collect every `spatial_register_focusable` call payload. */
function registerFocusableCalls(): Array<Record<string, unknown>> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "spatial_register_focusable")
    .map((c) => c[1] as Record<string, unknown>);
}

describe("GridView (spatial-nav)", () => {
  beforeEach(() => {
    mockInvoke.mockClear();
    mockInvoke.mockImplementation(defaultInvokeImpl);
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it("registers exactly one ui:grid zone at the grid root", async () => {
    const entities = { task: threeTasks() };

    await act(async () => {
      render(<GridHarness entities={entities} />);
    });
    // Let mount-effects settle.
    await act(async () => {
      await new Promise((r) => setTimeout(r, 50));
    });

    const calls = registerZoneCalls();
    const gridZones = calls.filter((c) => c.moniker === "ui:grid");
    expect(gridZones.length).toBe(1);

    // Zone must be inside a layer (production layer key).
    expect(gridZones[0].layerKey).toBeTruthy();
  });

  it("emits a wrapper element with data-moniker='ui:grid'", async () => {
    const entities = { task: threeTasks() };

    let result!: ReturnType<typeof render>;
    await act(async () => {
      result = render(<GridHarness entities={entities} />);
    });
    await act(async () => {
      await new Promise((r) => setTimeout(r, 50));
    });

    const node = result.container.querySelector("[data-moniker='ui:grid']");
    expect(node).not.toBeNull();
  });

  it("registers each cell as a Focusable leaf with grid_cell:R:K moniker", async () => {
    const entities = { task: threeTasks() };

    await act(async () => {
      render(<GridHarness entities={entities} />);
    });
    await act(async () => {
      await new Promise((r) => setTimeout(r, 50));
    });

    const focusableCalls = registerFocusableCalls();
    const cellMonikers = focusableCalls
      .map((c) => c.moniker)
      .filter(
        (m): m is string => typeof m === "string" && m.startsWith("grid_cell:"),
      );

    // 3 rows × 2 columns = 6 cell focusables. Use a Set-based assertion so
    // the test is not sensitive to registration order — what matters is the
    // identity of each cell, not the sequence.
    expect(new Set(cellMonikers)).toEqual(
      new Set([
        "grid_cell:0:title",
        "grid_cell:0:status",
        "grid_cell:1:title",
        "grid_cell:1:status",
        "grid_cell:2:title",
        "grid_cell:2:status",
      ]),
    );
  });

  it("registers cell focusables with parentZone = the ui:grid zone key", async () => {
    const entities = { task: threeTasks() };

    await act(async () => {
      render(<GridHarness entities={entities} />);
    });
    await act(async () => {
      await new Promise((r) => setTimeout(r, 50));
    });

    const zoneCalls = registerZoneCalls();
    const gridZone = zoneCalls.find((c) => c.moniker === "ui:grid");
    expect(gridZone).toBeTruthy();
    const gridZoneKey = gridZone!.key;
    expect(gridZoneKey).toBeTruthy();

    const focusableCalls = registerFocusableCalls();
    const cellFocusables = focusableCalls.filter(
      (c) =>
        typeof c.moniker === "string" &&
        (c.moniker as string).startsWith("grid_cell:"),
    );
    expect(cellFocusables.length).toBeGreaterThan(0);

    // Every cell focusable must point its parentZone at the grid zone's
    // key — that anchors the spatial-nav graph so cross-cell beam search
    // stays inside `ui:grid`.
    for (const cell of cellFocusables) {
      expect(cell.parentZone).toBe(gridZoneKey);
    }
  });
});
