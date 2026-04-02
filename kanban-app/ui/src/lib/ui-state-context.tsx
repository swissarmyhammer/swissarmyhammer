import {
  createContext,
  useContext,
  useEffect,
  useState,
  type ReactNode,
} from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

/** Shape of per-window state inside UIState. */
export interface WindowStateSnapshot {
  /** The board path assigned to this window. Empty string means no board. */
  board_path: string;
  inspector_stack: string[];
  /** The active view ID for this window. */
  active_view_id: string;
  /** The active perspective ID for this window. Empty string means no perspective selected. */
  active_perspective_id: string;
  /** Whether the command palette is open in this window. */
  palette_open: boolean;
  /** Palette mode for this window: "command" or "search". */
  palette_mode: "command" | "search";
  x?: number;
  y?: number;
  width?: number;
  height?: number;
  maximized?: boolean;
}

/** Shape of the UIState from the Rust backend. */
export interface UIStateSnapshot {
  keymap_mode: string;
  scope_chain: string[];
  open_boards: string[];
  /** Whether the system clipboard has a swissarmyhammer entity payload. */
  has_clipboard: boolean;
  /** The entity type on the clipboard (e.g. "task", "tag"), or null. */
  clipboard_entity_type: string | null;
  /** Per-window state map: window label → WindowStateSnapshot. */
  windows: Record<string, WindowStateSnapshot>;
  recent_boards: Array<{ path: string; name: string; last_opened: string }>;
}

const DEFAULT_STATE: UIStateSnapshot = {
  keymap_mode: "cua",
  scope_chain: [],
  open_boards: [],
  has_clipboard: false,
  clipboard_entity_type: null,
  windows: {},
  recent_boards: [],
};

interface UIStateContextValue {
  state: UIStateSnapshot;
  loading: boolean;
}

const UIStateContext = createContext<UIStateContextValue>({
  state: DEFAULT_STATE,
  loading: true,
});

/** Provider that fetches UIState on mount and subscribes to changes. */
export function UIStateProvider({ children }: { children: ReactNode }) {
  const [state, setState] = useState<UIStateSnapshot>(DEFAULT_STATE);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    // Initial fetch
    invoke<UIStateSnapshot>("get_ui_state")
      .then((s) => {
        setState(s);
        setLoading(false);
      })
      .catch((err) => {
        console.error("Failed to fetch UIState:", err);
        setLoading(false);
      });

    // Subscribe to changes
    const unlisten = listen<UIStateSnapshot>("ui-state-changed", (event) => {
      setState(event.payload);
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  return (
    <UIStateContext.Provider value={{ state, loading }}>
      {children}
    </UIStateContext.Provider>
  );
}

/** Read the current UIState. Must be inside a UIStateProvider. */
export function useUIState(): UIStateSnapshot {
  return useContext(UIStateContext).state;
}

/** Read UIState with loading flag. */
export function useUIStateLoading(): UIStateContextValue {
  return useContext(UIStateContext);
}
