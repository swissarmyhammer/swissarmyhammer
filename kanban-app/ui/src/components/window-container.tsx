/**
 * WindowContainer owns the top-level window scope and all window-lifecycle
 * concerns. It is the outermost container in the component tree.
 *
 * Owns:
 * - CommandScopeProvider moniker="window:{WINDOW_LABEL}"
 * - TooltipProvider, Toaster, InitProgressListener
 * - ActiveBoardPathProvider
 * - AppShell (global keybindings -- must work even with no board loaded)
 * - Window-level state: openBoards, activeBoardPath, board, loading
 * - Board-level Tauri event listeners: board-opened, board-changed
 * - Board switching logic (handleSwitchBoard)
 * - Calls refreshEntities(boardPath) from RustEngineContainer context on board switch
 *
 * Does NOT own:
 * - Entity state (entitiesByType) -- owned by RustEngineContainer
 * - Entity event listeners -- owned by RustEngineContainer
 * - Inspector panel state (panelStack, InspectorSyncBridge) -- stays in AppContent
 */

import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { TooltipProvider } from "@/components/ui/tooltip";
import { Toaster } from "sonner";
import { InitProgressListener } from "@/components/init-progress-listener";
import { AppShell } from "@/components/app-shell";
import {
  CommandScopeProvider,
  ActiveBoardPathProvider,
  useDispatchCommand,
} from "@/lib/command-scope";
import {
  useRefreshEntities,
  useSetEntitiesByType,
  useEngineSetActiveBoardPath,
} from "@/components/rust-engine-container";
import type { BoardData, OpenBoard } from "@/types/kanban";

// ---------------------------------------------------------------------------
// Module-level constants
// ---------------------------------------------------------------------------

/** Parse URL params once at module level. */
const URL_PARAMS = new URLSearchParams(window.location.search);

/** Initial board path from URL (set when opening a new window for a specific board). */
const INITIAL_BOARD_PATH = URL_PARAMS.get("board") ?? undefined;

/** Window label for per-window state persistence. */
const WINDOW_LABEL = getCurrentWindow().label;

// ---------------------------------------------------------------------------
// Contexts — expose window-level state to descendants
// ---------------------------------------------------------------------------

const OpenBoardsContext = createContext<OpenBoard[]>([]);

/**
 * Returns the list of currently open boards.
 * Re-renders when the list changes.
 */
export function useOpenBoards(): OpenBoard[] {
  return useContext(OpenBoardsContext);
}

const ActiveBoardPathContext = createContext<string | undefined>(undefined);

/**
 * Returns the active board path for this window.
 * Re-renders when the path changes.
 */
export function useActiveBoardPath(): string | undefined {
  return useContext(ActiveBoardPathContext);
}

const HandleSwitchBoardContext = createContext<(path: string) => void>(
  () => {},
);

/**
 * Returns a function to switch the active board for this window.
 * Persists the change via the backend file.switchBoard command.
 */
export function useHandleSwitchBoard(): (path: string) => void {
  return useContext(HandleSwitchBoardContext);
}

const BoardDataContext = createContext<BoardData | null>(null);

/**
 * Returns the current board data (metadata, columns, tags, etc.).
 * Null when no board is loaded.
 */
export function useBoardData(): BoardData | null {
  return useContext(BoardDataContext);
}

const LoadingContext = createContext<boolean>(true);

/**
 * Returns whether the window is currently loading board data.
 */
export function useWindowLoading(): boolean {
  return useContext(LoadingContext);
}

// ---------------------------------------------------------------------------
// WindowContainer
// ---------------------------------------------------------------------------

interface WindowContainerProps {
  children: ReactNode;
}

/**
 * Top-level window container that owns the window command scope,
 * board lifecycle state, and global UI shell.
 *
 * Renders the CommandScopeProvider for the window moniker first, then
 * delegates to WindowContainerInner which can use useDispatchCommand
 * within that scope.
 *
 * Must render inside RustEngineContainer so it can access refreshEntities,
 * setEntitiesByType, and setEngineActiveBoardPath.
 */
export function WindowContainer({ children }: WindowContainerProps) {
  return (
    <CommandScopeProvider commands={[]} moniker={`window:${WINDOW_LABEL}`}>
      <WindowContainerInner>{children}</WindowContainerInner>
    </CommandScopeProvider>
  );
}

/**
 * Inner implementation of WindowContainer. Renders inside the window-scoped
 * CommandScopeProvider so useDispatchCommand picks up the correct scope chain
 * including the window moniker.
 */
function WindowContainerInner({ children }: WindowContainerProps) {
  const refreshEntities = useRefreshEntities();
  const setEntitiesByType = useSetEntitiesByType();
  const setEngineActiveBoardPath = useEngineSetActiveBoardPath();

  const [board, setBoard] = useState<BoardData | null>(null);
  const [loading, setLoading] = useState(true);
  const [openBoards, setOpenBoards] = useState<OpenBoard[]>([]);
  /** Per-window active board path. Secondary windows get it from URL; main restores from backend. */
  const [activeBoardPath, setActiveBoardPath] = useState<string | undefined>(
    INITIAL_BOARD_PATH,
  );
  const activeBoardPathRef = useRef(activeBoardPath);
  activeBoardPathRef.current = activeBoardPath;

  /** Ad-hoc dispatch for file.switchBoard in event handlers. */
  const dispatch = useDispatchCommand();
  const dispatchRef = useRef(dispatch);
  dispatchRef.current = dispatch;

  // Intentional empty deps: reads activeBoardPathRef to avoid stale closure.
  // Uses refreshEntities from the container to update entities internally.
  const refresh = useCallback(async () => {
    setLoading(true);
    const currentPath = activeBoardPathRef.current;

    // Use the container's refreshEntities which updates entities internally
    // and returns the full result with openBoards and boardData.
    const result = currentPath
      ? await refreshEntities(currentPath)
      : await refreshEntities("");
    // Open boards always update -- even if board data failed.
    setOpenBoards(result.openBoards);

    // Pick or fall back to a valid active board path. Handles both initial
    // mount (no path yet) and board-closed (path no longer in open list).
    const pathStillOpen =
      currentPath && result.openBoards.some((b) => b.path === currentPath);
    if ((!currentPath || !pathStillOpen) && result.openBoards.length > 0) {
      const active =
        result.openBoards.find((b) => b.is_active) ?? result.openBoards[0];
      setActiveBoardPath(active.path);
      activeBoardPathRef.current = active.path;
      setEngineActiveBoardPath(active.path);
      // Persist the fallback selection so it survives hot reload
      dispatchRef
        .current("file.switchBoard", {
          args: { windowLabel: WINDOW_LABEL, path: active.path },
        })
        .catch(() => {});
      // Re-fetch with the correct path if we fell back
      if (active.path !== currentPath) {
        const corrected = await refreshEntities(active.path);
        setBoard(corrected.boardData);
        setLoading(false);
        return;
      }
    }

    if (result.openBoards.length === 0) {
      // All boards closed -- clear stale state so the placeholder shows.
      setBoard(null);
      setEntitiesByType({});
      setActiveBoardPath(undefined);
      setEngineActiveBoardPath(undefined);
      setLoading(false);
      return;
    }
    setBoard(result.boardData);
    setLoading(false);
  }, [refreshEntities, setEntitiesByType, setEngineActiveBoardPath]);

  // Restore window state from backend on mount.
  // For main window: reads board_path + inspector_stack from config.
  // For secondary windows: board comes from URL param, this restores inspector.
  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const uiState = await invoke<{
          windows: Record<
            string,
            {
              board_path?: string;
              inspector_stack?: string[];
              active_view_id?: string;
            }
          >;
        }>("get_ui_state");
        if (cancelled) return;
        const winState = uiState.windows?.[WINDOW_LABEL];

        // Restore board path from backend config (main window only -- secondary gets it from URL)
        if (!INITIAL_BOARD_PATH && winState?.board_path) {
          await dispatchRef.current("file.switchBoard", {
            args: { windowLabel: WINDOW_LABEL, path: winState.board_path },
          });
          if (cancelled) return;
          setActiveBoardPath(winState.board_path);
          activeBoardPathRef.current = winState.board_path;
          setEngineActiveBoardPath(winState.board_path);
        }
      } catch {
        // No saved state -- will fall through to refresh below
      }
      if (cancelled) return;
      await refresh();
      if (cancelled) return;
    })();
    return () => {
      cancelled = true;
    };
  }, [refresh, setEngineActiveBoardPath]);

  // ---------------------------------------------------------------------------
  // Board-level event listeners (board-opened, board-changed).
  // Entity event listeners are handled by RustEngineContainer.
  // ---------------------------------------------------------------------------

  useEffect(() => {
    const unlisteners = [
      // board-opened: emitted only to the window that initiated the open (via emit_to).
      getCurrentWindow().listen<{ path: string }>(
        "board-opened",
        async (event: { payload: { path: string } }) => {
          const newPath = event.payload.path;
          // Persist window->board mapping so it survives hot reload / restart
          dispatchRef
            .current("file.switchBoard", {
              args: { windowLabel: WINDOW_LABEL, path: newPath },
            })
            .catch(() => {});
          setActiveBoardPath(newPath);
          activeBoardPathRef.current = newPath;
          setEngineActiveBoardPath(newPath);
          setLoading(true);
          const result = await refreshEntities(newPath);
          setOpenBoards(result.openBoards);
          setBoard(result.boardData);
          setLoading(false);
        },
      ),
      // board-changed: structural change (open/close/switch). All windows
      // refresh their open boards list. If this window's board was closed,
      // fall back to another open board.
      listen("board-changed", async () => {
        let boards: OpenBoard[] = [];
        try {
          boards = await invoke<OpenBoard[]>("list_open_boards");
        } catch {
          /* ignore */
        }
        setOpenBoards(boards);

        if (boards.length === 0) {
          setBoard(null);
          setEntitiesByType({});
          setActiveBoardPath(undefined);
          setEngineActiveBoardPath(undefined);
          setLoading(false);
          return;
        }

        // Check if UIState says this window should show a different board
        let assignedPath: string | undefined;
        try {
          const uiState = await invoke<{
            windows: Record<string, { board_path?: string }>;
          }>("get_ui_state");
          assignedPath = uiState.windows?.[WINDOW_LABEL]?.board_path;
        } catch {
          /* ignore */
        }

        const currentPath = activeBoardPathRef.current;

        // If the backend assigned a different board to this window, switch.
        if (
          assignedPath &&
          assignedPath !== currentPath &&
          boards.some((b) => b.path === assignedPath)
        ) {
          setActiveBoardPath(assignedPath);
          activeBoardPathRef.current = assignedPath;
          setEngineActiveBoardPath(assignedPath);
          setLoading(true);
          const result = await refreshEntities(assignedPath);
          setOpenBoards(result.openBoards);
          setBoard(result.boardData);
          setLoading(false);
          return;
        }

        // If this window's board is still open, keep it and refresh data
        const stillOpen =
          currentPath && boards.some((b) => b.path === currentPath);
        if (stillOpen) {
          setLoading(true);
          const result = await refreshEntities(currentPath);
          setBoard(result.boardData);
          setLoading(false);
          return;
        }

        // Board was closed -- fall back to another open board and persist
        const fallback = boards.find((b) => b.is_active) ?? boards[0];
        setActiveBoardPath(fallback.path);
        activeBoardPathRef.current = fallback.path;
        setEngineActiveBoardPath(fallback.path);
        dispatchRef
          .current("file.switchBoard", {
            args: { windowLabel: WINDOW_LABEL, path: fallback.path },
          })
          .catch(() => {});
        setLoading(true);
        const result = await refreshEntities(fallback.path);
        setBoard(result.boardData);
        setLoading(false);
      }),
    ];
    return () => {
      for (const p of unlisteners) {
        p.then((fn: () => void) => fn());
      }
    };
  }, [refresh, refreshEntities, setEntitiesByType, setEngineActiveBoardPath]);

  /** Switch this window's active board. Persists via backend file.switchBoard command. */
  const handleSwitchBoard = useCallback(
    async (path: string) => {
      setActiveBoardPath(path);
      activeBoardPathRef.current = path;
      setEngineActiveBoardPath(path);
      // Clear stale board data so the loading spinner shows immediately
      // instead of rendering the previous board's content during the switch.
      setBoard(null);
      setEntitiesByType({});
      try {
        await dispatch("file.switchBoard", {
          args: { windowLabel: WINDOW_LABEL, path },
        });
      } catch {
        /* ignore */
      }
      refresh();
    },
    [refresh, dispatch, setEngineActiveBoardPath, setEntitiesByType],
  );

  return (
    <TooltipProvider delayDuration={400}>
      <Toaster position="bottom-right" richColors />
      <InitProgressListener />
      <ActiveBoardPathProvider value={activeBoardPath}>
        <OpenBoardsContext.Provider value={openBoards}>
          <ActiveBoardPathContext.Provider value={activeBoardPath}>
            <HandleSwitchBoardContext.Provider value={handleSwitchBoard}>
              <BoardDataContext.Provider value={board}>
                <LoadingContext.Provider value={loading}>
                  <AppShell
                    openBoards={openBoards}
                    onSwitchBoard={handleSwitchBoard}
                  >
                    {children}
                  </AppShell>
                </LoadingContext.Provider>
              </BoardDataContext.Provider>
            </HandleSwitchBoardContext.Provider>
          </ActiveBoardPathContext.Provider>
        </OpenBoardsContext.Provider>
      </ActiveBoardPathProvider>
    </TooltipProvider>
  );
}
