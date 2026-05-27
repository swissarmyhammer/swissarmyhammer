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

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    label: "main",
    listen: mockWindowListen,
  }),
}));

// Mock the views-context so PerspectiveProvider's `useViews()` resolves to a
// stable view kind ("board") without dragging the real ViewsProvider tree
// into this test. Mirrors the pattern used in `perspective-context.test.tsx`.
vi.mock("@/lib/views-context", () => ({
  useViews: () => ({
    views: [{ id: "board-1", name: "Board", kind: "board" }],
    activeView: { id: "board-1", name: "Board", kind: "board" },
    setActiveViewId: vi.fn(),
    refresh: vi.fn(() => Promise.resolve()),
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
import { PerspectiveProvider } from "@/lib/perspective-context";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { FocusLayer } from "@/components/focus-layer";
import { asSegment } from "@/types/spatial";

/** Identity-stable layer name for the test window root, matches App.tsx. */
const WINDOW_LAYER_NAME = asSegment("window");

/**
 * Wrap children in the spatial-focus + window-root layer providers that
 * `WindowContainer` (via `AppShell -> useEnclosingLayerFq`) requires.
 *
 * `WindowContainer` mounts `AppShell`, which calls `useEnclosingLayerFq()` to
 * thread the window-root layer key into the palette's portal-out
 * `<FocusLayer>`. The hook throws outside any `<FocusLayer>`, so every
 * `render(...)` in this file must sit under this wrapping — mirroring
 * `App.tsx`'s production setup.
 */
function withSpatialFocus(node: React.ReactElement): React.ReactElement {
  return (
    <SpatialFocusProvider>
      <FocusLayer name={WINDOW_LAYER_NAME}>{node}</FocusLayer>
    </SpatialFocusProvider>
  );
}

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
        withSpatialFocus(
          <RustEngineContainer>
            <WindowContainer>
              <span data-testid="child">hello</span>
            </WindowContainer>
          </RustEngineContainer>,
        ),
      );
    });

    expect(screen.getByTestId("child").textContent).toBe("hello");
  });

  it("provides activeBoardPath context (initially none)", async () => {
    await act(async () => {
      render(
        withSpatialFocus(
          <RustEngineContainer>
            <WindowContainer>
              <ActiveBoardPathProbe />
            </WindowContainer>
          </RustEngineContainer>,
        ),
      );
    });

    expect(screen.getByTestId("active-board-path").textContent).toBe("none");
  });

  it("provides openBoards context (initially empty)", async () => {
    await act(async () => {
      render(
        withSpatialFocus(
          <RustEngineContainer>
            <WindowContainer>
              <OpenBoardsProbe />
            </WindowContainer>
          </RustEngineContainer>,
        ),
      );
    });

    expect(screen.getByTestId("open-boards-count").textContent).toBe("0");
  });

  it("registers board-opened window listener on mount", async () => {
    await act(async () => {
      render(
        withSpatialFocus(
          <RustEngineContainer>
            <WindowContainer>
              <div>child</div>
            </WindowContainer>
          </RustEngineContainer>,
        ),
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
        withSpatialFocus(
          <RustEngineContainer>
            <WindowContainer>
              <div>child</div>
            </WindowContainer>
          </RustEngineContainer>,
        ),
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
        withSpatialFocus(
          <RustEngineContainer>
            <WindowContainer>
              <ActiveBoardPathProbe />
            </WindowContainer>
          </RustEngineContainer>,
        ),
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
        withSpatialFocus(
          <RustEngineContainer>
            <WindowContainer>
              <OpenBoardsProbe />
            </WindowContainer>
          </RustEngineContainer>,
        ),
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
        withSpatialFocus(
          <RustEngineContainer>
            <WindowContainer>
              <ActiveBoardPathProbe />
              <SwitchBoardProbe />
            </WindowContainer>
          </RustEngineContainer>,
        ),
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
        withSpatialFocus(
          <RustEngineContainer>
            <WindowContainer>
              <LoadingProbe />
              <SwitchBoardProbe />
            </WindowContainer>
          </RustEngineContainer>,
        ),
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

  /**
   * Regression: board switch must not leave a stale perspective filter
   * hiding the new board's cards.
   *
   * Wire-level contract:
   *   1. Frontend calls `handleSwitchBoard("/new/board")`, which dispatches
   *      `file.switchBoard` and the backend writes `board_path` for the
   *      window. The backend reset (in `UIState::set_window_board`) clears
   *      `active_perspective_id` and `filtered_task_ids` when the path
   *      differs, then emits `ui-state-changed { kind: "board_switch" }`
   *      carrying the new snapshot.
   *   2. `UIStateProvider` applies the new snapshot. With the new board's
   *      perspectives loaded and `active_perspective_id === ""`,
   *      `useAutoSelectActivePerspective` MUST dispatch `perspective.switch`
   *      for the new board's first matching perspective (repair path 1).
   *   3. The frontend never observes a (new board, stale id, stale filter)
   *      tuple — by the time the perspective list lands, the id is already
   *      empty, so no render frame surfaces a filter referencing stale
   *      task IDs.
   *
   * This test stubs the UIState transition (post-clear) and asserts the
   * dispatched `perspective.switch` carries the new board's first
   * perspective id.
   */
  it("board switch with cleared active_perspective_id triggers perspective.switch for new board's first perspective", async () => {
    // New board has two perspectives; the first matching the active view
    // kind ("board") is what `useAutoSelectActivePerspective` will pick.
    const newBoardPerspectives = [
      { id: "p-new-default", name: "New Default", view: "board" },
      { id: "p-new-other", name: "Other", view: "board" },
    ];

    mockInvoke.mockImplementation((cmd: string, args?: unknown) => {
      if (cmd === "get_ui_state") {
        // Initial snapshot: window has a board with a stale perspective
        // — this is the pre-switch state that should be replaced by the
        // ui-state-changed event below.
        return Promise.resolve({
          palette_open: false,
          palette_mode: "command",
          keymap_mode: "cua",
          scope_chain: [],
          open_boards: ["/old/board"],
          windows: {
            main: {
              board_path: "/old/board",
              inspector_stack: [],
              active_view_id: "board-1",
              active_perspective_id: "p-old",
              filtered_task_ids: ["t-old-1", "t-old-2"],
              palette_open: false,
              palette_mode: "command",
              app_mode: "normal",
            },
          },
          recent_boards: [],
        });
      }
      if (cmd === "list_schemas") return Promise.resolve([]);
      if (cmd === "list_open_boards")
        return Promise.resolve([
          { path: "/new/board", name: "New Board", is_active: true },
        ]);
      if (cmd === "get_board_data")
        return Promise.resolve({
          board: { entity_type: "board", id: "b-new", name: "New Board" },
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
      if (cmd === "dispatch_command") {
        const a = args as { cmd?: string };
        if (a?.cmd === "perspective.list") {
          // The new board's perspective list — returned by the
          // perspective.list dispatch once the auto-select hook fetches.
          return Promise.resolve({
            result: { perspectives: newBoardPerspectives, count: 2 },
            undoable: false,
          });
        }
        // All other dispatches (file.switchBoard, perspective.switch, ...)
        // resolve to null — we capture them via mockInvoke.mock.calls below.
        return Promise.resolve(null);
      }
      return Promise.resolve(null);
    });

    await act(async () => {
      render(
        withSpatialFocus(
          <RustEngineContainer>
            <WindowContainer>
              <PerspectiveProvider>
                <SwitchBoardProbe />
              </PerspectiveProvider>
            </WindowContainer>
          </RustEngineContainer>,
        ),
      );
    });

    // Trigger the board switch — fires file.switchBoard.
    await act(async () => {
      screen.getByTestId("switch-board-btn").click();
    });

    // Emit the post-switch UIState snapshot the backend would broadcast
    // AFTER set_window_board ran with a differing path: board_path moved
    // forward, active_perspective_id cleared to "", filtered_task_ids
    // gone (None on the wire → key omitted).
    await act(async () => {
      emitTauriEvent("ui-state-changed", {
        kind: "board_switch",
        state: {
          palette_open: false,
          palette_mode: "command",
          keymap_mode: "cua",
          scope_chain: [],
          open_boards: ["/new/board"],
          windows: {
            main: {
              board_path: "/new/board",
              inspector_stack: [],
              active_view_id: "board-1",
              active_perspective_id: "",
              // filtered_task_ids omitted — backend resets to None which
              // serialises as an absent key (see
              // `never_switched_window_omits_filtered_task_ids_in_to_json`).
              palette_open: false,
              palette_mode: "command",
              app_mode: "normal",
            },
          },
          recent_boards: [],
        },
      });
    });

    // Auto-select must dispatch `perspective.switch` for the new board's
    // first matching perspective. The dispatch is asynchronous (sits
    // inside a useEffect → microtask), so wait for it.
    await waitFor(() => {
      const switchCalls = mockInvoke.mock.calls.filter((c: unknown[]) => {
        if (c[0] !== "dispatch_command") return false;
        const a = c[1] as { cmd?: string };
        return a?.cmd === "perspective.switch";
      });
      expect(switchCalls.length).toBeGreaterThan(0);
      const switchCall = switchCalls[switchCalls.length - 1];
      const callArgs = switchCall[1] as {
        cmd?: string;
        args?: { perspective_id?: string };
      };
      expect(callArgs.args?.perspective_id).toBe("p-new-default");
    });

    // Confirm no perspective.switch was dispatched with the stale id —
    // the frontend must never re-arm a filter referencing the old board's
    // perspective once the cleared snapshot has landed.
    const switchCallsForStaleId = mockInvoke.mock.calls.filter(
      (c: unknown[]) => {
        if (c[0] !== "dispatch_command") return false;
        const a = c[1] as {
          cmd?: string;
          args?: { perspective_id?: string };
        };
        return (
          a?.cmd === "perspective.switch" && a?.args?.perspective_id === "p-old"
        );
      },
    );
    expect(switchCallsForStaleId).toHaveLength(0);
  });
});
