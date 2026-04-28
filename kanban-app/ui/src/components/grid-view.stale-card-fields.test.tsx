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
// Mock GridView's context dependencies.
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

// Task-like schema with ONLY the fields the task entity actually declares.
// Views that reference `status`, `priority`, or `due_date` must NOT resolve
// these names — they're supposed to drop with a warn.
vi.mock("@/lib/schema-context", () => ({
  useSchema: () => ({
    getSchema: () => ({
      entity: { entity_type: "task", search_display_field: "title" },
      fields: [
        { name: "title", type: "string", section: "header", display: "text" },
        { name: "tags", type: "array", section: "header", display: "badges" },
        {
          name: "assignees",
          type: "array",
          section: "header",
          display: "badges",
        },
        { name: "due", type: "date", section: "dates", display: "text" },
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
      focusedMoniker: null,
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

// Render nothing for DataTable — this test only exercises column resolution
// in `useGridData`, not the table body.
vi.mock("@/components/data-table", () => ({
  DataTable: () => null,
}));

// ---------------------------------------------------------------------------
// Import after mocks.
// ---------------------------------------------------------------------------

import { GridView } from "./grid-view";
import { TooltipProvider } from "@/components/ui/tooltip";

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("GridView — stale card_fields", () => {
  let warnSpy: ReturnType<typeof vi.spyOn>;

  beforeEach(() => {
    mockActivePerspective.mockReturnValue({
      activePerspective: null,
      applySort: (entities: unknown[]) => entities,
      groupField: undefined,
    });
    // `vi.spyOn(console, "warn")` returns the same spy on repeated calls
    // within a test file — reset it so each test sees only its own warns.
    warnSpy = vi.spyOn(console, "warn").mockImplementation(() => {});
    warnSpy.mockClear();
  });

  it("warns once per unknown card_field naming the view, field, and valid fields list", async () => {
    await act(async () => {
      render(
        <TooltipProvider>
          <GridView
            view={{
              // Use a unique view id so the module-scoped dedupe Set doesn't
              // swallow this test's warn because another test ran first.
              id: "v-stale-1",
              name: "Tasks Grid",
              kind: "grid",
              entity_type: "task",
              // Three unknown + one valid. Only `title` resolves; the other
              // three must each produce exactly one warn.
              card_fields: ["title", "status", "priority", "due_date"],
            }}
          />
        </TooltipProvider>,
      );
    });

    // One warn per unknown card_field — three total.
    const warnMsgs = warnSpy.mock.calls
      .map((c: unknown[]) => (typeof c[0] === "string" ? c[0] : ""))
      .filter((s: string) => s.includes("unknown card_field"));

    expect(warnMsgs).toHaveLength(3);

    // Each warn must mention the view id, name, bad field, and the valid set.
    for (const msg of warnMsgs) {
      expect(msg).toContain("v-stale-1");
      expect(msg).toContain("Tasks Grid");
      // Valid field names list must appear in every warn so the author can
      // correct their typo without digging through schema files.
      expect(msg).toContain("title");
      expect(msg).toContain("tags");
      expect(msg).toContain("assignees");
      expect(msg).toContain("due");
    }

    // And each bad name is called out in its own warn.
    expect(warnMsgs.some((m: string) => m.includes('"status"'))).toBe(true);
    expect(warnMsgs.some((m: string) => m.includes('"priority"'))).toBe(true);
    expect(warnMsgs.some((m: string) => m.includes('"due_date"'))).toBe(true);
  });

  it("does not warn when all card_fields are valid", async () => {
    await act(async () => {
      render(
        <TooltipProvider>
          <GridView
            view={{
              id: "v-stale-2",
              name: "Clean Grid",
              kind: "grid",
              entity_type: "task",
              card_fields: ["title", "tags", "assignees", "due"],
            }}
          />
        </TooltipProvider>,
      );
    });

    const warnMsgs = warnSpy.mock.calls
      .map((c: unknown[]) => (typeof c[0] === "string" ? c[0] : ""))
      .filter((s: string) => s.includes("unknown card_field"));

    expect(warnMsgs).toHaveLength(0);
  });

  it("warns only once for the same (view, field) pair across re-renders", async () => {
    const view = {
      id: "v-stale-3",
      name: "Dedupe Grid",
      kind: "grid" as const,
      entity_type: "task",
      card_fields: ["title", "unknownField"],
    };

    let rendered: ReturnType<typeof render> | undefined;
    await act(async () => {
      rendered = render(
        <TooltipProvider>
          <GridView view={view} />
        </TooltipProvider>,
      );
    });

    // Force a re-render by re-rendering with the same view object.
    await act(async () => {
      rendered!.rerender(
        <TooltipProvider>
          <GridView view={view} />
        </TooltipProvider>,
      );
    });

    const warnMsgs = warnSpy.mock.calls
      .map((c: unknown[]) => (typeof c[0] === "string" ? c[0] : ""))
      .filter(
        (s: string) =>
          s.includes("unknown card_field") && s.includes("v-stale-3"),
      );

    // Despite two renders, only one warn for this (view, field) pair.
    expect(warnMsgs).toHaveLength(1);
  });
});
