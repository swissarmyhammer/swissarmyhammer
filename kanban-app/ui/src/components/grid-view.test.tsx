import { describe, it, vi } from "vitest";
import { render, act } from "@testing-library/react";

// ---------------------------------------------------------------------------
// Tauri API mocks — must come before component imports.
// ---------------------------------------------------------------------------

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(() => Promise.resolve()),
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
// Mock dependencies that GridView uses.
// ---------------------------------------------------------------------------

const mockActivePerspective = vi.hoisted(() =>
  vi.fn(() => ({
    activePerspective: null as {
      id: string;
      name: string;
      sort?: { field: string; direction: string }[];
    } | null,

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
    getEntityCommands: () => [],
    schemas: {},
    loading: false,
  }),
  useSchemaOptional: () => ({
    getSchema: () => undefined,
    getFieldDef: () => undefined,
    getEntityCommands: () => [],
  }),
}));

vi.mock("@/lib/entity-store-context", () => ({
  useEntityStore: () => ({
    getEntities: () => [],
    getEntity: () => undefined,
    subscribe: () => () => {},
  }),
  useFieldValue: () => undefined,
}));

vi.mock("@/lib/entity-focus-context", () => ({
  useEntityFocus: () => ({
    setFocus: vi.fn(),
    registerScope: vi.fn(),
    unregisterScope: vi.fn(),
    getScope: vi.fn(),
    registerSpatialKey: vi.fn(),
    unregisterSpatialKey: vi.fn(),
    getFocusedMoniker: () => null,
    subscribeFocus: () => () => {},
  }),
  useFocusedMoniker: () => null,
  useFocusedScope: () => null,
  useIsFocused: () => false,
}));

vi.mock("@/hooks/use-grid", () => ({
  useGrid: () => ({
    cursor: null,
    mode: "normal",
    selection: null,
    enterEdit: vi.fn(),
    exitEdit: vi.fn(),
    enterVisual: vi.fn(),
    exitVisual: vi.fn(),
    expandSelection: vi.fn(),
    getSelectedRange: () => null,
  }),
}));

// ---------------------------------------------------------------------------
// Import after mocks.
// ---------------------------------------------------------------------------

import { GridView } from "./grid-view";

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("GridView", () => {
  it("renders without crash when activePerspective is null", async () => {
    mockActivePerspective.mockReturnValue({
      activePerspective: null,

      applySort: (entities: unknown[]) => entities,
      groupField: undefined,
    });

    // Should not throw ReferenceError: activePerspective is not defined.
    // Wrap render + a microtask flush in async act() so the post-mount
    // setVisibleRowCount effect in DataTable settles inside the test's
    // act() scope instead of emitting an "update was not wrapped in act(...)"
    // warning.
    await act(async () => {
      render(
        <GridView
          view={{
            id: "v1",
            name: "Grid",
            kind: "grid",
            entity_type: "task",
          }}
        />,
      );
    });
  });

  it("renders without crash when activePerspective has sort entries", async () => {
    mockActivePerspective.mockReturnValue({
      activePerspective: {
        id: "p1",
        name: "Default",
        sort: [{ field: "title", direction: "asc" }],
      },

      applySort: (entities: unknown[]) => entities,
      groupField: undefined,
    });

    await act(async () => {
      render(
        <GridView
          view={{
            id: "v1",
            name: "Grid",
            kind: "grid",
            entity_type: "task",
          }}
        />,
      );
    });
  });
});
