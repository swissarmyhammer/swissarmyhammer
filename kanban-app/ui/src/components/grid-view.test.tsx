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
    focusedMoniker: null,
    setFocus: vi.fn(),
    registerScope: vi.fn(),
    unregisterScope: vi.fn(),
    getScope: vi.fn(),
    broadcastNavCommand: vi.fn(),
    registerClaim: vi.fn(),
    unregisterClaim: vi.fn(),
  }),
  useIsFocused: () => false,
}));

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
