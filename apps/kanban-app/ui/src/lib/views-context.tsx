import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  type ReactNode,
} from "react";
import { invoke } from "@tauri-apps/api/core";
import { useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { subscribeStoreChanged } from "@/lib/mcp-notifications";
import { useDispatchCommand } from "@/lib/command-scope";
import type { ViewDef } from "@/types/kanban";
import { useUIState } from "./ui-state-context";

/** This window's label — stable for the lifetime of the window. */
const WINDOW_LABEL = getCurrentWindow().label;

interface ViewsContextValue {
  views: ViewDef[];
  activeView: ViewDef | null;
  setActiveViewId: (id: string) => void;
  refresh: () => Promise<void>;
}

const ViewsContext = createContext<ViewsContextValue | null>(null);

export function ViewsProvider({ children }: { children: ReactNode }) {
  const [views, setViews] = useState<ViewDef[]>([]);

  // active_view_id comes from UIState per-window data — it is the single source of truth.
  // UIStateProvider keeps it in sync via the "ui-state-changed" event.
  const uiState = useUIState();
  const active_view_id = uiState.windows?.[WINDOW_LABEL]?.active_view_id ?? "";

  const dispatch = useDispatchCommand("view.set");

  /** Dispatch a view switch through the command system so UIState owns the change. */
  const setActiveViewId = useCallback(
    (id: string) => {
      dispatch({ args: { view_id: id } }).catch(console.error);
    },
    [dispatch],
  );

  const refresh = useCallback(async () => {
    try {
      const result = await invoke<ViewDef[]>("list_views");
      setViews(result);
    } catch (error) {
      console.error("Failed to load views:", error);
    }
  }, []);

  // On mount: load views. UIState already provides active_view_id.
  useEffect(() => {
    let cancelled = false;
    (async () => {
      if (cancelled) return;
      await refresh();
    })();
    return () => {
      cancelled = true;
    };
  }, [refresh]);

  // Re-fetch views when view entities change (file watcher or commands), and
  // on structural board changes (a board switch reloads its view set). The
  // input source is the MCP `notifications/store/changed` plane: the `store`
  // check is the store-name filter — this context only cares about the "view"
  // store and structural board/column changes, ignoring tasks, tags, etc.
  useEffect(() => {
    let disposed = false;
    const unsubPromise = subscribeStoreChanged((batch) => {
      if (
        batch.some(
          (n) =>
            n.store === "view" || n.store === "board" || n.store === "column",
        )
      ) {
        refresh();
      }
    });
    return () => {
      disposed = true;
      unsubPromise.then((unsub) => {
        if (disposed) unsub();
      });
    };
  }, [refresh]);

  // Derive the active view object from UIState's active_view_id.
  // Falls back to the first view if the ID is empty or not found.
  const activeView = useMemo(
    () => views.find((v) => v.id === active_view_id) ?? views[0] ?? null,
    [views, active_view_id],
  );

  const value = useMemo<ViewsContextValue>(
    () => ({ views, activeView, setActiveViewId, refresh }),
    [views, activeView, setActiveViewId, refresh],
  );

  return (
    <ViewsContext.Provider value={value}>{children}</ViewsContext.Provider>
  );
}

export function useViews(): ViewsContextValue {
  const ctx = useContext(ViewsContext);
  if (!ctx) throw new Error("useViews must be used within ViewsProvider");
  return ctx;
}
