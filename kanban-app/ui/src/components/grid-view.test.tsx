import { describe, it, expect, vi } from "vitest";
import { render } from "@testing-library/react";

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
    activePerspective: null,
    applyFilter: (entities: unknown[]) => entities,
    applySort: (entities: unknown[]) => entities,
    groupField: undefined,
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
  useSchemaOptional: () => undefined,
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
    registerClaimPredicates: vi.fn(),
    unregisterClaimPredicates: vi.fn(),
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
  it("renders without crash when activePerspective is null", () => {
    mockActivePerspective.mockReturnValue({
      activePerspective: null,
      applyFilter: (entities: unknown[]) => entities,
      applySort: (entities: unknown[]) => entities,
      groupField: undefined,
    });

    // Should not throw ReferenceError: activePerspective is not defined
    expect(() =>
      render(
        <GridView
          view={{
            id: "v1",
            name: "Grid",
            kind: "grid",
            entity_type: "task",
          }}
        />,
      ),
    ).not.toThrow();
  });

  it("renders without crash when activePerspective has sort entries", () => {
    mockActivePerspective.mockReturnValue({
      activePerspective: {
        id: "p1",
        name: "Default",
        sort: [{ field: "title", direction: "asc" }],
      },
      applyFilter: (entities: unknown[]) => entities,
      applySort: (entities: unknown[]) => entities,
      groupField: undefined,
    });

    expect(() =>
      render(
        <GridView
          view={{
            id: "v1",
            name: "Grid",
            kind: "grid",
            entity_type: "task",
          }}
        />,
      ),
    ).not.toThrow();
  });
});
