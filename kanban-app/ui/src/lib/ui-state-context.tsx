import { createContext, useContext, useEffect, useState, type ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

/** Shape of the UIState from the Rust backend. */
export interface UIStateSnapshot {
  inspector_stack: string[];
  active_view_id: string;
  palette_open: boolean;
  keymap_mode: string;
  scope_chain: string[];
}

const DEFAULT_STATE: UIStateSnapshot = {
  inspector_stack: [],
  active_view_id: "",
  palette_open: false,
  keymap_mode: "cua",
  scope_chain: [],
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
