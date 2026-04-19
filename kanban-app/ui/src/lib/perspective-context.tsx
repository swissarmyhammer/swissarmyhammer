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
import { useDispatchCommand } from "@/lib/command-scope";
import type { PerspectiveDef } from "@/types/kanban";
import { useUIState } from "./ui-state-context";
import { useViews } from "./views-context";

/** This window's label — stable for the lifetime of the window. */
const WINDOW_LABEL = getCurrentWindow().label;

/** Payload for `entity-field-changed` — carries the exact field delta. */
interface EntityFieldChangedEvent {
  entity_type: string;
  id: string;
  fields?: Record<string, unknown>;
}

/**
 * Apply a field delta to the perspective with matching id, preserving object
 * identity for every other perspective in the list.
 *
 * This is how a field change should propagate: the backend tells us exactly
 * which entity changed and which fields changed, so we mutate only that one
 * perspective's object and leave the rest untouched. React consumers that
 * depend on unchanged perspectives never see new references, so they don't
 * re-render.
 */
function applyFieldDelta(
  prev: PerspectiveDef[],
  id: string,
  fields: Record<string, unknown>,
): PerspectiveDef[] {
  const idx = prev.findIndex((p) => p.id === id);
  if (idx < 0) {
    console.warn("[filter-diag] perspective applyFieldDelta MISS", {
      id,
      knownIds: prev.map((p) => p.id),
    });
    return prev;
  }
  const beforeFilter = (prev[idx] as { filter?: unknown }).filter;
  const updated = { ...prev[idx], ...fields } as PerspectiveDef;
  const afterFilter = (updated as { filter?: unknown }).filter;
  console.warn("[filter-diag] perspective applyFieldDelta", {
    id,
    changedFields: Object.keys(fields),
    beforeFilter,
    afterFilter,
  });
  const next = prev.slice();
  next[idx] = updated;
  return next;
}

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
/** Fetches perspectives on mount and exposes a refresh callback + loaded flag. */
function usePerspectivesFetch(
  dispatch: (
    cmd: string,
    opts?: { args?: Record<string, unknown> },
  ) => Promise<unknown>,
): {
  perspectives: PerspectiveDef[];
  setPerspectives: React.Dispatch<React.SetStateAction<PerspectiveDef[]>>;
  loaded: boolean;
  refresh: () => Promise<void>;
} {
  const [perspectives, setPerspectives] = useState<PerspectiveDef[]>([]);
  const [loaded, setLoaded] = useState(false);

  const refresh = useCallback(async () => {
    console.warn("[filter-diag] perspective REFRESH (full refetch)");
    try {
      const wrapped = (await dispatch("perspective.list")) as {
        result?: { perspectives?: PerspectiveDef[] };
      };
      const list = wrapped?.result?.perspectives ?? [];
      console.warn("[filter-diag] perspective REFRESH complete", {
        count: list.length,
        ids: list.map((p) => p.id),
      });
      setPerspectives(list);
      setLoaded(true);
    } catch (error) {
      console.error("Failed to load perspectives:", error);
    }
  }, [dispatch]);

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

  return { perspectives, setPerspectives, loaded, refresh };
}

/**
 * Auto-create a "Default" perspective when none exist for the current view
 * kind. Uses a ref to avoid repeated creation attempts per kind.
 */
function useAutoCreateDefaultPerspective(
  loaded: boolean,
  perspectives: PerspectiveDef[],
  viewKind: string,
  dispatch: (
    cmd: string,
    opts?: { args?: Record<string, unknown> },
  ) => Promise<unknown>,
) {
  const autoCreatedForKindRef = useRef<string | null>(null);
  useEffect(() => {
    if (!loaded) return;
    if (autoCreatedForKindRef.current === viewKind) return;
    if (perspectives.some((p) => p.view === viewKind)) return;
    autoCreatedForKindRef.current = viewKind;
    dispatch("perspective.save", {
      args: { name: "Default", view: viewKind },
    }).catch(console.error);
  }, [loaded, perspectives, viewKind, dispatch]);
}

/**
 * Wire event listeners for perspective entity updates.
 *
 * - entity-field-changed: apply the delta in place, preserving identity for
 *   unchanged perspectives. Crucial during active editing — a full refetch
 *   here would churn every downstream consumer on every keystroke's save.
 * - entity-created/removed, board-changed: full refetch because list shape
 *   changed.
 */
function usePerspectiveEventListeners(
  setPerspectives: React.Dispatch<React.SetStateAction<PerspectiveDef[]>>,
  refresh: () => Promise<void>,
) {
  useEffect(() => {
    const unlisteners = [
      listen<EntityFieldChangedEvent>("entity-field-changed", (event) => {
        console.warn("[filter-diag] event entity-field-changed", {
          entity_type: event.payload.entity_type,
          id: event.payload.id,
          fieldKeys: event.payload.fields
            ? Object.keys(event.payload.fields)
            : null,
        });
        if (event.payload.entity_type !== "perspective") return;
        if (!event.payload.fields) {
          console.warn(
            "[filter-diag] event entity-field-changed perspective WITHOUT fields — falling back to refetch",
            { id: event.payload.id },
          );
          refresh();
          return;
        }
        setPerspectives((prev) =>
          applyFieldDelta(prev, event.payload.id, event.payload.fields!),
        );
      }),
      listen<{ entity_type: string }>("entity-created", (event) => {
        if (event.payload.entity_type === "perspective") {
          console.warn("[filter-diag] event entity-created → refresh");
          refresh();
        }
      }),
      listen<{ entity_type: string }>("entity-removed", (event) => {
        if (event.payload.entity_type === "perspective") {
          console.warn("[filter-diag] event entity-removed → refresh");
          refresh();
        }
      }),
      listen("board-changed", () => {
        console.warn("[filter-diag] event board-changed → refresh");
        refresh();
      }),
    ];
    return () => {
      for (const p of unlisteners) p.then((fn) => fn());
    };
  }, [setPerspectives, refresh]);
}

export function PerspectiveProvider({ children }: { children: ReactNode }) {
  const uiState = useUIState();
  const active_perspective_id =
    uiState.windows?.[WINDOW_LABEL]?.active_perspective_id ?? "";
  const { activeView } = useViews();
  const viewKind = activeView?.kind ?? "board";
  const dispatch = useDispatchCommand();

  const { perspectives, setPerspectives, loaded, refresh } =
    usePerspectivesFetch(dispatch);
  useAutoCreateDefaultPerspective(loaded, perspectives, viewKind, dispatch);
  usePerspectiveEventListeners(setPerspectives, refresh);

  const setActivePerspectiveId = useCallback(
    (id: string) => {
      dispatch("ui.perspective.set", {
        args: { perspective_id: id },
      }).catch(console.error);
    },
    [dispatch],
  );

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
