import { describe, it, expect, vi } from "vitest";
import { render, act, screen, fireEvent } from "@testing-library/react";
import React, { useContext, useEffect } from "react";

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

// Mock DataTable so tests can inject a probe component into GridView's
// CommandScope subtree. The mock reads a module-scoped slot and renders
// whatever component was assigned to it — set the slot to a component that
// calls a grid command, render GridView, and the command executes inside
// the real scope.
let dataTableSlot: React.ReactNode = null;
vi.mock("@/components/data-table", () => ({
  DataTable: () => <>{dataTableSlot}</>,
}));

// ---------------------------------------------------------------------------
// Import after mocks.
// ---------------------------------------------------------------------------

import { GridView } from "./grid-view";
import { TooltipProvider } from "@/components/ui/tooltip";
import { CommandScopeContext, resolveCommand } from "@/lib/command-scope";

// ---------------------------------------------------------------------------
// Probe — reaches into the grid's CommandScope and runs a command by id.
// ---------------------------------------------------------------------------

/**
 * Looks up a command by id in the enclosing CommandScope and invokes its
 * `execute` on mount. Used by tests to exercise keyboard commands without
 * simulating the full key-binding pipeline.
 */
function RunCommandProbe({ id }: { id: string }) {
  const scope = useContext(CommandScopeContext);
  useEffect(() => {
    const cmd = resolveCommand(scope, id);
    if (cmd?.execute) cmd.execute();
  }, [scope, id]);
  return null;
}

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
        <TooltipProvider>
          <GridView
            view={{
              id: "v1",
              name: "Grid",
              kind: "grid",
              entity_type: "task",
            }}
          />
        </TooltipProvider>,
      );
    });
  });

  it("renders a visible + button with entity-type aria-label", async () => {
    mockActivePerspective.mockReturnValue({
      activePerspective: null,
      applySort: (entities: unknown[]) => entities,
      groupField: undefined,
    });

    await act(async () => {
      render(
        <TooltipProvider>
          <GridView
            view={{
              id: "v1",
              name: "Grid",
              kind: "grid",
              entity_type: "task",
            }}
          />
        </TooltipProvider>,
      );
    });

    const button = screen.getByRole("button", { name: "Add Task" });
    expect(button).toBeTruthy();
  });

  it("uses the correct aria-label for non-task entity types", async () => {
    mockActivePerspective.mockReturnValue({
      activePerspective: null,
      applySort: (entities: unknown[]) => entities,
      groupField: undefined,
    });

    await act(async () => {
      render(
        <TooltipProvider>
          <GridView
            view={{
              id: "v-tag",
              name: "Tags",
              kind: "grid",
              entity_type: "tag",
            }}
          />
        </TooltipProvider>,
      );
    });

    expect(screen.getByRole("button", { name: "Add Tag" })).toBeTruthy();
  });

  // Parameterised dispatch tests — the + button, `grid.newBelow`, and
  // `grid.newAbove` must all dispatch `entity.add:{entityType}` uniformly
  // for every entity type that has a grid view. Before this test covered
  // all three types, the tag grid worked but the task and project grids
  // silently failed in the field — there was no automated proof that the
  // three grids behave identically.
  const DISPATCH_CASES: Array<{
    entityType: string;
    buttonLabel: string;
  }> = [
    { entityType: "task", buttonLabel: "Add Task" },
    { entityType: "tag", buttonLabel: "Add Tag" },
    { entityType: "project", buttonLabel: "Add Project" },
  ];

  describe.each(DISPATCH_CASES)(
    "entity.add dispatch for $entityType grid",
    ({ entityType, buttonLabel }) => {
      it("dispatches entity.add:{entityType} when + button is clicked", async () => {
        mockActivePerspective.mockReturnValue({
          activePerspective: null,
          applySort: (entities: unknown[]) => entities,
          groupField: undefined,
        });
        mockInvoke.mockClear();

        await act(async () => {
          render(
            <TooltipProvider>
              <GridView
                view={{
                  id: `v-${entityType}`,
                  name: "Grid",
                  kind: "grid",
                  entity_type: entityType,
                }}
              />
            </TooltipProvider>,
          );
        });

        const button = screen.getByRole("button", { name: buttonLabel });
        await act(async () => {
          fireEvent.click(button);
        });

        // Verify dispatch_command was invoked with entity.add:{entityType}.
        // We intentionally do NOT pass a title override — schemas supply the
        // default label for each entity type (see `addNewEntity` in grid-view).
        const dispatchCall = mockInvoke.mock.calls.find(
          (c) => c[0] === "dispatch_command",
        );
        expect(dispatchCall).toBeTruthy();
        const payload = dispatchCall?.[1] as
          | { cmd?: string; args?: Record<string, unknown> }
          | undefined;
        expect(payload?.cmd).toBe(`entity.add:${entityType}`);
      });

      it("dispatches entity.add:{entityType} via the grid.newBelow keyboard command", async () => {
        mockActivePerspective.mockReturnValue({
          activePerspective: null,
          applySort: (entities: unknown[]) => entities,
          groupField: undefined,
        });
        mockInvoke.mockClear();
        dataTableSlot = <RunCommandProbe id="grid.newBelow" />;

        await act(async () => {
          render(
            <TooltipProvider>
              <GridView
                view={{
                  id: `v-${entityType}`,
                  name: "Grid",
                  kind: "grid",
                  entity_type: entityType,
                }}
              />
            </TooltipProvider>,
          );
        });

        const dispatchCalls = mockInvoke.mock.calls.filter(
          (c) => c[0] === "dispatch_command",
        );
        // Exactly one dispatch — from the probe running grid.newBelow.
        expect(dispatchCalls.length).toBe(1);
        const payload = dispatchCalls[0][1] as
          | { cmd?: string; args?: Record<string, unknown> }
          | undefined;
        expect(payload?.cmd).toBe(`entity.add:${entityType}`);

        dataTableSlot = null;
      });

      it("dispatches entity.add:{entityType} via the grid.newAbove keyboard command", async () => {
        mockActivePerspective.mockReturnValue({
          activePerspective: null,
          applySort: (entities: unknown[]) => entities,
          groupField: undefined,
        });
        mockInvoke.mockClear();
        dataTableSlot = <RunCommandProbe id="grid.newAbove" />;

        await act(async () => {
          render(
            <TooltipProvider>
              <GridView
                view={{
                  id: `v-${entityType}`,
                  name: "Grid",
                  kind: "grid",
                  entity_type: entityType,
                }}
              />
            </TooltipProvider>,
          );
        });

        const dispatchCalls = mockInvoke.mock.calls.filter(
          (c) => c[0] === "dispatch_command",
        );
        expect(dispatchCalls.length).toBe(1);
        const payload = dispatchCalls[0][1] as
          | { cmd?: string; args?: Record<string, unknown> }
          | undefined;
        expect(payload?.cmd).toBe(`entity.add:${entityType}`);

        dataTableSlot = null;
      });
    },
  );

  it("renders the missing-entity-type fallback for non-empty-but-invalid entity_type", async () => {
    mockActivePerspective.mockReturnValue({
      activePerspective: null,
      applySort: (entities: unknown[]) => entities,
      groupField: undefined,
    });

    await act(async () => {
      render(
        <TooltipProvider>
          <GridView
            view={{
              id: "v-bad",
              name: "Bad",
              kind: "grid",
              // Invalid: contains uppercase and a space. `VALID_ENTITY_TYPE`
              // sanitizes this to "" and GridView should bail out without
              // rendering the AddEntityBar.
              entity_type: "Foo Bar",
            }}
          />
        </TooltipProvider>,
      );
    });

    expect(
      screen.getByText("View is missing an entity_type definition."),
    ).toBeTruthy();
    // No AddEntityBar rendered — "Add Foo Bar" button must not exist.
    expect(screen.queryByRole("button")).toBeNull();
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
        <TooltipProvider>
          <GridView
            view={{
              id: "v1",
              name: "Grid",
              kind: "grid",
              entity_type: "task",
            }}
          />
        </TooltipProvider>,
      );
    });
  });
});
