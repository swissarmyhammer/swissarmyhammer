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

// Mock the perspectives context so we can control perspectives list.
let mockPerspectivesValue = {
  perspectives: [] as Array<{ id: string; name: string; view: string }>,
  activePerspective: null as {
    id: string;
    name: string;
    view: string;
  } | null,
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

// Mock useSchema — returns empty schema by default.
vi.mock("@/lib/schema-context", () => ({
  useSchema: () => ({
    getSchema: () => ({ entity: { name: "task", fields: [] }, fields: [] }),
    getFieldDef: () => undefined,
    getEntityCommands: () => [],
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

import { PerspectiveTabBar } from "./perspective-tab-bar";

/** Renders PerspectiveTabBar inside the required TooltipProvider. */
function renderTabBar(delayDuration = 100) {
  return render(
    <TooltipProvider delayDuration={delayDuration}>
      <PerspectiveTabBar />
    </TooltipProvider>,
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
  async function replaceDocText(view: import("@codemirror/view").EditorView, text: string) {
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

    expect(container.querySelector(".cm-editor")).toBeNull();
    expect(mockInvoke).toHaveBeenCalledWith("dispatch_command", renameCall("New Name"));
  });

  it("CUA rename: Escape cancels without dispatching rename", async () => {
    const { view, container } = await setupRenameEditor("cua");
    await replaceDocText(view, "New Name");
    mockInvoke.mockClear();

    const cmContent = container.querySelector(".cm-content") as HTMLElement;
    await pressKey(cmContent, "Escape");

    expect(container.querySelector(".cm-editor")).toBeNull();
    expect(mockInvoke).not.toHaveBeenCalledWith("dispatch_command", renameCall("New Name"));
  });

  it("vim rename: Enter after text change dispatches perspective.rename", async () => {
    const { view, container } = await setupRenameEditor("vim");
    await replaceDocText(view, "New Name");
    mockInvoke.mockClear();

    const cmContent = container.querySelector(".cm-content") as HTMLElement;
    await pressKey(cmContent, "Enter");

    expect(container.querySelector(".cm-editor")).toBeNull();
    expect(mockInvoke).toHaveBeenCalledWith("dispatch_command", renameCall("New Name"));
  });

  it("vim rename: Escape after text change commits (dispatches rename)", async () => {
    const { view, container } = await setupRenameEditor("vim");
    await replaceDocText(view, "New Name");
    mockInvoke.mockClear();

    // Vim Escape from normal mode routes to commitAndExit, not cancel
    const cmContent = container.querySelector(".cm-content") as HTMLElement;
    await pressKey(cmContent, "Escape");

    expect(container.querySelector(".cm-editor")).toBeNull();
    expect(mockInvoke).toHaveBeenCalledWith("dispatch_command", renameCall("New Name"));
  });

  it("emacs rename: Enter after text change dispatches perspective.rename", async () => {
    const { view, container } = await setupRenameEditor("emacs");
    await replaceDocText(view, "New Name");
    mockInvoke.mockClear();

    const cmContent = container.querySelector(".cm-content") as HTMLElement;
    await pressKey(cmContent, "Enter");

    expect(container.querySelector(".cm-editor")).toBeNull();
    expect(mockInvoke).toHaveBeenCalledWith("dispatch_command", renameCall("New Name"));
  });

  it("emacs rename: Escape cancels without dispatching rename", async () => {
    const { view, container } = await setupRenameEditor("emacs");
    await replaceDocText(view, "New Name");
    mockInvoke.mockClear();

    const cmContent = container.querySelector(".cm-content") as HTMLElement;
    await pressKey(cmContent, "Escape");

    expect(container.querySelector(".cm-editor")).toBeNull();
    expect(mockInvoke).not.toHaveBeenCalledWith("dispatch_command", renameCall("New Name"));
  });
});
