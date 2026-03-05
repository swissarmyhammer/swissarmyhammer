import { createContext, useCallback, useContext, useEffect, useMemo, useState, type ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { ViewDef } from "@/types/kanban";

interface ViewsContextValue {
  views: ViewDef[];
  activeView: ViewDef | null;
  setActiveViewId: (id: string) => void;
  refresh: () => Promise<void>;
}

const ViewsContext = createContext<ViewsContextValue | null>(null);

export function ViewsProvider({ children }: { children: ReactNode }) {
  const [views, setViews] = useState<ViewDef[]>([]);
  const [activeViewId, setActiveViewId] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      const result = await invoke<ViewDef[]>("list_views");
      setViews(result);
      // Auto-select first view if none is active
      if (result.length > 0) {
        setActiveViewId((prev) => {
          if (prev && result.some((v) => v.id === prev)) return prev;
          return result[0].id;
        });
      }
    } catch (error) {
      console.error("Failed to load views:", error);
    }
  }, []);

  useEffect(() => {
    refresh();
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
