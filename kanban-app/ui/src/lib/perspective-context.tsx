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

/**
 * Payload shape for the `entity-field-changed` Tauri event, limited to
 * the keys this listener reads.
 *
 * The backend bridge (`kanban-app/src/watcher.rs::process_perspective_event`)
 * emits `WatchEvent::EntityFieldChanged { entity_type, id, changes }` for
 * perspective field changes, with `value = Value::Null` on every change
 * because the frontend is expected to re-fetch via `perspective.list`
 * rather than trust the wire values. There is no field-delta fast path
 * for perspectives on this channel.
 */
interface EntityFieldChangedEvent {
  entity_type: string;
  id: string;
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

/**
 * Shape of the dispatch function this module consumes.
 *
 * Pulled out as an alias so the `useRef<DispatchFn>` declarations don't
 * repeat the full signature in three places.
 */
type DispatchFn = (
  cmd: string,
  opts?: { args?: Record<string, unknown> },
) => Promise<unknown>;

/**
 * Keep a ref tracking the latest `dispatch` so callbacks can read
 * `ref.current` without depending on `dispatch` identity.
 *
 * `useDispatchCommand` returns a new callback whenever the effective scope
 * rotates — which happens on every `ui.setFocus` because `FocusedScopeContext`
 * rotates per-entity focus change. A raw dependency on `dispatch` therefore
 * churns the mount fetch / auto-create / auto-select effects on every
 * keystroke. Reading through a ref decouples those effects from dispatch
 * identity while still picking up the freshest dispatch each invocation.
 */
function useDispatchRef(
  dispatch: DispatchFn,
): React.MutableRefObject<DispatchFn> {
  const dispatchRef = useRef<DispatchFn>(dispatch);
  useEffect(() => {
    dispatchRef.current = dispatch;
  }, [dispatch]);
  return dispatchRef;
}

/** Provider that manages the perspectives list for this window.
 *
 * Follows the ViewsProvider pattern: self-contained state, own data fetching
 * via `perspective.list` command, event-driven refresh, and active perspective
 * selection from UIState.
 */
/**
 * Fetches perspectives on mount and exposes a refresh callback + loaded flag.
 *
 * `refresh` is stable (depends only on the ref object's identity, which
 * never changes) and reads `dispatchRef.current` so the mount-time
 * `useEffect(() => refresh(), [refresh])` fires exactly once per provider
 * mount — not once per focus change.
 */
function usePerspectivesFetch(
  dispatchRef: React.MutableRefObject<DispatchFn>,
): {
  perspectives: PerspectiveDef[];
  loaded: boolean;
  refresh: () => Promise<void>;
} {
  const [perspectives, setPerspectives] = useState<PerspectiveDef[]>([]);
  const [loaded, setLoaded] = useState(false);

  const refresh = useCallback(async () => {
    try {
      const wrapped = (await dispatchRef.current("perspective.list")) as {
        result?: { perspectives?: PerspectiveDef[] };
      };
      const list = wrapped?.result?.perspectives ?? [];
      setPerspectives(list);
      setLoaded(true);
    } catch (error) {
      console.error("Failed to load perspectives:", error);
    }
    // dispatchRef itself is a stable ref object; its identity never changes,
    // so `refresh` is stable across renders.
  }, [dispatchRef]);

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

  return { perspectives, loaded, refresh };
}

/**
 * Auto-create a "Default" perspective when none exist for the current view
 * kind. Uses a ref to avoid repeated creation attempts per kind.
 *
 * `dispatch` is read through `dispatchRef` so the effect does not re-run on
 * every focus change. The only legitimate re-entry signals are `loaded`,
 * `perspectives`, and `viewKind` — they pick up real state changes.
 */
function useAutoCreateDefaultPerspective(
  loaded: boolean,
  perspectives: PerspectiveDef[],
  viewKind: string,
  dispatchRef: React.MutableRefObject<DispatchFn>,
) {
  const autoCreatedForKindRef = useRef<string | null>(null);
  useEffect(() => {
    if (!loaded) return;
    if (autoCreatedForKindRef.current === viewKind) return;
    if (perspectives.some((p) => p.view === viewKind)) return;
    autoCreatedForKindRef.current = viewKind;
    dispatchRef
      .current("perspective.save", {
        args: { name: "Default", view: viewKind },
      })
      .catch(console.error);
  }, [loaded, perspectives, viewKind, dispatchRef]);
}

/**
 * Keep `UIState.active_perspective_id` in sync with a real perspective for
 * the current view kind.
 *
 * The invariant: whenever at least one perspective exists for the active
 * view kind, `UIState.active_perspective_id(window_label)` refers to one
 * of them. If the stored id is empty or names a perspective that doesn't
 * exist (deleted, or for a different view kind), dispatch
 * `perspective.set` for the first matching perspective.
 *
 * Runs in tandem with [`useAutoCreateDefaultPerspective`]. When no
 * perspectives exist for the current view kind, that hook creates a
 * "Default"; the list update fires this hook, which then selects it.
 *
 * The existing `activePerspective` memo in [`PerspectiveProvider`] keeps a
 * synchronous fallback (`perspectives[0]`) so the render happening *during*
 * this dispatch round-trip still shows a perspective instead of flickering
 * to "none selected".
 */
function useAutoSelectActivePerspective(
  loaded: boolean,
  perspectives: PerspectiveDef[],
  active_perspective_id: string,
  viewKind: string,
  dispatchRef: React.MutableRefObject<DispatchFn>,
) {
  useEffect(() => {
    if (!loaded) return;
    const matching = perspectives.filter((p) => p.view === viewKind);
    if (matching.length === 0) {
      // No perspectives for this view kind yet; let
      // useAutoCreateDefaultPerspective create one first.
      return;
    }
    const stillValid = matching.some((p) => p.id === active_perspective_id);
    if (stillValid) return;
    dispatchRef
      .current("perspective.set", {
        args: { perspective_id: matching[0].id },
      })
      .catch(console.error);
    // dispatchRef is a stable object; its `.current` is read lazily. The real
    // re-entry signals are loaded / perspectives / active id / viewKind.
  }, [loaded, perspectives, active_perspective_id, viewKind, dispatchRef]);
}

/**
 * Wire event listeners for perspective entity updates.
 *
 * All four events for a perspective — created, field-changed, removed,
 * board-changed — trigger a full `perspective.list` re-fetch. The backend
 * bridge emits `entity-field-changed` with empty/null values (the wire
 * format is `{ entity_type, id, changes }` where each change carries a
 * `Value::Null` placeholder) because perspective values are re-fetched
 * from the canonical YAML, not patched from the event payload. Given that
 * semantic, there is no usable field-delta fast path for perspectives,
 * so every event is a refetch signal.
 */
function usePerspectiveEventListeners(refresh: () => Promise<void>) {
  useEffect(() => {
    const unlisteners = [
      listen<EntityFieldChangedEvent>("entity-field-changed", (event) => {
        if (event.payload.entity_type !== "perspective") return;
        refresh();
      }),
      listen<{ entity_type: string }>("entity-created", (event) => {
        if (event.payload.entity_type === "perspective") {
          refresh();
        }
      }),
      listen<{ entity_type: string }>("entity-removed", (event) => {
        if (event.payload.entity_type === "perspective") {
          refresh();
        }
      }),
      listen("board-changed", () => {
        refresh();
      }),
    ];
    return () => {
      for (const p of unlisteners) p.then((fn) => fn());
    };
  }, [refresh]);
}

export function PerspectiveProvider({ children }: { children: ReactNode }) {
  const uiState = useUIState();
  const active_perspective_id =
    uiState.windows?.[WINDOW_LABEL]?.active_perspective_id ?? "";
  const { activeView } = useViews();
  const viewKind = activeView?.kind ?? "board";
  const dispatch = useDispatchCommand();

  // Route every perspective-owned effect through a ref so focus-change
  // driven `dispatch` identity churn does not re-run mount / auto-create /
  // auto-select. Backend-event refetches still fire via
  // `usePerspectiveEventListeners` below.
  const dispatchRef = useDispatchRef(dispatch);

  const { perspectives, loaded, refresh } = usePerspectivesFetch(dispatchRef);
  useAutoCreateDefaultPerspective(loaded, perspectives, viewKind, dispatchRef);
  useAutoSelectActivePerspective(
    loaded,
    perspectives,
    active_perspective_id,
    viewKind,
    dispatchRef,
  );
  usePerspectiveEventListeners(refresh);

  const setActivePerspectiveId = useCallback(
    (id: string) => {
      dispatchRef
        .current("perspective.set", {
          args: { perspective_id: id },
        })
        .catch(console.error);
      // Reads through the ref so this callback's identity does not churn
      // on focus change (would otherwise cascade to memoized consumers).
    },
    [dispatchRef],
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
