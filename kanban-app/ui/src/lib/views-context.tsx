import { createContext, useCallback, useContext, useEffect, useMemo, useState, type ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { ViewDef } from "@/types/kanban";

interface ViewsContextValue {
  views: ViewDef[];
  activeView: ViewDef | null;
  setActiveViewId: (id: string) => void;
  refresh: () => Promise<void>;
}

const ViewsContext = createContext<ViewsContextValue | null>(null);

/** Window label for per-window view persistence. */
const WINDOW_LABEL = getCurrentWindow().label;

export function ViewsProvider({ children }: { children: ReactNode }) {
  const [views, setViews] = useState<ViewDef[]>([]);
  const [activeViewId, setActiveViewIdState] = useState<string | null>(null);

  // Persist active view to backend per-window on change
  const setActiveViewId = useCallback((id: string) => {
    setActiveViewIdState(id);
    invoke("set_active_view", { viewId: id, windowLabel: WINDOW_LABEL }).catch(() => {});
  }, []);

  const refresh = useCallback(async () => {
    try {
      const result = await invoke<ViewDef[]>("list_views");
      setViews(result);

      // Keep current selection if still valid, else fall back to first view
      setActiveViewIdState((prev) => {
        if (prev && result.some((v) => v.id === prev)) return prev;
        return result.length > 0 ? result[0].id : null;
      });
    } catch (error) {
      console.error("Failed to load views:", error);
    }
  }, []);

  // On mount: restore persisted view from backend, then load views
  useEffect(() => {
    let cancelled = false;
    (async () => {
      // Restore the persisted active view ID for this window
      try {
        const ctx = await invoke<{ active_view_id: string | null }>(
          "get_ui_context",
          { windowLabel: WINDOW_LABEL },
        );
        if (cancelled) return;
        if (ctx.active_view_id) {
          setActiveViewIdState(ctx.active_view_id);
        }
      } catch {
        // No saved state — refresh will pick the first view
      }
      if (cancelled) return;
      await refresh();
    })();
    return () => { cancelled = true; };
  }, [refresh]);

  // Re-fetch views when view entities change (file watcher or commands)
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

  const activeView = useMemo(
    () => views.find((v) => v.id === activeViewId) ?? null,
    [views, activeViewId],
  );

  const value = useMemo<ViewsContextValue>(
    () => ({ views, activeView, setActiveViewId, refresh }),
    [views, activeView, setActiveViewId, refresh],
  );

  return (
    <ViewsContext.Provider value={value}>
      {children}
    </ViewsContext.Provider>
  );
}

export function useViews(): ViewsContextValue {
  const ctx = useContext(ViewsContext);
  if (!ctx) throw new Error("useViews must be used within ViewsProvider");
  return ctx;
}
