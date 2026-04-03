import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";

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

// Mock backendDispatch for the "+" button and rename actions.
const mockBackendDispatch = vi.fn(() => Promise.resolve(null));
vi.mock("@/lib/command-scope", async (importOriginal) => {
  const original = await importOriginal<typeof import("@/lib/command-scope")>();
  return {
    ...original,
    backendDispatch: (...args: unknown[]) => mockBackendDispatch(...args),
  };
});

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

import { PerspectiveTabBar } from "./perspective-tab-bar";

describe("PerspectiveTabBar", () => {
  beforeEach(() => {
    vi.clearAllMocks();
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

    render(<PerspectiveTabBar />);

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

    render(<PerspectiveTabBar />);

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

    render(<PerspectiveTabBar />);

    fireEvent.click(screen.getByText("Second"));
    expect(mockSetActivePerspectiveId).toHaveBeenCalledWith("p2");
  });

  it("creates a new perspective via '+' button", () => {
    mockPerspectivesValue = {
      ...mockPerspectivesValue,
      perspectives: [{ id: "p1", name: "Default", view: "board" }],
      activePerspective: { id: "p1", name: "Default", view: "board" },
    };

    render(<PerspectiveTabBar />);

    const addButton = screen.getByRole("button", { name: /add perspective/i });
    fireEvent.click(addButton);

    expect(mockBackendDispatch).toHaveBeenCalledWith(
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
    render(<PerspectiveTabBar />);

    const addButton = screen.getByRole("button", { name: /add perspective/i });
    expect(addButton).toBeDefined();
  });

  it("renders nothing when no active view", () => {
    mockViewsValue = {
      ...mockViewsValue,
      activeView: null as unknown as typeof mockViewsValue.activeView,
    };

    const { container } = render(<PerspectiveTabBar />);
    expect(container.innerHTML).toBe("");
  });

  it("calls useContextMenu handler on right-click", () => {
    mockPerspectivesValue = {
      ...mockPerspectivesValue,
      perspectives: [{ id: "p1", name: "Sprint View", view: "board" }],
      activePerspective: { id: "p1", name: "Sprint View", view: "board" },
    };

    render(<PerspectiveTabBar />);

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

    render(<PerspectiveTabBar />);

    const tab = screen.getByText("Sprint View");
    fireEvent.contextMenu(tab);

    // No custom "Rename" or "Delete" buttons should appear in the DOM
    expect(screen.queryByText("Rename")).toBeNull();
    expect(screen.queryByText("Delete")).toBeNull();
  });

  it("starts inline rename on double-click", () => {
    mockPerspectivesValue = {
      ...mockPerspectivesValue,
      perspectives: [{ id: "p1", name: "Sprint View", view: "board" }],
      activePerspective: { id: "p1", name: "Sprint View", view: "board" },
    };

    render(<PerspectiveTabBar />);

    const tab = screen.getByText("Sprint View");
    fireEvent.doubleClick(tab);

    // After double-click, an input should appear for renaming
    const input = screen.getByDisplayValue("Sprint View");
    expect(input).toBeDefined();
    expect(input.tagName).toBe("INPUT");
  });
});
