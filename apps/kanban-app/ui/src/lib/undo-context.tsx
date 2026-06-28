/**
 * Undo/redo context — pure passthrough to the Rust backend.
 *
 * Zero undo logic lives in TypeScript. The frontend dispatches `app.undo` and
 * `app.redo` commands to the backend and reflects whether undo/redo are
 * available. The initial state comes from `get_undo_state`; thereafter the
 * webview is a pure MCP client and tracks availability from the
 * `notifications/store/undo_changed` plane, which the backend emits whenever
 * the undo stack changes (a command, an undo, or a redo).
 */
import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useState,
  type ReactNode,
} from "react";
import { invoke } from "@tauri-apps/api/core";
import { subscribeUndoChanged } from "@/lib/mcp-notifications";
import { useDispatchCommand } from "@/lib/command-scope";

/** The shape of the undo state exposed to consumers. */
interface UndoState {
  /** Undo the most recent operation via the backend. */
  undo: () => Promise<void>;
  /** Redo the most recently undone operation via the backend. */
  redo: () => Promise<void>;
  /** Whether the backend has at least one undoable operation. */
  canUndo: boolean;
  /** Whether the backend has at least one redoable operation. */
  canRedo: boolean;
}

const UndoContext = createContext<UndoState>({
  undo: async () => {},
  redo: async () => {},
  canUndo: false,
  canRedo: false,
});

/**
 * Fetch undo/redo availability from the backend.
 *
 * Returns `{ canUndo: false, canRedo: false }` if the backend query is not
 * yet implemented or fails, so the UI degrades gracefully.
 */
async function fetchUndoState(): Promise<{
  canUndo: boolean;
  canRedo: boolean;
}> {
  try {
    const state = await invoke<{ can_undo: boolean; can_redo: boolean }>(
      "get_undo_state",
    );
    return { canUndo: state.can_undo, canRedo: state.can_redo };
  } catch {
    // Backend query not yet available — degrade gracefully
    return { canUndo: false, canRedo: false };
  }
}

/**
 * Provides undo/redo operations and state to the component tree.
 *
 * Dispatches undo/redo to the Rust backend, seeds `canUndo`/`canRedo` from
 * `get_undo_state` on mount, and thereafter tracks them from the MCP
 * `notifications/store/undo_changed` plane — the same control-state stream an
 * external agent observes.
 */
export function UndoProvider({ children }: { children: ReactNode }) {
  const [canUndo, setCanUndo] = useState(false);
  const [canRedo, setCanRedo] = useState(false);
  const dispatchUndo = useDispatchCommand("app.undo");
  const dispatchRedo = useDispatchCommand("app.redo");

  /** Seed undo/redo availability from the backend (initial state only). */
  const refreshState = useCallback(async () => {
    const state = await fetchUndoState();
    setCanUndo(state.canUndo);
    setCanRedo(state.canRedo);
  }, []);

  // Seed the initial state, then subscribe to the MCP undo-state plane. Every
  // command / undo / redo emits a fresh `undo_changed` carrying the new
  // availability, so there is no per-event refetch.
  useEffect(() => {
    let disposed = false;
    refreshState();
    const unsubPromise = subscribeUndoChanged((state) => {
      setCanUndo(state.can_undo);
      setCanRedo(state.can_redo);
    });
    return () => {
      disposed = true;
      unsubPromise.then((unsub) => {
        if (disposed) unsub();
      });
    };
  }, [refreshState]);

  const undo = useCallback(async () => {
    await dispatchUndo();
  }, [dispatchUndo]);

  const redo = useCallback(async () => {
    await dispatchRedo();
  }, [dispatchRedo]);

  const value: UndoState = { undo, redo, canUndo, canRedo };

  return <UndoContext.Provider value={value}>{children}</UndoContext.Provider>;
}

/**
 * Returns the undo/redo operations and state flags.
 *
 * Must be used within an UndoProvider.
 */
export function useUndoState(): UndoState {
  return useContext(UndoContext);
}
