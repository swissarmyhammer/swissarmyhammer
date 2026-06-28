import {
  createContext,
  useContext,
  useEffect,
  useState,
  type ReactNode,
} from "react";
import { invoke } from "@tauri-apps/api/core";
import { subscribeUiStateChanged } from "./mcp-notifications";

/** Shape of per-window state inside UIState. */
export interface WindowStateSnapshot {
  /** The board path assigned to this window. Empty string means no board. */
  board_path: string;
  inspector_stack: string[];
  /** The active view ID for this window. */
  active_view_id: string;
  /** The active perspective ID for this window. Empty string means no perspective selected. */
  active_perspective_id: string;
  /**
   * IDs of tasks visible under the active perspective's filter.
   *
   * Written atomically with `active_perspective_id` by the backend
   * `perspective.switch` command — both fields land in a single
   * `ui-state-changed` event so the frontend never paints a frame
   * where the perspective and its filter results are out of sync.
   *
   * Tri-state semantics (see `view-container.tsx`):
   *   * `undefined` — no `perspective.switch` has fired for this
   *     window yet (fresh launch, legacy snapshot). Treated as
   *     "no filter active": views show every task.
   *   * `[]` — switch has fired but the active filter matched no
   *     tasks. Views show zero rows. (Conflating with `undefined`
   *     would silently disable filters that matched nothing.)
   *   * non-empty array — views intersect with this id list.
   *
   * Transient on the backend (`#[serde(skip)]`): a window restart
   * starts undefined and the next switch repopulates the field.
   */
  filtered_task_ids?: string[];
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
  /**
   * User-chosen inspector panel width for this window (CSS pixels).
   *
   * Undefined falls back to the React default (420 px). One value per
   * window applies to every panel in the inspector stack so adjacent
   * panels keep tiling without overlap.
   */
  inspector_width?: number;
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
 * Discriminator values on the `notifications/ui_state/changed` bridge event.
 *
 * One per backend `UiStateChange` variant plus the two board-side-effect
 * result shapes (`board_switch`, `board_close`). The backend tags every
 * publish with `kind` so the frontend can skip `setState` for slices it owns —
 * notably `scope_chain`, which echoes back from every `app.setFocus` call and
 * would otherwise cascade re-renders through every `useUIState()` consumer.
 *
 * Kept in sync with `UiStateChange::kind()` in
 * `crates/swissarmyhammer-ui-state/src/state.rs` (the single source of truth
 * the publish path classifies against).
 */
export type UIStateChangeKind =
  | "scope_chain"
  | "palette_open"
  | "keymap_mode"
  | "inspector_stack"
  | "active_view"
  | "active_perspective"
  | "app_mode"
  | "inspector_width"
  | "perspective_switch"
  | "board_switch"
  | "board_close";

/**
 * Shape of the `notifications/ui_state/changed` bridge notification's params.
 *
 * The declared `UiStateChanged` payload from `swissarmyhammer-ui-state`:
 * a `kind` discriminator plus the full UI-state `state` snapshot after the
 * change.
 */
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

    // Subscribe to UI-state changes over the MCP notification bridge — the
    // webview is a pure MCP client, reading `notifications/ui_state/changed`
    // (the host re-broadcasts each bridge notification as a Tauri event named
    // by the notification method) rather than a bespoke direct Tauri event.
    // The declared `UiStateChanged` payload is `{ kind, state }`, so we can
    // early-return on kinds the frontend owns — skipping `setState` here keeps
    // `useUIState()` reference-stable across arrow-key focus moves and prevents
    // the fan-out re-render cascade through every `useUIState()` consumer.
    const unlisten = subscribeUiStateChanged((change) => {
      const payload = change as unknown as UIStateChangedEvent;
      if (FRONTEND_AUTHORITATIVE_KINDS.has(payload.kind)) return;
      setState(payload.state);
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
