/**
 * GridEmptyState — empty-grid affordances.
 *
 * Covers the empty-state branch of `GridView` (entities.length === 0):
 *   1. A prominent "New <EntityType>" button is rendered (not the faint
 *      `text-muted-foreground/50` `+` of the secondary `AddEntityBar`).
 *   2. Clicking the button dispatches `entity.add:{entityType}`.
 *   3. Right-clicking the empty-state wrapper fires
 *      `list_commands_for_scope` with a scope chain that contains
 *      `view:<viewId>` — the view-scoped command palette that produces
 *      "New <EntityType>" in the native context menu.
 *
 * Isolates this behaviour from the rest of the grid-view test suite
 * because the existing `grid-view.test.tsx` mocks `DataTable` (the probe
 * injection pattern) and asserts on keyboard-command dispatch. Mixing
 * the empty-state assertions into that file would blur the intent and
 * make the probe mock bleed into empty-state tests.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, act, screen, fireEvent } from "@testing-library/react";

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
// Dependency mocks.
// ---------------------------------------------------------------------------

vi.mock("@/components/perspective-container", () => ({
  useActivePerspective: () => ({
    activePerspective: null,
    applySort: (entities: unknown[]) => entities,
    groupField: undefined,
  }),
}));

vi.mock("@/lib/schema-context", () => ({
  useSchema: () => ({
    getSchema: () => ({
      entity: { entity_type: "tag", search_display_field: "tag_name" },
      fields: [
        {
          name: "tag_name",
          type: "string",
          section: "header",
          display: "text",
        },
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

vi.mock("@/lib/entity-store-context", () => ({
  useEntityStore: () => ({
    getEntities: () => [],
    getEntity: () => undefined,
    subscribe: () => () => {},
  }),
  useFieldValue: () => undefined,
}));

vi.mock("@/lib/entity-focus-context", () => {
  const actions = {
    setFocus: vi.fn(),
    registerScope: vi.fn(),
    unregisterScope: vi.fn(),
    getScope: vi.fn(),
    broadcastNavCommand: vi.fn(),
  };
  return {
    useEntityFocus: () => ({
      focusedFq: null,
      setFocus: vi.fn(),
      registerScope: vi.fn(),
      unregisterScope: vi.fn(),
      getScope: vi.fn(),
      broadcastNavCommand: vi.fn(),
    }),
    useFocusActions: () => actions,
    useOptionalFocusActions: () => actions,
    useEntityScopeRegistration: () => {},
    useFocusedMoniker: () => null,
    useFocusedMonikerRef: () => ({ current: null }),
    useIsFocused: () => false,
    useIsDirectFocus: () => false,
    useOptionalIsDirectFocus: () => false,
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

// ---------------------------------------------------------------------------
// Imports after mocks.
// ---------------------------------------------------------------------------

import { GridView } from "./grid-view";
import { TooltipProvider } from "@/components/ui/tooltip";
import { CommandScopeProvider } from "@/lib/command-scope";

// ---------------------------------------------------------------------------
// Test helpers.
// ---------------------------------------------------------------------------

/**
 * Render `GridView` for a tags grid with entities=[], wrapped in a
 * parent `CommandScopeProvider` that injects a `view:<id>` moniker the
 * way `ViewContainer` does in production.
 */
async function renderTagsEmptyGrid(viewId: string) {
  await act(async () => {
    render(
      <TooltipProvider>
        <CommandScopeProvider moniker={`view:${viewId}`}>
          <GridView
            view={{
              id: viewId,
              name: "Tags",
              kind: "grid",
              entity_type: "tag",
            }}
          />
        </CommandScopeProvider>
      </TooltipProvider>,
    );
  });
}

// ---------------------------------------------------------------------------
// Tests.
// ---------------------------------------------------------------------------

describe("GridEmptyState", () => {
  beforeEach(() => {
    mockInvoke.mockClear();
    mockInvoke.mockImplementation(async (cmd: string): Promise<unknown> => {
      if (cmd === "list_commands_for_scope") return [];
      return undefined;
    });
  });

  it("renders a prominent 'New Tag' button (not the muted AddEntityBar '+')", async () => {
    await renderTagsEmptyGrid("v-tags");

    const button = screen.getByRole("button", { name: "New Tag" });
    expect(button).toBeTruthy();

    // The button must use the primary `Button` variant (data-variant="default")
    // and must NOT carry the muted `text-muted-foreground/50` class that the
    // AddEntityBar uses for its secondary `+` affordance — that's the whole
    // point of the empty-state UX: make the call-to-action visible.
    expect(button.getAttribute("data-variant")).toBe("default");
    expect(button.className).not.toMatch(/text-muted-foreground\/50/);
  });

  it("dispatches entity.add:tag when the New Tag button is clicked", async () => {
    await renderTagsEmptyGrid("v-tags");

    const button = screen.getByRole("button", { name: "New Tag" });
    await act(async () => {
      fireEvent.click(button);
    });

    const dispatchCall = mockInvoke.mock.calls.find(
      (c) => c[0] === "dispatch_command",
    );
    expect(dispatchCall).toBeTruthy();
    const payload = dispatchCall?.[1] as { cmd?: string } | undefined;
    expect(payload?.cmd).toBe("entity.add:tag");
  });

  it("fires list_commands_for_scope on context-menu over the empty-state wrapper", async () => {
    await renderTagsEmptyGrid("v-tags");

    const wrapper = screen.getByTestId("grid-empty-state");
    await act(async () => {
      fireEvent.contextMenu(wrapper);
    });

    const listCall = mockInvoke.mock.calls.find(
      (c) => c[0] === "list_commands_for_scope",
    );
    expect(listCall).toBeTruthy();
    const args = listCall?.[1] as
      | { scopeChain?: string[]; contextMenu?: boolean }
      | undefined;
    expect(args?.contextMenu).toBe(true);
    // The scope chain must include the `view:<id>` moniker injected by the
    // parent CommandScopeProvider — that's how the backend emits the
    // view-scoped `entity.add:{type}` command.
    expect(args?.scopeChain).toEqual(expect.arrayContaining(["view:v-tags"]));
  });
});
