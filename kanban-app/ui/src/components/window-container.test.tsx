import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, act, waitFor } from "@testing-library/react";

// ---------------------------------------------------------------------------
// Mock Tauri APIs before importing components that use them.
// vi.hoisted runs before vi.mock hoisting so the references are valid.
// ---------------------------------------------------------------------------

type ListenCallback = (event: { payload: unknown }) => void;

const { mockInvoke, mockListen, mockWindowListen, listeners, windowListeners } =
  vi.hoisted(() => {
    const listeners = new Map<string, ListenCallback[]>();
    const windowListeners = new Map<string, ListenCallback[]>();
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const mockInvoke = vi.fn((..._args: any[]): Promise<any> => {
      const cmd = _args[0] as string;
      if (cmd === "get_ui_state")
        return Promise.resolve({
          palette_open: false,
          palette_mode: "command",
          keymap_mode: "cua",
          scope_chain: [],
          open_boards: [],
          windows: {},
          recent_boards: [],
        });
      if (cmd === "list_schemas") return Promise.resolve([]);
      if (cmd === "list_open_boards") return Promise.resolve([]);
      return Promise.resolve(null);
    });
    const mockListen = vi.fn(
      (eventName: string, cb: ListenCallback): Promise<() => void> => {
        const cbs = listeners.get(eventName) ?? [];
        cbs.push(cb);
        listeners.set(eventName, cbs);
        return Promise.resolve(() => {
          const arr = listeners.get(eventName);
          if (arr) {
            const idx = arr.indexOf(cb);
            if (idx >= 0) arr.splice(idx, 1);
          }
        });
      },
    );
    const mockWindowListen = vi.fn(
      (eventName: string, cb: ListenCallback): Promise<() => void> => {
        const cbs = windowListeners.get(eventName) ?? [];
        cbs.push(cb);
        windowListeners.set(eventName, cbs);
        return Promise.resolve(() => {
          const arr = windowListeners.get(eventName);
          if (arr) {
            const idx = arr.indexOf(cb);
            if (idx >= 0) arr.splice(idx, 1);
          }
        });
      },
    );
    return {
      mockInvoke,
      mockListen,
      mockWindowListen,
      listeners,
      windowListeners,
    };
  });

vi.mock("@tauri-apps/api/core", () => ({
  invoke: mockInvoke,
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: mockListen,
}));
vi.mock("@tauri-apps/api/webviewWindow", () => ({
  getCurrentWebviewWindow: () => ({
    label: "main",
    listen: vi.fn(() => Promise.resolve(() => {})),
  }),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    label: "main",
    listen: mockWindowListen,
  }),
}));

// Import after mocks
import { RustEngineContainer } from "./rust-engine-container";
import {
  WindowContainer,
  useOpenBoards,
  useActiveBoardPath,
  useHandleSwitchBoard,
  useWindowLoading,
  useBoardData,
} from "./window-container";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Emit a global Tauri event to registered listeners. */
function emitTauriEvent(eventName: string, payload: unknown) {
  const cbs = listeners.get(eventName) ?? [];
  for (const cb of cbs) {
    cb({ payload });
  }
}

/** Emit a window-scoped Tauri event to registered listeners. */
function emitWindowEvent(eventName: string, payload: unknown) {
  const cbs = windowListeners.get(eventName) ?? [];
  for (const cb of cbs) {
    cb({ payload });
  }
}

// ---------------------------------------------------------------------------
// Probe components
// ---------------------------------------------------------------------------

/** Renders the active board path from WindowContainer context. */
function ActiveBoardPathProbe() {
  const activeBoardPath = useActiveBoardPath();
  return (
    <span data-testid="active-board-path">{activeBoardPath ?? "none"}</span>
  );
}

/** Renders the open boards count from WindowContainer context. */
function OpenBoardsProbe() {
  const openBoards = useOpenBoards();
  return <span data-testid="open-boards-count">{openBoards.length}</span>;
}

/** Renders a switch board button using WindowContainer context. */
function SwitchBoardProbe() {
  const handleSwitchBoard = useHandleSwitchBoard();
  return (
    <button
      data-testid="switch-board-btn"
      onClick={() => handleSwitchBoard("/new/board")}
    >
      switch
    </button>
  );
}

/** Renders loading state and whether board data is present. */
function LoadingProbe() {
  const loading = useWindowLoading();
  const board = useBoardData();
  return (
    <>
      <span data-testid="loading-state">{loading ? "loading" : "ready"}</span>
      <span data-testid="board-state">{board ? "has-board" : "no-board"}</span>
    </>
  );
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("WindowContainer", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    listeners.clear();
    windowListeners.clear();
  });

  it("renders children", async () => {
    await act(async () => {
      render(
        <RustEngineContainer>
          <WindowContainer>
            <span data-testid="child">hello</span>
          </WindowContainer>
        </RustEngineContainer>,
      );
    });

    expect(screen.getByTestId("child").textContent).toBe("hello");
  });

  it("provides activeBoardPath context (initially none)", async () => {
    await act(async () => {
      render(
        <RustEngineContainer>
          <WindowContainer>
            <ActiveBoardPathProbe />
          </WindowContainer>
        </RustEngineContainer>,
      );
    });

    expect(screen.getByTestId("active-board-path").textContent).toBe("none");
  });

  it("provides openBoards context (initially empty)", async () => {
    await act(async () => {
      render(
        <RustEngineContainer>
          <WindowContainer>
            <OpenBoardsProbe />
          </WindowContainer>
        </RustEngineContainer>,
      );
    });

    expect(screen.getByTestId("open-boards-count").textContent).toBe("0");
  });

  it("registers board-opened window listener on mount", async () => {
    await act(async () => {
      render(
        <RustEngineContainer>
          <WindowContainer>
            <div>child</div>
          </WindowContainer>
        </RustEngineContainer>,
      );
    });

    const windowListenCalls = mockWindowListen.mock.calls.map(
      (c: unknown[]) => c[0],
    ) as string[];
    expect(windowListenCalls).toContain("board-opened");
  });

  it("registers board-changed global listener on mount", async () => {
    await act(async () => {
      render(
        <RustEngineContainer>
          <WindowContainer>
            <div>child</div>
          </WindowContainer>
        </RustEngineContainer>,
      );
    });

    const listenCalls = mockListen.mock.calls.map(
      (c: unknown[]) => c[0],
    ) as string[];
    expect(listenCalls).toContain("board-changed");
  });

  it("board-opened event updates activeBoardPath", async () => {
    // Mock refreshEntities to return board data
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "get_ui_state")
        return Promise.resolve({
          palette_open: false,
          palette_mode: "command",
          keymap_mode: "cua",
          scope_chain: [],
          open_boards: [],
          windows: {},
          recent_boards: [],
        });
      if (cmd === "list_schemas") return Promise.resolve([]);
      if (cmd === "list_open_boards")
        return Promise.resolve([
          { path: "/new/board", name: "New Board", is_active: true },
        ]);
      if (cmd === "get_board_data")
        return Promise.resolve({
          board: { entity_type: "board", id: "b1", name: "New Board" },
          columns: [],

          tags: [],
          summary: {
            total_tasks: 0,
            total_actors: 0,
            ready_tasks: 0,
            blocked_tasks: 0,
            done_tasks: 0,
            percent_complete: 0,
          },
        });
      if (cmd === "list_entities") return Promise.resolve({ entities: [] });
      if (cmd === "dispatch_command") return Promise.resolve(null);
      return Promise.resolve(null);
    });

    await act(async () => {
      render(
        <RustEngineContainer>
          <WindowContainer>
            <ActiveBoardPathProbe />
          </WindowContainer>
        </RustEngineContainer>,
      );
    });

    // Emit board-opened window event
    await act(async () => {
      emitWindowEvent("board-opened", { path: "/new/board" });
    });

    await waitFor(() => {
      expect(screen.getByTestId("active-board-path").textContent).toBe(
        "/new/board",
      );
    });
  });

  it("board-changed event refreshes open boards list", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "get_ui_state")
        return Promise.resolve({
          palette_open: false,
          palette_mode: "command",
          keymap_mode: "cua",
          scope_chain: [],
          open_boards: [],
          windows: {},
          recent_boards: [],
        });
      if (cmd === "list_schemas") return Promise.resolve([]);
      if (cmd === "list_open_boards")
        return Promise.resolve([
          { path: "/board-a", name: "Board A", is_active: true },
        ]);
      if (cmd === "get_board_data")
        return Promise.resolve({
          board: { entity_type: "board", id: "b1", name: "Board A" },
          columns: [],

          tags: [],
          summary: {
            total_tasks: 0,
            total_actors: 0,
            ready_tasks: 0,
            blocked_tasks: 0,
            done_tasks: 0,
            percent_complete: 0,
          },
        });
      if (cmd === "list_entities") return Promise.resolve({ entities: [] });
      if (cmd === "dispatch_command") return Promise.resolve(null);
      return Promise.resolve(null);
    });

    await act(async () => {
      render(
        <RustEngineContainer>
          <WindowContainer>
            <OpenBoardsProbe />
          </WindowContainer>
        </RustEngineContainer>,
      );
    });

    // Emit board-changed global event
    await act(async () => {
      emitTauriEvent("board-changed", {});
    });

    await waitFor(() => {
      expect(screen.getByTestId("open-boards-count").textContent).toBe("1");
    });
  });

  it("handleSwitchBoard updates activeBoardPath and dispatches file.switchBoard", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "get_ui_state")
        return Promise.resolve({
          palette_open: false,
          palette_mode: "command",
          keymap_mode: "cua",
          scope_chain: [],
          open_boards: [],
          windows: {},
          recent_boards: [],
        });
      if (cmd === "list_schemas") return Promise.resolve([]);
      if (cmd === "list_open_boards")
        return Promise.resolve([
          { path: "/new/board", name: "New Board", is_active: true },
        ]);
      if (cmd === "get_board_data")
        return Promise.resolve({
          board: { entity_type: "board", id: "b1", name: "Board" },
          columns: [],

          tags: [],
          summary: {
            total_tasks: 0,
            total_actors: 0,
            ready_tasks: 0,
            blocked_tasks: 0,
            done_tasks: 0,
            percent_complete: 0,
          },
        });
      if (cmd === "list_entities") return Promise.resolve({ entities: [] });
      if (cmd === "dispatch_command") return Promise.resolve(null);
      return Promise.resolve(null);
    });

    await act(async () => {
      render(
        <RustEngineContainer>
          <WindowContainer>
            <ActiveBoardPathProbe />
            <SwitchBoardProbe />
          </WindowContainer>
        </RustEngineContainer>,
      );
    });

    // Click switch board
    await act(async () => {
      screen.getByTestId("switch-board-btn").click();
    });

    await waitFor(() => {
      expect(screen.getByTestId("active-board-path").textContent).toBe(
        "/new/board",
      );
    });

    // Verify dispatch_command was called with file.switchBoard
    const dispatchCalls = mockInvoke.mock.calls.filter(
      (c: unknown[]) => c[0] === "dispatch_command",
    );
    expect(dispatchCalls.length).toBeGreaterThan(0);
    const switchCall = dispatchCalls.find((c: unknown[]) => {
      const args = c[1] as Record<string, unknown>;
      return args.cmd === "file.switchBoard";
    });
    expect(switchCall).toBeTruthy();
  });

  it("handleSwitchBoard clears board data so loading spinner shows", async () => {
    // Start with a board loaded so we can verify it gets cleared on switch.
    const boardData = {
      board: { entity_type: "board", id: "b1", name: "Board" },
      columns: [],
      tags: [],
      summary: {
        total_tasks: 0,
        total_actors: 0,
        ready_tasks: 0,
        blocked_tasks: 0,
        done_tasks: 0,
        percent_complete: 0,
      },
    };

    // Use a deferred promise for dispatch_command so we can observe the
    // intermediate state between handleSwitchBoard clearing the board and
    // the refresh completing.
    let resolveDispatch: (v: unknown) => void = () => {};
    const dispatchPromise = new Promise((resolve) => {
      resolveDispatch = resolve;
    });

    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "get_ui_state")
        return Promise.resolve({
          palette_open: false,
          palette_mode: "command",
          keymap_mode: "cua",
          scope_chain: [],
          open_boards: [],
          windows: {},
          recent_boards: [],
        });
      if (cmd === "list_schemas") return Promise.resolve([]);
      if (cmd === "list_open_boards")
        return Promise.resolve([
          { path: "/board/a", name: "Board A", is_active: true },
        ]);
      if (cmd === "get_board_data") return Promise.resolve(boardData);
      if (cmd === "list_entities") return Promise.resolve({ entities: [] });
      if (cmd === "dispatch_command") return dispatchPromise;
      return Promise.resolve(null);
    });

    await act(async () => {
      render(
        <RustEngineContainer>
          <WindowContainer>
            <LoadingProbe />
            <SwitchBoardProbe />
          </WindowContainer>
        </RustEngineContainer>,
      );
    });

    // Board should be loaded after initial render.
    await waitFor(() => {
      expect(screen.getByTestId("board-state").textContent).toBe("has-board");
      expect(screen.getByTestId("loading-state").textContent).toBe("ready");
    });

    // Trigger board switch — dispatch_command is blocked so we can inspect
    // intermediate state.
    act(() => {
      screen.getByTestId("switch-board-btn").click();
    });

    // Board should be cleared immediately (before dispatch resolves).
    await waitFor(() => {
      expect(screen.getByTestId("board-state").textContent).toBe("no-board");
    });

    // Resolve the dispatch and let refresh complete.
    await act(async () => {
      resolveDispatch(null);
    });

    // After refresh, board should be loaded again.
    await waitFor(() => {
      expect(screen.getByTestId("board-state").textContent).toBe("has-board");
      expect(screen.getByTestId("loading-state").textContent).toBe("ready");
    });
  });
});
