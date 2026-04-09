import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  type ReactNode,
} from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
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

  const dispatch = useDispatchCommand("ui.view.set");

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

  // Re-fetch views when view entities change (file watcher or commands).
  // The "view" check is an entity-type filter, not a field name — this context
  // only cares about view entities, so we ignore events for tasks, tags, etc.
  useEffect(() => {
    const unlisteners = [
      listen<{ entity_type: string }>("entity-field-changed", (event) => {
        if (event.payload.entity_type === "view") refresh();
      }),
      listen<{ entity_type: string }>("entity-created", (event) => {
        if (event.payload.entity_type === "view") refresh();
      }),
      listen<{ entity_type: string }>("entity-removed", (event) => {
        if (event.payload.entity_type === "view") refresh();
      }),
      listen("board-changed", () => refresh()),
    ];
    return () => {
      for (const p of unlisteners) p.then((fn) => fn());
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
