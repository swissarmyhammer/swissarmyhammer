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
  /** Application interaction mode for this window. */
  app_mode: "normal" | "command" | "search";
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

/**
 * Discriminator values on the `ui-state-changed` wire event.
 *
 * One per `UIStateChange` variant plus the two board-side-effect result
 * shapes (`board_switch`, `board_close`). The backend tags every emit with
 * `kind` so the frontend can skip `setState` for slices it owns — notably
 * `scope_chain`, which echoes back from every `ui.setFocus` call and would
 * otherwise cascade re-renders through every `useUIState()` consumer.
 *
 * Kept in sync with `emit_ui_state_change_if_needed` in
 * `kanban-app/src/commands.rs`.
 */
export type UIStateChangeKind =
  | "scope_chain"
  | "palette_open"
  | "keymap_mode"
  | "inspector_stack"
  | "active_view"
  | "active_perspective"
  | "app_mode"
  | "board_switch"
  | "board_close";

/** Shape of the `ui-state-changed` event payload from the Rust backend. */
export interface UIStateChangedEvent {
  kind: UIStateChangeKind;
  state: UIStateSnapshot;
}

/**
 * Kinds the `UIStateProvider` suppresses — the frontend is authoritative
 * for these slices, so applying the backend echo would only waste renders.
 *
 * Currently only `scope_chain`: the frontend drives focus via
 * `FocusedScopeContext` (see `entity-focus-context.tsx`), which is the
 * source of truth for the scope chain. The backend still emits these
 * events so the rest of the pipeline (menu rebuild, command logging)
 * stays uniform — this listener just refuses to propagate them into
 * `useUIState()` consumers.
 */
const FRONTEND_AUTHORITATIVE_KINDS: ReadonlySet<UIStateChangeKind> = new Set([
  "scope_chain",
]);

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

    // Subscribe to changes. The backend wraps every emit as
    // `{ kind, state }` so we can early-return on kinds the frontend owns —
    // skipping `setState` here is what keeps `useUIState()` reference-stable
    // across arrow-key focus moves and prevents the fan-out re-render
    // cascade through every `useUIState()` consumer.
    const unlisten = listen<UIStateChangedEvent>(
      "ui-state-changed",
      (event) => {
        if (FRONTEND_AUTHORITATIVE_KINDS.has(event.payload.kind)) return;
        setState(event.payload.state);
      },
    );

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
