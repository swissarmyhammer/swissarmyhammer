import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import type { ReactNode } from "react";

// Mock Tauri APIs before importing any modules that use them.
// eslint-disable-next-line @typescript-eslint/no-explicit-any
const mockInvoke = vi.fn((..._args: any[]) => Promise.resolve(null));
vi.mock("@tauri-apps/api/core", () => ({
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  invoke: (...args: any[]) => mockInvoke(...args),
}));

// eslint-disable-next-line @typescript-eslint/no-explicit-any
let listenCallbacks: Record<string, (event: { payload: any }) => void> = {};
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(
    (
      event: string,
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      cb: (e: { payload: any }) => void,
    ) => {
      listenCallbacks[event] = cb;
      return Promise.resolve(() => {});
    },
  ),
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

// Mock UIState so we can control active_perspective_id.
let mockUIState = {
  keymap_mode: "cua",
  scope_chain: [],
  open_boards: [],
  has_clipboard: false,
  clipboard_entity_type: null,
  windows: {} as Record<string, { active_perspective_id: string }>,
  recent_boards: [],
};

vi.mock("./ui-state-context", () => ({
  useUIState: () => mockUIState,
}));

vi.mock("./views-context", () => ({
  useViews: () => ({
    views: [{ id: "board-1", name: "Board", kind: "board" }],
    activeView: { id: "board-1", name: "Board", kind: "board" },
    setActiveViewId: vi.fn(),
    refresh: vi.fn(() => Promise.resolve()),
  }),
}));

import { PerspectiveProvider, usePerspectives } from "./perspective-context";
import {
  CommandScopeProvider,
  ActiveBoardPathProvider,
} from "@/lib/command-scope";
import type { PerspectiveDef } from "@/types/kanban";

/** Build a minimal PerspectiveDef for test use. */
function makePerspective(id: string, name: string): PerspectiveDef {
  return { id, name, view: "board" };
}

/** Wrapper that renders PerspectiveProvider inside a CommandScopeProvider
 *  with a window moniker, matching the real App.tsx tree. */
function wrapper({ children }: { children: ReactNode }) {
  return (
    <CommandScopeProvider commands={[]} moniker="window:main">
      <ActiveBoardPathProvider value="/tmp/test/.kanban">
        <PerspectiveProvider>{children}</PerspectiveProvider>
      </ActiveBoardPathProvider>
    </CommandScopeProvider>
  );
}

describe("PerspectiveProvider", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    listenCallbacks = {};
    mockUIState = {
      keymap_mode: "cua",
      scope_chain: [],
      open_boards: [],
      has_clipboard: false,
      clipboard_entity_type: null,
      windows: {},
      recent_boards: [],
    };
    // Default: perspective.list returns empty (wrapped in dispatch envelope)
    mockInvoke.mockResolvedValue({
      result: { perspectives: [], count: 0 },
      undoable: false,
    });
  });

  it("provides empty perspectives list as default", async () => {
    const { result } = renderHook(() => usePerspectives(), { wrapper });
    await act(async () => {});
    expect(result.current.perspectives).toEqual([]);
    expect(result.current.activePerspective).toBeNull();
  });

  it("fetches perspectives on mount via perspective.list command", async () => {
    const ps = [makePerspective("p1", "Sprint View")];
    mockInvoke.mockResolvedValue({
      result: { perspectives: ps, count: 1 },
      undoable: false,
    });

    const { result } = renderHook(() => usePerspectives(), { wrapper });
    await act(async () => {});

    expect(mockInvoke).toHaveBeenCalledWith("dispatch_command", {
      cmd: "perspective.list",
      scopeChain: ["window:main"],
      boardPath: "/tmp/test/.kanban",
    });
    expect(result.current.perspectives).toHaveLength(1);
    expect(result.current.perspectives[0].name).toBe("Sprint View");
  });

  it("activePerspective falls back to first perspective when no id set", async () => {
    const ps = [
      makePerspective("p1", "First"),
      makePerspective("p2", "Second"),
    ];
    mockInvoke.mockResolvedValue({
      result: { perspectives: ps, count: 2 },
      undoable: false,
    });

    const { result } = renderHook(() => usePerspectives(), { wrapper });
    await act(async () => {});

    // No active_perspective_id in UIState → falls back to first
    expect(result.current.activePerspective?.id).toBe("p1");
  });

  it("activePerspective uses active_perspective_id from UIState", async () => {
    mockUIState = {
      ...mockUIState,
      windows: { main: { active_perspective_id: "p2" } },
    };

    const ps = [
      makePerspective("p1", "First"),
      makePerspective("p2", "Second"),
    ];
    mockInvoke.mockResolvedValue({
      result: { perspectives: ps, count: 2 },
      undoable: false,
    });

    const { result } = renderHook(() => usePerspectives(), { wrapper });
    await act(async () => {});

    expect(result.current.activePerspective?.id).toBe("p2");
    expect(result.current.activePerspective?.name).toBe("Second");
  });

  it("setActivePerspectiveId dispatches ui.perspective.set to backend", async () => {
    mockInvoke.mockResolvedValue({
      result: { perspectives: [], count: 0 },
      undoable: false,
    });

    const { result } = renderHook(() => usePerspectives(), { wrapper });
    await act(async () => {});

    // Clear the mount-time invoke call so we can assert cleanly
    mockInvoke.mockClear();
    mockInvoke.mockResolvedValue(null);

    await act(async () => {
      result.current.setActivePerspectiveId("p1");
      // Let the fire-and-forget promise settle
      await new Promise((r) => setTimeout(r, 0));
    });

    expect(mockInvoke).toHaveBeenCalledWith("dispatch_command", {
      cmd: "ui.perspective.set",
      args: { perspective_id: "p1" },
      scopeChain: ["window:main"],
      boardPath: "/tmp/test/.kanban",
    });
  });

  it("refresh re-fetches the perspectives list", async () => {
    mockInvoke.mockResolvedValue({
      result: { perspectives: [], count: 0 },
      undoable: false,
    });

    const { result } = renderHook(() => usePerspectives(), { wrapper });
    await act(async () => {});
    expect(result.current.perspectives).toHaveLength(0);

    // Now return a perspective on next fetch
    mockInvoke.mockResolvedValue({
      result: { perspectives: [makePerspective("p1", "New")], count: 1 },
      undoable: false,
    });

    await act(async () => {
      await result.current.refresh();
    });

    expect(result.current.perspectives).toHaveLength(1);
  });

  it("refreshes on entity-field-changed event for perspective type", async () => {
    mockInvoke.mockResolvedValue({
      result: { perspectives: [], count: 0 },
      undoable: false,
    });

    const { result } = renderHook(() => usePerspectives(), { wrapper });
    await act(async () => {});

    // Set up a perspective to be returned on next fetch
    mockInvoke.mockResolvedValue({
      result: { perspectives: [makePerspective("p1", "Updated")], count: 1 },
      undoable: false,
    });

    await act(async () => {
      listenCallbacks["entity-field-changed"]?.({
        payload: { entity_type: "perspective", id: "p1" },
      });
      await new Promise((r) => setTimeout(r, 0));
    });

    expect(result.current.perspectives).toHaveLength(1);
    expect(result.current.perspectives[0].name).toBe("Updated");
  });

  it("refreshes on entity-created event for perspective type", async () => {
    mockInvoke.mockResolvedValue({
      result: { perspectives: [], count: 0 },
      undoable: false,
    });

    const { result } = renderHook(() => usePerspectives(), { wrapper });
    await act(async () => {});

    mockInvoke.mockResolvedValue({
      result: { perspectives: [makePerspective("p2", "Brand New")], count: 1 },
      undoable: false,
    });

    await act(async () => {
      listenCallbacks["entity-created"]?.({
        payload: { entity_type: "perspective", id: "p2" },
      });
      await new Promise((r) => setTimeout(r, 0));
    });

    expect(result.current.perspectives).toHaveLength(1);
    expect(result.current.perspectives[0].name).toBe("Brand New");
  });

  it("refreshes on entity-removed event for perspective type", async () => {
    const ps = [
      makePerspective("p1", "First"),
      makePerspective("p2", "Second"),
    ];
    mockInvoke.mockResolvedValue({
      result: { perspectives: ps, count: 2 },
      undoable: false,
    });

    const { result } = renderHook(() => usePerspectives(), { wrapper });
    await act(async () => {});
    expect(result.current.perspectives).toHaveLength(2);

    // After removal only one perspective remains
    mockInvoke.mockResolvedValue({
      result: { perspectives: [makePerspective("p1", "First")], count: 1 },
      undoable: false,
    });

    await act(async () => {
      listenCallbacks["entity-removed"]?.({
        payload: { entity_type: "perspective", id: "p2" },
      });
      await new Promise((r) => setTimeout(r, 0));
    });

    expect(result.current.perspectives).toHaveLength(1);
  });

  it("ignores entity events for non-perspective types", async () => {
    mockInvoke.mockResolvedValue({
      result: { perspectives: [], count: 0 },
      undoable: false,
    });

    const { result } = renderHook(() => usePerspectives(), { wrapper });
    await act(async () => {});

    const callCountBefore = mockInvoke.mock.calls.length;

    await act(async () => {
      listenCallbacks["entity-field-changed"]?.({
        payload: { entity_type: "task", id: "t1" },
      });
      await new Promise((r) => setTimeout(r, 0));
    });

    // No additional invoke calls for non-perspective events
    expect(mockInvoke.mock.calls.length).toBe(callCountBefore);
  });

  it("refreshes on board-changed event", async () => {
    mockInvoke.mockResolvedValue({
      result: { perspectives: [], count: 0 },
      undoable: false,
    });

    const { result } = renderHook(() => usePerspectives(), { wrapper });
    await act(async () => {});

    mockInvoke.mockResolvedValue({
      result: {
        perspectives: [makePerspective("p1", "After Board Change")],
        count: 1,
      },
      undoable: false,
    });

    await act(async () => {
      listenCallbacks["board-changed"]?.({ payload: undefined });
      await new Promise((r) => setTimeout(r, 0));
    });

    expect(result.current.perspectives).toHaveLength(1);
    expect(result.current.perspectives[0].name).toBe("After Board Change");
  });

  it("usePerspectives throws outside provider", () => {
    expect(() => renderHook(() => usePerspectives())).toThrow(
      "usePerspectives must be used within PerspectiveProvider",
    );
  });
});
