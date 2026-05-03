/**
 * Integration test for the end-to-end "external clearFilter" path:
 *
 *   When the active perspective's `filter` transitions from `"#bug"` to
 *   `undefined` (the shape the PerspectiveProvider returns after a refetch
 *   following `perspective.clearFilter` from a context menu / command
 *   palette / keybinding), the FilterFormulaBar's CM6 editor must reset its
 *   buffer to empty.
 *
 * The unit-level coverage lives in `filter-editor.external-clear.test.tsx`.
 * This test verifies the same path through the live `PerspectiveTabBar` tree
 * (which is what the user actually sees), to catch regressions in the wiring
 * between `activePerspective.filter`, the `FilterFormulaBar` key, and the
 * embedded `FilterEditor`.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, act } from "@testing-library/react";
import { TooltipProvider } from "@/components/ui/tooltip";

/**
 * Read the current CM6 doc contents inside the formula bar. CM6 renders the
 * placeholder inside `.cm-content` via a DOM widget, so `textContent` is NOT
 * a reliable way to assert on an empty buffer — the assertion lives in the
 * actual EditorState doc string. Returns an empty string if the formula bar
 * or editor view hasn't mounted yet.
 */
async function readFormulaBarDoc(container: HTMLElement): Promise<string> {
  const cmEditor = container
    .querySelector('[data-testid="filter-formula-bar"]')
    ?.querySelector(".cm-editor") as HTMLElement | null;
  if (!cmEditor) return "";
  const { EditorView } = await import("@codemirror/view");
  const view = EditorView.findFromDOM(cmEditor);
  return view?.state.doc.toString() ?? "";
}

// Mock Tauri APIs before importing any modules that use them.
// eslint-disable-next-line @typescript-eslint/no-explicit-any
const mockInvoke = vi.fn((..._args: any[]) => Promise.resolve(null));
vi.mock("@tauri-apps/api/core", () => ({
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  invoke: (...args: any[]) => mockInvoke(...args),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ label: "main" }),
}));
vi.mock("@tauri-apps/plugin-log", () => ({
  error: vi.fn(),
  warn: vi.fn(),
  info: vi.fn(),
  debug: vi.fn(),
  trace: vi.fn(),
  attachConsole: vi.fn(() => Promise.resolve()),
}));

/** Shape of a perspective in the mock context. */
type MockPerspective = {
  id: string;
  name: string;
  view: string;
  filter?: string;
  group?: string;
};

const mockSetActivePerspectiveId = vi.fn();
const mockRefresh = vi.fn(() => Promise.resolve());

// Mutable perspective context — individual tests mutate this before each
// `rerender(...)` call to simulate a backend-driven refresh.
let mockPerspectivesValue = {
  perspectives: [] as MockPerspective[],
  activePerspective: null as MockPerspective | null,
  setActivePerspectiveId: mockSetActivePerspectiveId,
  refresh: mockRefresh,
};

vi.mock("@/lib/perspective-context", () => ({
  usePerspectives: () => mockPerspectivesValue,
}));

let mockViewsValue = {
  views: [{ id: "board-1", name: "Board", kind: "board", icon: "kanban" }],
  activeView: { id: "board-1", name: "Board", kind: "board", icon: "kanban" },
  setActiveViewId: vi.fn(),
  refresh: vi.fn(() => Promise.resolve()),
};
vi.mock("@/lib/views-context", () => ({
  useViews: () => mockViewsValue,
}));

vi.mock("@/lib/context-menu", () => ({
  useContextMenu: () => vi.fn(),
}));

vi.mock("@/lib/entity-store-context", () => ({
  useEntityStore: () => ({ getEntities: () => [] }),
}));

vi.mock("@/components/window-container", () => ({
  useBoardData: () => ({ virtualTagMeta: [] }),
}));

vi.mock("@/lib/schema-context", () => ({
  useSchema: () => ({
    getSchema: () => ({ entity: { name: "task", fields: [] }, fields: [] }),
    getFieldDef: () => undefined,
    mentionableTypes: [],
    loading: false,
  }),
}));

let mockKeymapMode = "cua";
const mockUIState = () => ({
  keymap_mode: mockKeymapMode,
  scope_chain: [],
  open_boards: [],
  has_clipboard: false,
  clipboard_entity_type: null,
  windows: {},
  recent_boards: [],
});
vi.mock("@/lib/ui-state-context", () => ({
  useUIState: () => mockUIState(),
  useUIStateLoading: () => ({ state: mockUIState(), loading: false }),
}));

import { PerspectiveTabBar } from "./perspective-tab-bar";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { FocusLayer } from "./focus-layer";
import { asSegment } from "@/types/spatial";

/**
 * Render PerspectiveTabBar with the required providers. The spatial
 * provider stack (`SpatialFocusProvider` + `FocusLayer`) is required
 * since `PerspectiveTabBar` mounts spatial primitives and the
 * no-spatial-context fallback was removed in card
 * `01KQPVA127YMJ8D7NB6M824595`.
 */
function renderTabBar() {
  return render(
    <SpatialFocusProvider>
      <FocusLayer name={asSegment("window")}>
        <TooltipProvider delayDuration={100}>
          <PerspectiveTabBar />
        </TooltipProvider>
      </FocusLayer>
    </SpatialFocusProvider>,
  );
}

/**
 * Rebuild the `mockPerspectivesValue` object with a fresh reference so
 * React sees the identity change via the usePerspectives hook mock and
 * runs the effect in `FilterEditorBody`. Consumers set either a filter
 * string or `undefined` — the refetched shape after a clearFilter event.
 */
function setActivePerspectiveFilter(filter: string | undefined) {
  const current = mockPerspectivesValue.activePerspective;
  if (!current) throw new Error("no active perspective in mock");
  const updated: MockPerspective = { ...current, filter };
  mockPerspectivesValue = {
    ...mockPerspectivesValue,
    perspectives: mockPerspectivesValue.perspectives.map((p) =>
      p.id === current.id ? updated : p,
    ),
    activePerspective: updated,
  };
}

describe("PerspectiveTabBar — external clearFilter resets formula bar buffer", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockKeymapMode = "cua";
    mockPerspectivesValue = {
      perspectives: [],
      activePerspective: null,
      setActivePerspectiveId: mockSetActivePerspectiveId,
      refresh: mockRefresh,
    };
    mockViewsValue = {
      views: [{ id: "board-1", name: "Board", kind: "board", icon: "kanban" }],
      activeView: {
        id: "board-1",
        name: "Board",
        kind: "board",
        icon: "kanban",
      },
      setActiveViewId: vi.fn(),
      refresh: vi.fn(() => Promise.resolve()),
    };
  });

  it("formula bar CM6 buffer clears when activePerspective.filter transitions from '#bug' to undefined", async () => {
    // Seed: the active perspective carries filter="#bug".
    mockPerspectivesValue = {
      ...mockPerspectivesValue,
      perspectives: [
        { id: "p1", name: "Sprint", view: "board", filter: "#bug" },
      ],
      activePerspective: {
        id: "p1",
        name: "Sprint",
        view: "board",
        filter: "#bug",
      },
    };

    const { container, rerender } = renderTabBar();

    // Verify the CM6 buffer reflects the seeded filter.
    expect(await readFormulaBarDoc(container)).toBe("#bug");

    // Simulate the external `perspective.clearFilter` path: the backend
    // emits entity-field-changed; PerspectiveProvider refetches; the
    // active perspective arrives with `filter: undefined`.
    setActivePerspectiveFilter(undefined);

    await act(async () => {
      rerender(
        <SpatialFocusProvider>
          <FocusLayer name={asSegment("window")}>
            <TooltipProvider delayDuration={100}>
              <PerspectiveTabBar />
            </TooltipProvider>
          </FocusLayer>
        </SpatialFocusProvider>,
      );
      // Allow the reconciliation effect + setValue dispatch to run.
      await new Promise((r) => setTimeout(r, 20));
    });

    // The formula bar must remain the same mounted editor (keyed on
    // activePerspective.id only — no remount on filter change) AND its
    // buffer must now be empty.
    expect(await readFormulaBarDoc(container)).toBe("");

    // Invariant: the formula bar's CM6 placeholder should be visible again.
    const placeholder = container
      .querySelector('[data-testid="filter-formula-bar"]')
      ?.querySelector(".cm-placeholder");
    expect(placeholder).toBeTruthy();
  });

  it("formula bar CM6 buffer updates when activePerspective.filter transitions from '#bug' to '@alice'", async () => {
    // Reproduces the "external filter set" path — e.g. a filter set
    // dispatched from another window. The undo-of-clearFilter path is
    // covered by its own dedicated test below, which starts from the
    // cleared state (undefined) and restores a filter.
    mockPerspectivesValue = {
      ...mockPerspectivesValue,
      perspectives: [
        { id: "p1", name: "Sprint", view: "board", filter: "#bug" },
      ],
      activePerspective: {
        id: "p1",
        name: "Sprint",
        view: "board",
        filter: "#bug",
      },
    };

    const { container, rerender } = renderTabBar();
    expect(await readFormulaBarDoc(container)).toBe("#bug");

    setActivePerspectiveFilter("@alice");
    await act(async () => {
      rerender(
        <SpatialFocusProvider>
          <FocusLayer name={asSegment("window")}>
            <TooltipProvider delayDuration={100}>
              <PerspectiveTabBar />
            </TooltipProvider>
          </FocusLayer>
        </SpatialFocusProvider>,
      );
      await new Promise((r) => setTimeout(r, 20));
    });

    expect(await readFormulaBarDoc(container)).toBe("@alice");
  });

  it("undo of clearFilter restores the previous filter: activePerspective.filter transitions from undefined to '#bug' → CM6 buffer shows '#bug'", async () => {
    // Literal undo sequence for `perspective.clearFilter`:
    //
    //   1. Some earlier action cleared the filter → active perspective
    //      arrives with `filter: undefined`.
    //   2. User hits Ctrl+Z (or equivalent). The undo handler re-sets the
    //      previous filter value; PerspectiveProvider refetches; the
    //      active perspective now carries `filter: "#bug"` again.
    //
    // The formula bar's CM6 buffer must follow. Starting from undefined
    // (not from a different non-empty filter) mirrors the real undo
    // shape one-to-one — the perspective's filter field is cleared, not
    // replaced.
    mockPerspectivesValue = {
      ...mockPerspectivesValue,
      perspectives: [
        { id: "p1", name: "Sprint", view: "board", filter: undefined },
      ],
      activePerspective: {
        id: "p1",
        name: "Sprint",
        view: "board",
        filter: undefined,
      },
    };

    const { container, rerender } = renderTabBar();
    // Baseline: cleared state — the buffer is empty and the placeholder
    // should be visible.
    expect(await readFormulaBarDoc(container)).toBe("");

    // Undo restores the prior filter. PerspectiveProvider refetches and
    // re-renders the tab bar with the restored value.
    setActivePerspectiveFilter("#bug");
    await act(async () => {
      rerender(
        <SpatialFocusProvider>
          <FocusLayer name={asSegment("window")}>
            <TooltipProvider delayDuration={100}>
              <PerspectiveTabBar />
            </TooltipProvider>
          </FocusLayer>
        </SpatialFocusProvider>,
      );
      await new Promise((r) => setTimeout(r, 20));
    });

    expect(await readFormulaBarDoc(container)).toBe("#bug");
  });

  it("switching perspectives still remounts the editor (key={activePerspective.id} is preserved)", async () => {
    // Guard against an over-eager fix: the `key={activePerspective.id}` on
    // the FilterFormulaBar must still remount when the active perspective
    // changes. Filter reconciliation handles *same-perspective* mutations;
    // perspective switches must go through the remount path so CM6 state
    // (selection, vim mode) resets cleanly.
    mockPerspectivesValue = {
      ...mockPerspectivesValue,
      perspectives: [
        { id: "p1", name: "First", view: "board", filter: "#bug" },
        { id: "p2", name: "Second", view: "board", filter: "@alice" },
      ],
      activePerspective: {
        id: "p1",
        name: "First",
        view: "board",
        filter: "#bug",
      },
    };

    const { container, rerender } = renderTabBar();
    expect(await readFormulaBarDoc(container)).toBe("#bug");

    // Switch active perspective — the FilterFormulaBar is keyed on id, so
    // it unmounts and remounts with the new initial filter.
    mockPerspectivesValue = {
      ...mockPerspectivesValue,
      activePerspective: {
        id: "p2",
        name: "Second",
        view: "board",
        filter: "@alice",
      },
    };
    await act(async () => {
      rerender(
        <SpatialFocusProvider>
          <FocusLayer name={asSegment("window")}>
            <TooltipProvider delayDuration={100}>
              <PerspectiveTabBar />
            </TooltipProvider>
          </FocusLayer>
        </SpatialFocusProvider>,
      );
      await new Promise((r) => setTimeout(r, 20));
    });

    expect(await readFormulaBarDoc(container)).toBe("@alice");
  });
});
