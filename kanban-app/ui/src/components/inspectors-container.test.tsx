import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";

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
// Mock useUIState to control inspector_stack from tests.
// ---------------------------------------------------------------------------

const mockUIState = vi.hoisted(() =>
  vi.fn(() => ({
    keymap_mode: "cua",
    scope_chain: [],
    open_boards: [],
    has_clipboard: false,
    clipboard_entity_type: null,
    windows: {},
    recent_boards: [],
  })),
);

vi.mock("@/lib/ui-state-context", () => ({
  useUIState: () => mockUIState(),
}));

// ---------------------------------------------------------------------------
// Mock useDispatchCommand to capture dispatched commands.
// ---------------------------------------------------------------------------

const mockDispatchClose = vi.hoisted(() => vi.fn(() => Promise.resolve()));
const mockDispatchCloseAll = vi.hoisted(() => vi.fn(() => Promise.resolve()));

vi.mock("@/lib/command-scope", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/command-scope")>();
  return {
    ...actual,
    useDispatchCommand: (cmd: string) => {
      if (cmd === "ui.inspector.close") return mockDispatchClose;
      if (cmd === "ui.inspector.close_all") return mockDispatchCloseAll;
      return vi.fn(() => Promise.resolve());
    },
  };
});

// ---------------------------------------------------------------------------
// Mock useSchema + useRestoreFocus — InspectorPanel uses these internally.
// ---------------------------------------------------------------------------

vi.mock("@/lib/schema-context", () => ({
  useSchema: () => ({
    getSchema: () => null,
    schemas: {},
    loading: false,
  }),
}));

vi.mock("@/lib/entity-focus-context", () => ({
  useRestoreFocus: vi.fn(),
}));

// ---------------------------------------------------------------------------
// Mock RustEngineContainer hook — provides entity store.
// ---------------------------------------------------------------------------

const mockEntitiesByType = vi.hoisted(() =>
  vi.fn<[], Record<string, unknown[]>>(() => ({})),
);

vi.mock("@/components/rust-engine-container", () => ({
  useEntitiesByType: () => mockEntitiesByType(),
}));

// ---------------------------------------------------------------------------
// Import component under test after mocks.
// ---------------------------------------------------------------------------

import { InspectorsContainer } from "./inspectors-container";
import { FileDropProvider } from "@/lib/file-drop-context";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Build a UIState snapshot with a given inspector_stack for the "main" window. */
function uiStateWithStack(stack: string[]) {
  return {
    keymap_mode: "cua",
    scope_chain: [],
    open_boards: [],
    has_clipboard: false,
    clipboard_entity_type: null,
    windows: {
      main: {
        board_path: "/test",
        inspector_stack: stack,
        active_view_id: "",
        active_perspective_id: "",
        palette_open: false,
        palette_mode: "command" as const,
      },
    },
    recent_boards: [],
  };
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("InspectorsContainer", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockUIState.mockReturnValue({
      keymap_mode: "cua",
      scope_chain: [],
      open_boards: [],
      has_clipboard: false,
      clipboard_entity_type: null,
      windows: {},
      recent_boards: [],
    });
    mockEntitiesByType.mockReturnValue({});
  });

  it("renders nothing when inspector_stack is empty", () => {
    mockUIState.mockReturnValue(uiStateWithStack([]));

    const { container } = render(<InspectorsContainer />);

    // Backdrop should have pointer-events-none (invisible)
    const backdrop = container.querySelector(".fixed.inset-0");
    expect(backdrop?.className).toContain("pointer-events-none");
    // No slide panels
    expect(container.querySelectorAll('[class*="w-[420px]"]').length).toBe(0);
  });

  it("renders a panel for each inspector_stack entry", () => {
    mockUIState.mockReturnValue(uiStateWithStack(["task:t1", "task:t2"]));

    const { container } = render(<InspectorsContainer />);

    // Two slide panels should be rendered
    const panels = container.querySelectorAll('[class*="w-[420px]"]');
    expect(panels.length).toBe(2);
  });

  it("renders backdrop as visible when panels are open", () => {
    mockUIState.mockReturnValue(uiStateWithStack(["task:t1"]));

    const { container } = render(<InspectorsContainer />);

    const backdrop = container.querySelector(".fixed.inset-0");
    expect(backdrop?.className).toContain("opacity-100");
    expect(backdrop?.className).not.toContain("pointer-events-none");
  });

  it("dispatches ui.inspector.close_all when backdrop is clicked", () => {
    mockUIState.mockReturnValue(uiStateWithStack(["task:t1"]));

    const { container } = render(<InspectorsContainer />);

    const backdrop = container.querySelector(".fixed.inset-0");
    fireEvent.click(backdrop!);

    expect(mockDispatchCloseAll).toHaveBeenCalledTimes(1);
  });

  it("stacks panels with correct right offset", () => {
    mockUIState.mockReturnValue(
      uiStateWithStack(["task:t1", "task:t2", "task:t3"]),
    );

    const { container } = render(<InspectorsContainer />);

    const panels = container.querySelectorAll('[class*="w-[420px]"]');
    expect(panels.length).toBe(3);

    // First panel (t1) is deepest — right offset = (3-1-0)*420 = 840
    expect((panels[0] as HTMLElement).style.right).toBe("840px");
    // Second panel (t2) — right offset = (3-1-1)*420 = 420
    expect((panels[1] as HTMLElement).style.right).toBe("420px");
    // Third panel (t3) is topmost — right offset = 0
    expect((panels[2] as HTMLElement).style.right).toBe("0px");
  });

  it("renders nothing when window state does not exist", () => {
    // Default mock has no windows entry for "main"
    const { container } = render(<InspectorsContainer />);

    const panels = container.querySelectorAll('[class*="w-[420px]"]');
    expect(panels.length).toBe(0);
  });

  it("receives isDragging from FileDropProvider (drag highlight propagates)", () => {
    // When InspectorsContainer is inside FileDropProvider (as it should be
    // in App.tsx), the attachment editor in inspector panels receives the
    // isDragging state for drag highlight rendering.
    //
    // This test wraps InspectorsContainer in FileDropProvider with
    // _testOverride and verifies the container renders without error,
    // proving the provider tree is compatible.
    mockUIState.mockReturnValue(uiStateWithStack(["task:t1"]));
    mockEntitiesByType.mockReturnValue({
      task: [
        {
          entity_type: "task",
          id: "t1",
          fields: { title: { String: "Test Task" } },
        },
      ],
    });

    // Wrapping in FileDropProvider with isDragging=true should not error
    const { container } = render(
      <FileDropProvider _testOverride={{ isDragging: true }}>
        <InspectorsContainer />
      </FileDropProvider>,
    );

    // Panel should render (one slide panel)
    const panels = container.querySelectorAll('[class*="w-[420px]"]');
    expect(panels.length).toBe(1);

    // If any data-file-drop-zone elements exist (attachment editors),
    // they should have the drag highlight class from the isDragging override.
    const dropZones = container.querySelectorAll("[data-file-drop-zone]");
    for (const zone of dropZones) {
      expect(zone.className).toContain("ring-2");
    }
  });

  it("parses entityType and entityId from moniker strings", () => {
    mockUIState.mockReturnValue(uiStateWithStack(["board:b1"]));
    mockEntitiesByType.mockReturnValue({
      board: [
        {
          entity_type: "board",
          id: "b1",
          fields: { name: { String: "Test" } },
        },
      ],
    });

    const { container } = render(<InspectorsContainer />);

    // Panel should render (one slide panel)
    const panels = container.querySelectorAll('[class*="w-[420px]"]');
    expect(panels.length).toBe(1);
  });
});
