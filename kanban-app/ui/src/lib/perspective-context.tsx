import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  backendDispatch,
  CommandScopeContext,
  scopeChainFromScope,
  useActiveBoardPath,
} from "@/lib/command-scope";
import type { PerspectiveDef } from "@/types/kanban";
import { useUIState } from "./ui-state-context";
import { useViews } from "./views-context";

/** This window's label — stable for the lifetime of the window. */
const WINDOW_LABEL = getCurrentWindow().label;

interface PerspectivesContextValue {
  perspectives: PerspectiveDef[];
  activePerspective: PerspectiveDef | null;
  setActivePerspectiveId: (id: string) => void;
  refresh: () => Promise<void>;
}

const PerspectivesContext = createContext<PerspectivesContextValue | null>(
  null,
);

/** Provider that manages the perspectives list for this window.
 *
 * Follows the ViewsProvider pattern: self-contained state, own data fetching
 * via `perspective.list` command, event-driven refresh, and active perspective
 * selection from UIState.
 */
export function PerspectiveProvider({ children }: { children: ReactNode }) {
  const [perspectives, setPerspectives] = useState<PerspectiveDef[]>([]);
  const [loaded, setLoaded] = useState(false);

  // active_perspective_id comes from UIState per-window data — it is the single
  // source of truth. UIStateProvider keeps it in sync via the "ui-state-changed"
  // event.
  const uiState = useUIState();
  const active_perspective_id =
    uiState.windows?.[WINDOW_LABEL]?.active_perspective_id ?? "";

  const { activeView } = useViews();
  const viewKind = activeView?.kind ?? "board";

  // Read scope chain from the CommandScope tree so dispatch calls include the
  // window moniker (e.g. "window:main"). Without this, secondary windows
  // silently target the "main" window slot in UIState.
  const scope = useContext(CommandScopeContext);
  const scopeChain = useMemo(() => scopeChainFromScope(scope), [scope]);
  const boardPath = useActiveBoardPath();

  /** Dispatch a perspective switch through the command system so UIState owns
   *  the change. */
  const setActivePerspectiveId = useCallback(
    (id: string) => {
      backendDispatch({
        cmd: "ui.perspective.set",
        args: { perspective_id: id },
        scopeChain,
        ...(boardPath ? { boardPath } : {}),
      }).catch(console.error);
    },
    [scopeChain, boardPath],
  );

  const refresh = useCallback(async () => {
    try {
      const wrapped = (await backendDispatch({
        cmd: "perspective.list",
        scopeChain,
        ...(boardPath ? { boardPath } : {}),
      })) as { result?: { perspectives?: PerspectiveDef[] } };
      setPerspectives(wrapped?.result?.perspectives ?? []);
      setLoaded(true);
    } catch (error) {
      console.error("Failed to load perspectives:", error);
    }
  }, [scopeChain, boardPath]);

  // On mount: load perspectives. UIState already provides active_perspective_id.
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

  // Auto-create a "Default" perspective when none exist for the current view kind.
  // Uses a ref to avoid repeated creation attempts per view kind.
  const autoCreatedForKindRef = useRef<string | null>(null);
  useEffect(() => {
    if (!loaded) return;
    if (autoCreatedForKindRef.current === viewKind) return;
    const hasForKind = perspectives.some((p) => p.view === viewKind);
    if (!hasForKind) {
      autoCreatedForKindRef.current = viewKind;
      backendDispatch({
        cmd: "perspective.save",
        args: { name: "Default", view: viewKind },
        scopeChain,
        ...(boardPath ? { boardPath } : {}),
      }).catch(console.error);
    }
  }, [loaded, perspectives, viewKind]);

  // Re-fetch perspectives when perspective entities change (file watcher or
  // commands). The "perspective" check is an entity-type filter — this context
  // only cares about perspective entities, so we ignore events for tasks, tags,
  // etc.
  useEffect(() => {
    const unlisteners = [
      listen<{ entity_type: string }>("entity-field-changed", (event) => {
        if (event.payload.entity_type === "perspective") refresh();
      }),
      listen<{ entity_type: string }>("entity-created", (event) => {
        if (event.payload.entity_type === "perspective") refresh();
      }),
      listen<{ entity_type: string }>("entity-removed", (event) => {
        if (event.payload.entity_type === "perspective") refresh();
      }),
      listen("board-changed", () => refresh()),
    ];
    return () => {
      for (const p of unlisteners) p.then((fn) => fn());
    };
  }, [refresh]);

  // Derive the active perspective object from UIState's active_perspective_id.
  // Falls back to the first perspective if the ID is empty or not found.
  const activePerspective = useMemo(
    () =>
      perspectives.find((p) => p.id === active_perspective_id) ??
      perspectives[0] ??
      null,
    [perspectives, active_perspective_id],
  );

  const value = useMemo<PerspectivesContextValue>(
    () => ({
      perspectives,
      activePerspective,
      setActivePerspectiveId,
      refresh,
    }),
    [perspectives, activePerspective, setActivePerspectiveId, refresh],
  );

  return (
    <PerspectivesContext.Provider value={value}>
      {children}
    </PerspectivesContext.Provider>
  );
}

/** Read the current perspectives context. Must be inside a PerspectiveProvider. */
export function usePerspectives(): PerspectivesContextValue {
  const ctx = useContext(PerspectivesContext);
  if (!ctx)
    throw new Error("usePerspectives must be used within PerspectiveProvider");
  return ctx;
}
