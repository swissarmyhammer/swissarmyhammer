/**
 * Regression test for the GridView perspective-filter wiring.
 *
 * Before 01KP3ERHEDP86C2JYYR7NM1593's review fix, GridView read entities
 * directly from `useEntityStore().getEntities(entityType)` and ignored the
 * per-window `UIState.filtered_task_ids` slot — so switching perspectives
 * in a grid showed all tasks unfiltered. The fix routes the entity list
 * through the shared `useFilteredEntities` selector (see
 * `lib/use-filtered-tasks.ts`), which intersects `task` entities with the
 * filtered id list while passing non-task entity types through unchanged.
 *
 * These tests pin the new behavior by:
 *   1. Mocking `useUIState` to drive `filtered_task_ids`.
 *   2. Mocking `useEntityStore.getEntities` to return a known full task list.
 *   3. Asserting that GridView's rendered rows (via the mocked DataTable
 *      which captures the resolved entities) reflect the filter.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
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

// ---------------------------------------------------------------------------
// Mock context dependencies.
// ---------------------------------------------------------------------------

const mockActivePerspective = vi.hoisted(() =>
  vi.fn(() => ({
    activePerspective: null as { id: string; name: string } | null,
    applySort: (entities: unknown[]) => entities,
    groupField: undefined as string | undefined,
  })),
);

vi.mock("@/components/perspective-container", () => ({
  useActivePerspective: () => mockActivePerspective(),
}));

vi.mock("@/lib/schema-context", () => ({
  useSchema: () => ({
    getSchema: () => ({
      entity: { entity_type: "task", search_display_field: "title" },
      fields: [
        { name: "title", type: "string", section: "header", display: "text" },
      ],
    }),
    schemas: {},
    loading: false,
  }),
  useSchemaOptional: () => ({
    getSchema: () => undefined,
    getFieldDef: () => undefined,
  }),
}));

// Mutable entity-store mock — tests set the full unfiltered list and let
// GridView's `useFilteredEntities` apply the per-window filter on top.
const mockGetEntities = vi.hoisted(() =>
  vi.fn<(entityType: string) => unknown[]>(() => []),
);
vi.mock("@/lib/entity-store-context", () => ({
  useEntityStore: () => ({
    getEntities: (entityType: string) => mockGetEntities(entityType),
    getEntity: () => undefined,
    subscribe: () => () => {},
  }),
  useFieldValue: () => undefined,
}));

// Mock useUIState directly — `useFilteredEntities` reads
// `uiState.windows[label].filtered_task_ids` from this.
const mockUIState = vi.hoisted(() =>
  vi.fn(() => ({
    keymap_mode: "cua",
    scope_chain: [],
    open_boards: [],
    has_clipboard: false,
    clipboard_entity_type: null,
    windows: {} as Record<
      string,
      { active_perspective_id?: string; filtered_task_ids?: string[] }
    >,
    recent_boards: [],
  })),
);
vi.mock("@/lib/ui-state-context", () => ({
  useUIState: () => mockUIState(),
}));

vi.mock("@/lib/entity-focus-context", () => {
  const actions = {
    setFocus: vi.fn(),
    registerScope: vi.fn(),
    unregisterScope: vi.fn(),
    getScope: vi.fn(),
  };
  return {
    useEntityFocus: () => ({
      focusedFq: null,
      setFocus: vi.fn(),
      registerScope: vi.fn(),
      unregisterScope: vi.fn(),
      getScope: vi.fn(),
    }),
    useFocusActions: () => actions,
    useOptionalFocusActions: () => actions,
    useEntityScopeRegistration: () => {},
    useFocusedMoniker: () => null,
    useFocusedMonikerRef: () => ({ current: null }),
    useIsFocused: () => false,
    useIsDirectFocus: () => false,
    useOptionalIsDirectFocus: () => false,
    useOptionalFocusStore: () => null,
    useFocusBySegmentPath: () => vi.fn(),
    useFocusedFq: () => null,
    useFocusedSegmentMoniker: () => null,
  };
});

vi.mock("@/hooks/use-grid", () => ({
  useGrid: () => ({
    cursor: { row: 0, col: 0 },
    mode: "normal",
    setCursor: vi.fn(),
    moveCursor: vi.fn(),
    startEdit: vi.fn(),
    endEdit: vi.fn(),
    toggleVisual: vi.fn(),
    clearVisual: vi.fn(),
    getSelectedRange: () => null,
  }),
}));

// Capture the `rows` prop passed to DataTable so we can assert on the
// resolved entity list (the value reaching the rendering layer, after
// `applySort` in `useGridData`).
type CapturedData = { rows?: unknown[] } | null;
let lastDataTableProps: CapturedData = null;
vi.mock("@/components/data-table", () => ({
  DataTable: (props: { rows?: unknown[] }) => {
    lastDataTableProps = props;
    return null;
  },
}));

// ---------------------------------------------------------------------------
// Import after mocks.
// ---------------------------------------------------------------------------

import { GridView } from "./grid-view";
import { TooltipProvider } from "@/components/ui/tooltip";

/** Make a minimal entity record. */
function ent(id: string) {
  return { id, entity_type: "task", moniker: `task:${id}`, fields: {} };
}

describe("GridView — perspective filter (filtered_task_ids)", () => {
  beforeEach(() => {
    lastDataTableProps = null;
    mockActivePerspective.mockReturnValue({
      activePerspective: null,
      applySort: (entities: unknown[]) => entities,
      groupField: undefined,
    });
    mockUIState.mockReturnValue({
      keymap_mode: "cua",
      scope_chain: [],
      open_boards: [],
      has_clipboard: false,
      clipboard_entity_type: null,
      windows: {},
      recent_boards: [],
    });
  });

  it("intersects task rows with filtered_task_ids when the slot is non-empty", async () => {
    mockGetEntities.mockImplementation(() => [
      ent("t1"),
      ent("t2"),
      ent("t3"),
      ent("t4"),
    ]);
    mockUIState.mockReturnValue({
      keymap_mode: "cua",
      scope_chain: [],
      open_boards: [],
      has_clipboard: false,
      clipboard_entity_type: null,
      windows: {
        main: { active_perspective_id: "p1", filtered_task_ids: ["t1", "t3"] },
      },
      recent_boards: [],
    });

    await act(async () => {
      render(
        <TooltipProvider>
          <GridView
            view={{
              id: "v-task",
              name: "Task Grid",
              kind: "grid",
              entity_type: "task",
            }}
          />
        </TooltipProvider>,
      );
    });

    const data = (lastDataTableProps?.rows ?? []) as Array<{ id: string }>;
    expect(data.map((e) => e.id).sort()).toEqual(["t1", "t3"]);
  });

  it("renders zero rows when filtered_task_ids is an empty list (filter matched nothing)", async () => {
    mockGetEntities.mockImplementation(() => [ent("t1"), ent("t2")]);
    mockUIState.mockReturnValue({
      keymap_mode: "cua",
      scope_chain: [],
      open_boards: [],
      has_clipboard: false,
      clipboard_entity_type: null,
      windows: {
        main: { active_perspective_id: "p1", filtered_task_ids: [] },
      },
      recent_boards: [],
    });

    await act(async () => {
      render(
        <TooltipProvider>
          <GridView
            view={{
              id: "v-task",
              name: "Task Grid",
              kind: "grid",
              entity_type: "task",
            }}
          />
        </TooltipProvider>,
      );
    });

    // With entities=[] after intersection, the empty-state path kicks in
    // and DataTable is never rendered — that's also a valid "zero rows"
    // signal. Either path satisfies the contract; we assert that no row
    // for t1/t2 leaked through.
    const data = (lastDataTableProps?.rows ?? []) as Array<{ id: string }>;
    expect(data.map((e) => e.id)).not.toContain("t1");
    expect(data.map((e) => e.id)).not.toContain("t2");
  });

  it("passes all rows through unchanged when filtered_task_ids is undefined (no switch fired yet)", async () => {
    mockGetEntities.mockImplementation(() => [ent("t1"), ent("t2"), ent("t3")]);
    // No `filtered_task_ids` key on the window snapshot → tri-state
    // `undefined` → no filter active.
    mockUIState.mockReturnValue({
      keymap_mode: "cua",
      scope_chain: [],
      open_boards: [],
      has_clipboard: false,
      clipboard_entity_type: null,
      windows: { main: { active_perspective_id: "" } },
      recent_boards: [],
    });

    await act(async () => {
      render(
        <TooltipProvider>
          <GridView
            view={{
              id: "v-task",
              name: "Task Grid",
              kind: "grid",
              entity_type: "task",
            }}
          />
        </TooltipProvider>,
      );
    });

    const data = (lastDataTableProps?.rows ?? []) as Array<{ id: string }>;
    expect(data.map((e) => e.id).sort()).toEqual(["t1", "t2", "t3"]);
  });

  it("does NOT apply the task filter to non-task entity types (tag grid is unaffected)", async () => {
    // Non-task entities pass through `useFilteredEntities` unchanged even
    // when `filtered_task_ids` is restrictive — the perspective DSL
    // operates over tasks, not tags.
    mockGetEntities.mockImplementation(() => [
      { id: "tag1", entity_type: "tag", moniker: "tag:tag1", fields: {} },
      { id: "tag2", entity_type: "tag", moniker: "tag:tag2", fields: {} },
    ]);
    mockUIState.mockReturnValue({
      keymap_mode: "cua",
      scope_chain: [],
      open_boards: [],
      has_clipboard: false,
      clipboard_entity_type: null,
      windows: {
        main: { active_perspective_id: "p1", filtered_task_ids: [] },
      },
      recent_boards: [],
    });

    await act(async () => {
      render(
        <TooltipProvider>
          <GridView
            view={{
              id: "v-tag",
              name: "Tags Grid",
              kind: "grid",
              entity_type: "tag",
            }}
          />
        </TooltipProvider>,
      );
    });

    const data = (lastDataTableProps?.rows ?? []) as Array<{ id: string }>;
    expect(data.map((e) => e.id).sort()).toEqual(["tag1", "tag2"]);
  });
});
