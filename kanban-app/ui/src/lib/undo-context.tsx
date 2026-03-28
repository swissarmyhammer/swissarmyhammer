/**
 * Undo/redo context — pure passthrough to the Rust backend.
 *
 * Zero undo logic lives in TypeScript. The frontend dispatches `app.undo` and
 * `app.redo` commands to the backend and queries `get_undo_state` to reflect
 * whether undo/redo are available. State is refreshed on every `entity-changed`
 * Tauri event.
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
import { listen } from "@tauri-apps/api/event";

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
async function fetchUndoState(): Promise<{ canUndo: boolean; canRedo: boolean }> {
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
 * Dispatches undo/redo to the Rust backend and refreshes `canUndo`/`canRedo`
 * from `get_undo_state` on mount and on every `entity-changed` event.
 */
export function UndoProvider({ children }: { children: ReactNode }) {
  const [canUndo, setCanUndo] = useState(false);
  const [canRedo, setCanRedo] = useState(false);

  /** Refresh undo/redo availability from the backend. */
  const refreshState = useCallback(async () => {
    const state = await fetchUndoState();
    setCanUndo(state.canUndo);
    setCanRedo(state.canRedo);
  }, []);

  // Fetch initial state and subscribe to entity changes
  useEffect(() => {
    refreshState();
    const unlisten = listen("entity-changed", () => {
      refreshState();
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [refreshState]);

  const undo = useCallback(async () => {
    await invoke("dispatch_command", { cmd: "app.undo" });
    await refreshState();
  }, [refreshState]);

  const redo = useCallback(async () => {
    await invoke("dispatch_command", { cmd: "app.redo" });
    await refreshState();
  }, [refreshState]);

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
