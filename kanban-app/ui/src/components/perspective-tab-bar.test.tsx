import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, act } from "@testing-library/react";
import { TooltipProvider } from "@/components/ui/tooltip";

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

// Track perspective context values for assertions.
const mockSetActivePerspectiveId = vi.fn();
const mockRefresh = vi.fn(() => Promise.resolve());

/** Shape of a perspective in the mock context — includes optional filter/group. */
type MockPerspective = {
  id: string;
  name: string;
  view: string;
  filter?: string;
  group?: string;
};

// Mock the perspectives context so we can control perspectives list.
let mockPerspectivesValue = {
  perspectives: [] as MockPerspective[],
  activePerspective: null as MockPerspective | null,
  setActivePerspectiveId: mockSetActivePerspectiveId,
  refresh: mockRefresh,
};

vi.mock("@/lib/perspective-context", () => ({
  usePerspectives: () => mockPerspectivesValue,
}));

// Mock the views context so we can control the active view kind.
let mockViewsValue = {
  views: [{ id: "board-1", name: "Board", kind: "board", icon: "kanban" }],
  activeView: { id: "board-1", name: "Board", kind: "board", icon: "kanban" },
  setActiveViewId: vi.fn(),
  refresh: vi.fn(() => Promise.resolve()),
};

vi.mock("@/lib/views-context", () => ({
  useViews: () => mockViewsValue,
}));

// Mock useContextMenu — returns a handler that records calls.
const mockContextMenuHandler = vi.fn();
vi.mock("@/lib/context-menu", () => ({
  useContextMenu: () => mockContextMenuHandler,
}));

// Mock useEntityStore — needed by useMentionExtensions (used in FilterEditor).
vi.mock("@/lib/entity-store-context", () => ({
  useEntityStore: () => ({ getEntities: () => [] }),
}));

// Mock board data context — provides virtual tag metadata from the backend.
vi.mock("@/components/window-container", () => ({
  useBoardData: () => ({ virtualTagMeta: [] }),
}));

// Mock useSchema — returns empty schema by default.
vi.mock("@/lib/schema-context", () => ({
  useSchema: () => ({
    getSchema: () => ({ entity: { name: "task", fields: [] }, fields: [] }),
    getFieldDef: () => undefined,
    mentionableTypes: [],
    loading: false,
  }),
}));

// Mock useUIState — required by TextEditor (CM6 keymap selection).
// Mutable so tests can switch between CUA/vim/emacs.
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

import { PerspectiveTabBar, triggerStartRename } from "./perspective-tab-bar";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { FocusLayer } from "./focus-layer";
import { asSegment } from "@/types/spatial";

/**
 * Renders PerspectiveTabBar inside the required providers. The spatial
 * provider stack (`SpatialFocusProvider` + `FocusLayer`) is required
 * since `PerspectiveTabBar` mounts `<FocusScope>` / `<FocusScope>` and
 * the no-spatial-context fallback was removed in card
 * `01KQPVA127YMJ8D7NB6M824595`. `<EntityFocusProvider>` is required
 * because `FilterEditorDrillOutWiring` (mounted by the filter editor's
 * surrounding `<FocusScope>`) calls `useFocusActions()` to dispatch
 * focus changes when the editor's drill-out fires.
 */
function renderTabBar(delayDuration = 100) {
  return render(
    <SpatialFocusProvider>
      <FocusLayer name={asSegment("window")}>
        <EntityFocusProvider>
          <TooltipProvider delayDuration={delayDuration}>
            <PerspectiveTabBar />
          </TooltipProvider>
        </EntityFocusProvider>
      </FocusLayer>
    </SpatialFocusProvider>,
  );
}

describe("PerspectiveTabBar", () => {
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

  it("renders tabs for perspectives matching the current view kind", () => {
    mockPerspectivesValue = {
      ...mockPerspectivesValue,
      perspectives: [
        { id: "p1", name: "Sprint View", view: "board" },
        { id: "p2", name: "Backlog", view: "board" },
        { id: "p3", name: "Grid Thing", view: "grid" },
      ],
      activePerspective: { id: "p1", name: "Sprint View", view: "board" },
    };

    renderTabBar();

    // Should show board perspectives only (not grid)
    expect(screen.getByText("Sprint View")).toBeDefined();
    expect(screen.getByText("Backlog")).toBeDefined();
    expect(screen.queryByText("Grid Thing")).toBeNull();
  });

  it("highlights the active perspective tab", () => {
    mockPerspectivesValue = {
      ...mockPerspectivesValue,
      perspectives: [
        { id: "p1", name: "First", view: "board" },
        { id: "p2", name: "Second", view: "board" },
      ],
      activePerspective: { id: "p2", name: "Second", view: "board" },
    };

    renderTabBar();

    const activeTab = screen.getByText("Second").closest("button");
    const inactiveTab = screen.getByText("First").closest("button");

    // Active tab should have distinct styling
    expect(activeTab?.className).toContain("border-primary");
    expect(inactiveTab?.className).not.toContain("border-primary");
  });

  it("switches perspective on tab click", () => {
    mockPerspectivesValue = {
      ...mockPerspectivesValue,
      perspectives: [
        { id: "p1", name: "First", view: "board" },
        { id: "p2", name: "Second", view: "board" },
      ],
      activePerspective: { id: "p1", name: "First", view: "board" },
    };

    renderTabBar();

    fireEvent.click(screen.getByText("Second"));
    expect(mockSetActivePerspectiveId).toHaveBeenCalledWith("p2");
  });

  it("creates a new perspective via '+' button", () => {
    mockPerspectivesValue = {
      ...mockPerspectivesValue,
      perspectives: [{ id: "p1", name: "Default", view: "board" }],
      activePerspective: { id: "p1", name: "Default", view: "board" },
    };

    renderTabBar();

    const addButton = screen.getByRole("button", { name: /add perspective/i });
    fireEvent.click(addButton);

    expect(mockInvoke).toHaveBeenCalledWith(
      "dispatch_command",
      expect.objectContaining({
        cmd: "perspective.save",
        args: expect.objectContaining({
          name: expect.any(String),
          view: "board",
        }),
      }),
    );
  });

  it("renders the '+' button", () => {
    renderTabBar();

    const addButton = screen.getByRole("button", { name: /add perspective/i });
    expect(addButton).toBeDefined();
  });

  it("renders nothing when no active view", () => {
    mockViewsValue = {
      ...mockViewsValue,
      activeView: null as unknown as typeof mockViewsValue.activeView,
    };

    const { container } = renderTabBar();
    // PerspectiveTabBar returns null; TooltipProvider adds no visible DOM.
    expect(container.querySelector("[class]")).toBeNull();
  });

  it("calls useContextMenu handler on right-click", () => {
    mockPerspectivesValue = {
      ...mockPerspectivesValue,
      perspectives: [{ id: "p1", name: "Sprint View", view: "board" }],
      activePerspective: { id: "p1", name: "Sprint View", view: "board" },
    };

    renderTabBar();

    const tab = screen.getByText("Sprint View");
    fireEvent.contextMenu(tab);

    // The useContextMenu hook handler should have been called
    expect(mockContextMenuHandler).toHaveBeenCalled();
  });

  it("does not render a custom React context menu div", () => {
    mockPerspectivesValue = {
      ...mockPerspectivesValue,
      perspectives: [{ id: "p1", name: "Sprint View", view: "board" }],
      activePerspective: { id: "p1", name: "Sprint View", view: "board" },
    };

    renderTabBar();

    const tab = screen.getByText("Sprint View");
    fireEvent.contextMenu(tab);

    // No custom "Rename" or "Delete" buttons should appear in the DOM
    expect(screen.queryByText("Rename")).toBeNull();
    expect(screen.queryByText("Delete")).toBeNull();
  });

  it("starts inline rename on double-click with CM6 editor", () => {
    mockPerspectivesValue = {
      ...mockPerspectivesValue,
      perspectives: [{ id: "p1", name: "Sprint View", view: "board" }],
      activePerspective: { id: "p1", name: "Sprint View", view: "board" },
    };

    const { container } = renderTabBar();

    const tab = screen.getByText("Sprint View");
    fireEvent.doubleClick(tab);

    // After double-click, a CM6 editor should appear (not a plain <input>)
    const cmEditor = container.querySelector(".cm-editor");
    expect(cmEditor).toBeTruthy();
    expect(container.querySelector("input")).toBeNull();
  });

  it("enters inline rename mode for the active perspective when triggerStartRename is called", () => {
    mockPerspectivesValue = {
      ...mockPerspectivesValue,
      perspectives: [
        { id: "p1", name: "First", view: "board" },
        { id: "p2", name: "Second", view: "board" },
      ],
      activePerspective: { id: "p2", name: "Second", view: "board" },
    };

    const { container } = renderTabBar();

    // The formula bar always renders one CM6 editor when a perspective is
    // active — so before triggering rename, we expect exactly one editor.
    expect(container.querySelectorAll(".cm-editor").length).toBe(1);

    // Dispatching ui.entity.startRename (via triggerStartRename — the
    // same code path the AppShell global command handler calls) should put
    // the active tab into rename mode.
    act(() => {
      triggerStartRename();
    });

    // After triggering, a second CM6 editor (the inline rename editor)
    // should have mounted inside the active tab button.
    expect(container.querySelectorAll(".cm-editor").length).toBe(2);

    // The rename editor lives inside the active tab's button, not in the
    // formula bar. Find it via the active tab and verify it shows the
    // active perspective's name.
    const activeTab = screen.getByText("Second").closest("button");
    expect(activeTab).toBeTruthy();
    const renameEditor = activeTab?.querySelector(".cm-editor");
    expect(renameEditor).toBeTruthy();
    const renameContent = renameEditor?.querySelector(".cm-content");
    expect(renameContent?.textContent).toContain("Second");

    // The inactive tab should still render plain text, not an editor
    const inactiveTab = screen.getByText("First").closest("button");
    expect(inactiveTab?.querySelector(".cm-editor")).toBeNull();
  });

  it("triggerStartRename is a no-op when there is no active perspective", () => {
    mockPerspectivesValue = {
      ...mockPerspectivesValue,
      perspectives: [{ id: "p1", name: "First", view: "board" }],
      activePerspective: null,
    };

    const { container } = renderTabBar();

    // With no active perspective, the formula bar is also hidden, so there
    // should be zero CM6 editors in the tab bar.
    expect(container.querySelectorAll(".cm-editor").length).toBe(0);

    act(() => {
      triggerStartRename();
    });

    // Still zero — no active perspective means no rename target
    expect(container.querySelectorAll(".cm-editor").length).toBe(0);
  });

  it("renders CM6 editor with the perspective name as initial value", () => {
    mockPerspectivesValue = {
      ...mockPerspectivesValue,
      perspectives: [{ id: "p1", name: "My View", view: "board" }],
      activePerspective: { id: "p1", name: "My View", view: "board" },
    };

    const { container } = renderTabBar();

    const tab = screen.getByText("My View");
    fireEvent.doubleClick(tab);

    // CM6 renders the text inside .cm-content
    const cmContent = container.querySelector(".cm-content");
    expect(cmContent?.textContent).toContain("My View");
  });

  it("shows a tooltip on hover of the add-perspective button", async () => {
    renderTabBar(0);

    const addButton = screen.getByRole("button", { name: /add perspective/i });

    // Hover the button to trigger the Radix tooltip.
    await act(async () => {
      fireEvent.pointerMove(addButton, { clientX: 10, clientY: 10 });
      fireEvent.mouseEnter(addButton);
      // Allow Radix tooltip to open (even with 0 delay it schedules async).
      await new Promise((r) => setTimeout(r, 100));
    });

    // The tooltip content should be visible.
    const tooltip = screen.getByRole("tooltip");
    expect(tooltip).toBeDefined();
    expect(tooltip.textContent).toBe("New perspective");
  });

  it("does not have an HTML title attribute on the add-perspective button", () => {
    renderTabBar();

    const addButton = screen.getByRole("button", { name: /add perspective/i });
    expect(addButton.getAttribute("title")).toBeNull();
  });

  // =========================================================================
  // Rename integration — Enter/Escape across keymap modes
  // =========================================================================

  /**
   * Opens the inline rename editor for a perspective named "Original" and
   * returns the CM6 EditorView (via EditorView.findFromDOM) so tests can
   * modify the document before pressing Enter/Escape.
   */
  async function setupRenameEditor(keymapMode: string) {
    mockKeymapMode = keymapMode;
    mockPerspectivesValue = {
      ...mockPerspectivesValue,
      perspectives: [{ id: "p1", name: "Original", view: "board" }],
      activePerspective: { id: "p1", name: "Original", view: "board" },
    };
    const result = renderTabBar();
    const tab = screen.getByText("Original");
    fireEvent.doubleClick(tab);

    const cmEditor = result.container.querySelector(
      ".cm-editor",
    ) as HTMLElement;
    expect(cmEditor).toBeTruthy();

    // Get the CM6 EditorView — same pattern as entity-card.test.tsx
    const { EditorView } = await import("@codemirror/view");
    const view = EditorView.findFromDOM(cmEditor);
    expect(view).toBeTruthy();

    return { ...result, view: view!, cmEditor };
  }

  /** Replace the CM6 document text and wait for onChange to propagate. */
  async function replaceDocText(
    view: import("@codemirror/view").EditorView,
    text: string,
  ) {
    await act(async () => {
      view.dispatch({
        changes: { from: 0, to: view.state.doc.length, insert: text },
      });
      // Allow onChange listener tick to propagate
      await new Promise((r) => setTimeout(r, 20));
    });
  }

  /** Dispatch a keydown on the given target and wait for effects. */
  async function pressKey(target: HTMLElement, key: string) {
    await act(async () => {
      target.dispatchEvent(
        new KeyboardEvent("keydown", {
          key,
          bubbles: true,
          cancelable: true,
        }),
      );
      await new Promise((r) => setTimeout(r, 50));
    });
  }

  /** Expected mockInvoke shape for a successful rename dispatch. */
  const renameCall = (newName: string) =>
    expect.objectContaining({
      cmd: "perspective.rename",
      args: expect.objectContaining({
        id: "p1",
        new_name: newName,
      }),
    });

  it("CUA rename: Enter after text change dispatches perspective.rename", async () => {
    const { view, container } = await setupRenameEditor("cua");
    await replaceDocText(view, "New Name");
    mockInvoke.mockClear();

    const cmContent = container.querySelector(".cm-content") as HTMLElement;
    await pressKey(cmContent, "Enter");

    // Rename editor is gone; only the formula bar's CM6 editor remains
    expect(container.querySelectorAll(".cm-editor").length).toBe(1);
    expect(mockInvoke).toHaveBeenCalledWith(
      "dispatch_command",
      renameCall("New Name"),
    );
  });

  it("CUA rename: Escape cancels without dispatching rename", async () => {
    const { view, container } = await setupRenameEditor("cua");
    await replaceDocText(view, "New Name");
    mockInvoke.mockClear();

    const cmContent = container.querySelector(".cm-content") as HTMLElement;
    await pressKey(cmContent, "Escape");

    // Rename editor is gone; only the formula bar's CM6 editor remains
    expect(container.querySelectorAll(".cm-editor").length).toBe(1);
    expect(mockInvoke).not.toHaveBeenCalledWith(
      "dispatch_command",
      renameCall("New Name"),
    );
  });

  it("vim rename: Enter after text change dispatches perspective.rename", async () => {
    const { view, container } = await setupRenameEditor("vim");
    await replaceDocText(view, "New Name");
    mockInvoke.mockClear();

    const cmContent = container.querySelector(".cm-content") as HTMLElement;
    await pressKey(cmContent, "Enter");

    // Rename editor is gone; only the formula bar's CM6 editor remains
    expect(container.querySelectorAll(".cm-editor").length).toBe(1);
    expect(mockInvoke).toHaveBeenCalledWith(
      "dispatch_command",
      renameCall("New Name"),
    );
  });

  it("vim rename: Escape after text change commits (dispatches rename)", async () => {
    const { view, container } = await setupRenameEditor("vim");
    await replaceDocText(view, "New Name");
    mockInvoke.mockClear();

    // Vim Escape from normal mode routes to commitAndExit, not cancel
    const cmContent = container.querySelector(".cm-content") as HTMLElement;
    await pressKey(cmContent, "Escape");

    // Rename editor is gone; only the formula bar's CM6 editor remains
    expect(container.querySelectorAll(".cm-editor").length).toBe(1);
    expect(mockInvoke).toHaveBeenCalledWith(
      "dispatch_command",
      renameCall("New Name"),
    );
  });

  it("emacs rename: Enter after text change dispatches perspective.rename", async () => {
    const { view, container } = await setupRenameEditor("emacs");
    await replaceDocText(view, "New Name");
    mockInvoke.mockClear();

    const cmContent = container.querySelector(".cm-content") as HTMLElement;
    await pressKey(cmContent, "Enter");

    // Rename editor is gone; only the formula bar's CM6 editor remains
    expect(container.querySelectorAll(".cm-editor").length).toBe(1);
    expect(mockInvoke).toHaveBeenCalledWith(
      "dispatch_command",
      renameCall("New Name"),
    );
  });

  it("emacs rename: Escape cancels without dispatching rename", async () => {
    const { view, container } = await setupRenameEditor("emacs");
    await replaceDocText(view, "New Name");
    mockInvoke.mockClear();

    const cmContent = container.querySelector(".cm-content") as HTMLElement;
    await pressKey(cmContent, "Escape");

    // Rename editor is gone; only the formula bar's CM6 editor remains
    expect(container.querySelectorAll(".cm-editor").length).toBe(1);
    expect(mockInvoke).not.toHaveBeenCalledWith(
      "dispatch_command",
      renameCall("New Name"),
    );
  });

  // =========================================================================
  // Filter formula bar — always-visible CM6 editor in the right of the tab bar
  // =========================================================================

  describe("Filter formula bar", () => {
    it("renders filter editor inline (not in a popover) when a perspective is active", () => {
      mockPerspectivesValue = {
        ...mockPerspectivesValue,
        perspectives: [{ id: "p1", name: "Sprint View", view: "board" }],
        activePerspective: { id: "p1", name: "Sprint View", view: "board" },
      };

      renderTabBar();

      // FilterEditor should be present without clicking anything
      expect(screen.getByTestId("filter-editor")).toBeDefined();
    });

    it("does not render filter editor when no perspective is active", () => {
      mockPerspectivesValue = {
        ...mockPerspectivesValue,
        perspectives: [{ id: "p1", name: "Sprint View", view: "board" }],
        activePerspective: null,
      };

      renderTabBar();

      expect(screen.queryByTestId("filter-editor")).toBeNull();
    });

    it("shows cm-placeholder in formula bar when filter is empty", () => {
      mockPerspectivesValue = {
        ...mockPerspectivesValue,
        perspectives: [{ id: "p1", name: "Sprint View", view: "board" }],
        activePerspective: { id: "p1", name: "Sprint View", view: "board" },
      };

      const { container } = renderTabBar();

      const placeholder = container.querySelector(".cm-placeholder");
      expect(placeholder).toBeTruthy();
    });

    it("filter icon button is highlighted (text-primary) when active perspective has a filter", () => {
      mockPerspectivesValue = {
        ...mockPerspectivesValue,
        perspectives: [
          { id: "p1", name: "Sprint View", view: "board", filter: "#bug" },
        ],
        activePerspective: {
          id: "p1",
          name: "Sprint View",
          view: "board",
          filter: "#bug",
        },
      };

      renderTabBar();

      // Use exact match to distinguish "Filter" (tab icon) from "Clear filter" (formula bar)
      const filterButton = screen.getByRole("button", { name: "Filter" });
      expect(filterButton.className).toContain("text-primary");
    });

    it("filter button click does not open a popover", () => {
      mockPerspectivesValue = {
        ...mockPerspectivesValue,
        perspectives: [{ id: "p1", name: "Sprint View", view: "board" }],
        activePerspective: { id: "p1", name: "Sprint View", view: "board" },
      };

      renderTabBar();

      const filterButton = screen.getByRole("button", { name: /filter/i });
      fireEvent.click(filterButton);

      // No Radix popover/dialog should appear in the DOM
      expect(screen.queryByRole("dialog")).toBeNull();
    });

    it("clicking the filter button on the active tab focuses the formula bar CM6 editor", () => {
      mockPerspectivesValue = {
        ...mockPerspectivesValue,
        perspectives: [{ id: "p1", name: "Sprint View", view: "board" }],
        activePerspective: { id: "p1", name: "Sprint View", view: "board" },
      };

      const { container } = renderTabBar();

      const filterButton = screen.getByRole("button", { name: "Filter" });
      fireEvent.click(filterButton);

      // After clicking filter button, the CM6 content area should have focus
      const cmContent = container.querySelector(".cm-content") as HTMLElement;
      expect(cmContent).toBeTruthy();
      expect(document.activeElement).toBe(cmContent);
    });

    it("clicking the formula bar container area focuses the CM6 editor", () => {
      mockPerspectivesValue = {
        ...mockPerspectivesValue,
        perspectives: [{ id: "p1", name: "Sprint View", view: "board" }],
        activePerspective: { id: "p1", name: "Sprint View", view: "board" },
      };

      const { container } = renderTabBar();

      // Click the formula bar container (outside the CM editor itself)
      const formulaBar = container.querySelector(
        '[data-testid="filter-formula-bar"]',
      ) as HTMLElement;
      expect(formulaBar).toBeTruthy();
      fireEvent.click(formulaBar);

      const cmContent = container.querySelector(".cm-content") as HTMLElement;
      expect(document.activeElement).toBe(cmContent);
    });

    it("formula bar container has cursor-text class to signal it is editable", () => {
      mockPerspectivesValue = {
        ...mockPerspectivesValue,
        perspectives: [{ id: "p1", name: "Sprint View", view: "board" }],
        activePerspective: { id: "p1", name: "Sprint View", view: "board" },
      };

      const { container } = renderTabBar();

      const formulaBar = container.querySelector(
        '[data-testid="filter-formula-bar"]',
      );
      expect(formulaBar).toBeTruthy();
      expect(formulaBar?.className).toContain("cursor-text");
    });
  });

  // =========================================================================
  // Rect regression — first-paint kernel-stored rects must be non-zero
  // =========================================================================
  //
  // Source-of-truth assertion for kanban card `01KQ9Z56M556DQHYMA502B9FKB`'s
  // rect-regression criterion: none of the perspective tabs' kernel-stored
  // rects may be zero-sized at first paint, including the active tab whose
  // box is wider due to inline `<FilterFocusButton>` + `<GroupPopoverButton>`
  // chrome.
  //
  // Beam search picks candidates by their rect center / left-edge ordering;
  // a zero-sized rect from a tab leaf would either tie with siblings (making
  // the picks non-deterministic) or fail the in-beam filter outright (making
  // the next tab unreachable). Both manifestations were on the user's
  // candidate-cause list — pin first-paint non-zero rects so a future
  // measurement-timing regression surfaces immediately.
  //
  // Mounts the bar inside the production-shaped spatial-nav stack so each
  // tab's `<FocusScope>` registers itself with the kernel via
  // `spatial_register_scope`. The recorded `rect` argument is the rect
  // snapshotted by `<FocusScope>`'s mount effect via
  // `node.getBoundingClientRect()` — the kernel's source-of-truth at first
  // paint.
  // =========================================================================

  describe("rect regression — first paint", () => {
    it("kernel-stored rects of all perspective tabs are non-zero on first paint", async () => {
      mockPerspectivesValue = {
        ...mockPerspectivesValue,
        perspectives: [
          { id: "p1", name: "Sprint", view: "board" },
          { id: "p2", name: "Backlog", view: "board" },
          { id: "p3", name: "Archive", view: "board" },
        ],
        // p2 is active — its leaf is widened by the inline
        // `<FilterFocusButton>` + `<GroupPopoverButton>` chrome that only
        // the active tab renders. The regression guard must hold for the
        // wider leaf as well as the flanking ones.
        activePerspective: { id: "p2", name: "Backlog", view: "board" },
      };

      // Lazy-import to avoid pulling the spatial-nav stack into the simpler
      // tests above (and to keep this rect-regression block self-contained).
      const { FocusLayer } = await import("./focus-layer");
      const { SpatialFocusProvider } =
        await import("@/lib/spatial-focus-context");
      const { EntityFocusProvider: EFP } =
        await import("@/lib/entity-focus-context");
      const { asSegment } = await import("@/types/spatial");

      const { unmount } = render(
        <SpatialFocusProvider>
          <FocusLayer name={asSegment("window")}>
            <EFP>
              <TooltipProvider delayDuration={100}>
                <PerspectiveTabBar />
              </TooltipProvider>
            </EFP>
          </FocusLayer>
        </SpatialFocusProvider>,
      );

      // Flush the register effects scheduled by `<FocusScope>` on mount.
      await act(async () => {
        await Promise.resolve();
      });
      await act(async () => {
        await Promise.resolve();
      });

      // Collect the rect for each perspective tab from the
      // `spatial_register_scope` invocation argument bag. After the
      // iteration-2 reshape the per-tab wrapper is a `<FocusScope>`, not
      // a `<FocusScope>` leaf — the rect lives on the zone now and the
      // inner name / filter / group leaves are smaller children inside
      // it.
      type RectArg = {
        x: number;
        y: number;
        width: number;
        height: number;
      };
      const tabRects = new Map<string, RectArg>();
      for (const call of mockInvoke.mock.calls) {
        if (call[0] !== "spatial_register_scope") continue;
        const args = call[1] as { segment: string; rect: RectArg };
        if (typeof args?.segment !== "string") continue;
        if (!args.segment.startsWith("perspective_tab:")) continue;
        tabRects.set(args.segment, args.rect);
      }

      expect(
        tabRects.size,
        "all three perspective tabs must register a rect with the kernel",
      ).toBe(3);
      expect(tabRects.has("perspective_tab:p1")).toBe(true);
      expect(tabRects.has("perspective_tab:p2")).toBe(true);
      expect(tabRects.has("perspective_tab:p3")).toBe(true);

      // Every tab's rect must be non-zero in both dimensions. A leaf with
      // a zero-width or zero-height rect is invisible to beam search.
      for (const [moniker, rect] of tabRects) {
        expect(
          rect.width,
          `${moniker} rect.width must be > 0 at first paint (got ${rect.width})`,
        ).toBeGreaterThan(0);
        expect(
          rect.height,
          `${moniker} rect.height must be > 0 at first paint (got ${rect.height})`,
        ).toBeGreaterThan(0);
      }

      unmount();
    });
  });
});
