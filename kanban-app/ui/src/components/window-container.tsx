/**
 * WindowContainer owns the top-level window scope and all window-lifecycle
 * concerns. In the production tree it sits inside `CommandBusyProvider` and
 * `RustEngineContainer` (see `App.tsx`) so dispatched commands and refetches
 * share the same in-flight counter.
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
 * - CommandBusyProvider -- lifted to `App.tsx` so both `useDispatchCommand`
 *   (called inside this container) and `refreshEntities` (called inside
 *   RustEngineContainer, which wraps this container) write into the same counter.
 * - Entity state (entitiesByType) -- owned by RustEngineContainer
 * - Entity event listeners -- owned by RustEngineContainer
 * - Inspector panel state (panelStack, InspectorSyncBridge) -- stays in AppContent
 */

import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
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
  type DispatchOptions,
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
 * Ad-hoc dispatch callable returned by `useDispatchCommand()` (no preset).
 *
 * Written explicitly rather than `ReturnType<typeof useDispatchCommand>`
 * because TS picks the last overload for `ReturnType` on an overloaded
 * function, which would resolve to the pre-bound (one-arg) shape.
 */
type DispatchFn = (cmd: string, opts?: DispatchOptions) => Promise<unknown>;

/** Shared dependencies passed to the window-lifecycle hooks below. */
interface WindowBoardDeps {
  activeBoardPathRef: React.MutableRefObject<string | undefined>;
  dispatchRef: React.MutableRefObject<DispatchFn>;
  refreshEntities: ReturnType<typeof useRefreshEntities>;
  setBoard: React.Dispatch<React.SetStateAction<BoardData | null>>;
  setLoading: React.Dispatch<React.SetStateAction<boolean>>;
  setOpenBoards: React.Dispatch<React.SetStateAction<OpenBoard[]>>;
  setActiveBoardPath: React.Dispatch<React.SetStateAction<string | undefined>>;
  setEntitiesByType: ReturnType<typeof useSetEntitiesByType>;
  setEngineActiveBoardPath: ReturnType<typeof useEngineSetActiveBoardPath>;
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

  // Memoize the deps bundle so child hooks can use `[deps, ...]` dep arrays
  // without retriggering every render. All members are stable: refs are
  // `useRef`-backed, setters are React-guaranteed stable, and `refreshEntities`
  // is `useCallback`-memoized in RustEngineContainer.
  const deps: WindowBoardDeps = useMemo(
    () => ({
      activeBoardPathRef,
      dispatchRef,
      refreshEntities,
      setBoard,
      setLoading,
      setOpenBoards,
      setActiveBoardPath,
      setEntitiesByType,
      setEngineActiveBoardPath,
    }),
    [refreshEntities, setEntitiesByType, setEngineActiveBoardPath],
  );

  const refresh = useWindowRefresh(deps);
  useRestoreWindowStateOnMount(deps, refresh);
  useBoardEventListeners(deps);
  const handleSwitchBoard = useSwitchBoardHandler(deps, refresh, dispatch);

  return (
    <WindowProviderTree
      activeBoardPath={activeBoardPath}
      openBoards={openBoards}
      board={board}
      loading={loading}
      handleSwitchBoard={handleSwitchBoard}
    >
      {children}
    </WindowProviderTree>
  );
}

/** Reset all window-local board state when no boards remain open. */
function clearWindowBoardState(deps: WindowBoardDeps): void {
  deps.setBoard(null);
  deps.setEntitiesByType({});
  deps.setActiveBoardPath(undefined);
  deps.setEngineActiveBoardPath(undefined);
  deps.setLoading(false);
}

/** Fire-and-forget `file.switchBoard` to persist the window's active board. */
function persistActiveBoard(deps: WindowBoardDeps, path: string): void {
  deps.dispatchRef
    .current("file.switchBoard", {
      args: { windowLabel: WINDOW_LABEL, path },
    })
    .catch(() => {});
}

/**
 * If the window has no active path or the path is no longer open, pick a
 * fallback from `openBoards` (active-flagged first, else first available),
 * persist it, and — if it differs from the current path — reload its data.
 * Returns `true` when the caller should exit (fallback refetch completed).
 */
async function applyFallbackBoardIfNeeded(
  deps: WindowBoardDeps,
  currentPath: string | undefined,
  openBoards: OpenBoard[],
): Promise<boolean> {
  const pathStillOpen =
    currentPath && openBoards.some((b) => b.path === currentPath);
  if ((currentPath && pathStillOpen) || openBoards.length === 0) {
    return false;
  }
  const active = openBoards.find((b) => b.is_active) ?? openBoards[0];
  deps.setActiveBoardPath(active.path);
  deps.activeBoardPathRef.current = active.path;
  deps.setEngineActiveBoardPath(active.path);
  persistActiveBoard(deps, active.path);
  if (active.path === currentPath) return false;
  const corrected = await deps.refreshEntities(active.path);
  deps.setBoard(corrected.boardData);
  deps.setLoading(false);
  return true;
}

/** Body of the window `refresh` callback — extracted for line-count limits. */
async function runWindowRefresh(deps: WindowBoardDeps): Promise<void> {
  deps.setLoading(true);
  const currentPath = deps.activeBoardPathRef.current;

  const result = currentPath
    ? await deps.refreshEntities(currentPath)
    : await deps.refreshEntities("");
  // Open boards always update — even if board data failed.
  deps.setOpenBoards(result.openBoards);

  if (await applyFallbackBoardIfNeeded(deps, currentPath, result.openBoards)) {
    return;
  }

  if (result.openBoards.length === 0) {
    clearWindowBoardState(deps);
    return;
  }
  deps.setBoard(result.boardData);
  deps.setLoading(false);
}

/**
 * Pulls the full window state (open boards, active path, board data) from the
 * backend and reconciles with the current window. If this window's active path
 * is missing or no longer open, falls back to the backend's active board and
 * persists the selection via `file.switchBoard` so it survives a hot reload.
 */
function useWindowRefresh(deps: WindowBoardDeps): () => Promise<void> {
  return useCallback(() => runWindowRefresh(deps), [deps]);
}

/**
 * Apply any persisted window→board mapping from `get_ui_state`. Only writes
 * state when no URL-supplied board is present (main window restore path).
 * Silently skips on error — callers always fall through to `refresh()`.
 */
async function applyRestoredWindowState(
  deps: WindowBoardDeps,
  isCancelled: () => boolean,
): Promise<void> {
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
    if (isCancelled()) return;
    const winState = uiState.windows?.[WINDOW_LABEL];
    if (INITIAL_BOARD_PATH || !winState?.board_path) return;
    await deps.dispatchRef.current("file.switchBoard", {
      args: { windowLabel: WINDOW_LABEL, path: winState.board_path },
    });
    if (isCancelled()) return;
    deps.setActiveBoardPath(winState.board_path);
    deps.activeBoardPathRef.current = winState.board_path;
    deps.setEngineActiveBoardPath(winState.board_path);
  } catch {
    // No saved state — caller falls through to refresh().
  }
}

/**
 * Restores window state from the backend on mount:
 * - Main window: reads `board_path` from `get_ui_state`.
 * - Secondary windows: board comes from the URL param; this still runs `refresh`.
 *
 * Cancellable via an effect-scoped flag to avoid late state writes after unmount.
 */
function useRestoreWindowStateOnMount(
  deps: WindowBoardDeps,
  refresh: () => Promise<void>,
): void {
  useEffect(() => {
    let cancelled = false;
    (async () => {
      await applyRestoredWindowState(deps, () => cancelled);
      if (cancelled) return;
      await refresh();
    })();
    return () => {
      cancelled = true;
    };
  }, [deps, refresh]);
}

/** Switch the window to `path`, reload its data, and optionally persist. */
async function adoptBoard(
  deps: WindowBoardDeps,
  path: string,
  persist: boolean,
): Promise<void> {
  deps.setActiveBoardPath(path);
  deps.activeBoardPathRef.current = path;
  deps.setEngineActiveBoardPath(path);
  if (persist) persistActiveBoard(deps, path);
  deps.setLoading(true);
  const result = await deps.refreshEntities(path);
  deps.setOpenBoards(result.openBoards);
  deps.setBoard(result.boardData);
  deps.setLoading(false);
}

/** Keep the window's current board, just reload its data. */
async function refreshCurrentBoard(
  deps: WindowBoardDeps,
  path: string,
): Promise<void> {
  deps.setLoading(true);
  const result = await deps.refreshEntities(path);
  deps.setBoard(result.boardData);
  deps.setLoading(false);
}

/** Query UIState for this window's assigned board path, if any. */
async function fetchAssignedBoardPath(): Promise<string | undefined> {
  try {
    const uiState = await invoke<{
      windows: Record<string, { board_path?: string }>;
    }>("get_ui_state");
    return uiState.windows?.[WINDOW_LABEL]?.board_path;
  } catch {
    return undefined;
  }
}

/** Body of the `board-changed` listener — extracted for line-count limits. */
async function runBoardChanged(deps: WindowBoardDeps): Promise<void> {
  let boards: OpenBoard[] = [];
  try {
    boards = await invoke<OpenBoard[]>("list_open_boards");
  } catch {
    /* ignore */
  }
  deps.setOpenBoards(boards);

  if (boards.length === 0) {
    clearWindowBoardState(deps);
    return;
  }

  const assignedPath = await fetchAssignedBoardPath();
  const currentPath = deps.activeBoardPathRef.current;

  if (
    assignedPath &&
    assignedPath !== currentPath &&
    boards.some((b) => b.path === assignedPath)
  ) {
    await adoptBoard(deps, assignedPath, false);
    return;
  }

  if (currentPath && boards.some((b) => b.path === currentPath)) {
    await refreshCurrentBoard(deps, currentPath);
    return;
  }

  const fallback = boards.find((b) => b.is_active) ?? boards[0];
  await adoptBoard(deps, fallback.path, true);
}

/**
 * Registers board-level Tauri event listeners (`board-opened`, `board-changed`)
 * for the lifetime of the window. Entity-level events are owned by
 * `RustEngineContainer`.
 */
function useBoardEventListeners(deps: WindowBoardDeps): void {
  useEffect(() => {
    const unlisteners = [
      getCurrentWindow().listen<{ path: string }>(
        "board-opened",
        async (event) => {
          await adoptBoard(deps, event.payload.path, true);
        },
      ),
      listen("board-changed", () => runBoardChanged(deps)),
    ];
    return () => {
      for (const p of unlisteners) {
        p.then((fn: () => void) => fn());
      }
    };
  }, [deps]);
}

/**
 * Returns the callback bound to `useHandleSwitchBoard()` for descendants.
 * Clears stale board data eagerly so the loading spinner renders immediately
 * instead of the previous board briefly showing during the switch.
 */
function useSwitchBoardHandler(
  deps: WindowBoardDeps,
  refresh: () => Promise<void>,
  dispatch: DispatchFn,
): (path: string) => Promise<void> {
  return useCallback(
    async (path: string) => {
      deps.setActiveBoardPath(path);
      deps.activeBoardPathRef.current = path;
      deps.setEngineActiveBoardPath(path);
      deps.setBoard(null);
      deps.setEntitiesByType({});
      try {
        await dispatch("file.switchBoard", {
          args: { windowLabel: WINDOW_LABEL, path },
        });
      } catch {
        /* ignore */
      }
      refresh();
    },
    [deps, refresh, dispatch],
  );
}

interface WindowProviderTreeProps {
  activeBoardPath: string | undefined;
  openBoards: OpenBoard[];
  board: BoardData | null;
  loading: boolean;
  handleSwitchBoard: (path: string) => void;
  children: ReactNode;
}

/**
 * Provider tree that exposes window-scope state (open boards, active path,
 * board data, loading flag, switch handler) to descendants, and mounts the
 * global `AppShell`, `Toaster`, tooltip provider, and init-progress listener.
 */
function WindowProviderTree({
  activeBoardPath,
  openBoards,
  board,
  loading,
  handleSwitchBoard,
  children,
}: WindowProviderTreeProps) {
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
