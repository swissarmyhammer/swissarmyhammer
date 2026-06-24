/**
 * Cross-window drag session context.
 *
 * Subscribes to the drag lifecycle on the MCP notification bridge
 * (`notifications/ui_state/drag_started | drag_cancelled | drag_completed`) and
 * exposes the active session (if any) plus helpers for starting/cancelling/
 * completing sessions. The provider registers via the statically-imported
 * `listen` (synchronous registration); the `subscribeDrag*` seams in
 * `mcp-notifications.ts` remain the public plugin/MCP-client subscription API.
 * The webview consumes drag as a pure MCP client — never the legacy direct
 * Tauri `drag-session-*` events.
 */

import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useState,
  type ReactNode,
} from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useDispatchCommand, type DispatchOptions } from "@/lib/command-scope";
import {
  DRAG_STARTED_EVENT,
  DRAG_CANCELLED_EVENT,
  DRAG_COMPLETED_EVENT,
  type DragStarted,
} from "@/lib/mcp-notifications";

type DispatchFn = (cmd: string, opts?: DispatchOptions) => Promise<unknown>;

/**
 * Discriminated union mirroring the Rust-side `DragSource` enum.
 *
 * `focus_chain` is the typical task-drag-from-card source; `file` is an
 * external OS file dragged in from the host desktop. The `kind` tag
 * matches the `#[serde(tag = "kind", rename_all = "snake_case")]`
 * attribute on the Rust enum so a narrowing check on `from.kind` picks
 * the variant's fields off the wire payload directly.
 */
export type DragSource =
  | {
      kind: "focus_chain";
      entity_type: string;
      entity_id: string;
      fields: Record<string, unknown>;
      source_board_path: string;
      source_window_label: string;
    }
  | {
      kind: "file";
      path: string;
    };

/**
 * Payload carried by `notifications/ui_state/drag_started`.
 *
 * The wire payload carries both the legacy flat fields (`task_id`,
 * `source_board_path`, `source_window_label`) and the new `from`
 * discriminated-union envelope. Listeners that already read flat fields
 * keep working for focus-chain drags (file drags leave them empty).
 * New listeners should prefer `from` and narrow on `from.kind` — that's
 * the only shape file drags populate.
 *
 * Flat-field values for file drags:
 * - `task_id` / `source_board_path` — empty strings.
 * - `source_window_label` — the Tauri window that initiated the drag.
 */
export interface DragSession {
  session_id: string;
  source_board_path: string;
  source_window_label: string;
  task_id: string;
  task_fields: Record<string, unknown>;
  copy_mode: boolean;
  /** Discriminated-union drag source — see {@link DragSource}. */
  from: DragSource;
}

interface DragSessionContextValue {
  /** The active cross-window drag session, if any. */
  session: DragSession | null;
  /** Start a drag session (called by the source window). */
  startSession: (
    taskId: string,
    taskFields: Record<string, unknown>,
    copyMode: boolean,
  ) => Promise<void>;
  /**
   * Start a file-source drag session for an OS file dropped into the app.
   *
   * The file path must be absolute — it's normally the temp-file path
   * returned by the `save_dropped_file` Tauri command (which writes the
   * browser-delivered `File` object's bytes out so the Rust side can
   * attach it without a roundtrip through the web layer). The resulting
   * session carries `from.kind === "file"` and the `drag.complete`
   * dispatch is routed through the `PasteMatrix`'s
   * `attachment_onto_task` handler — dropping a file onto a task creates
   * a new attachment, identical to paste.
   */
  startFileSession: (filePath: string, copyMode?: boolean) => Promise<void>;
  /** Cancel the active drag session. */
  cancelSession: () => Promise<void>;
  /** Complete the drag session by dropping in a target column. */
  completeSession: (
    targetColumn: string,
    options?: {
      dropIndex?: number;
      beforeId?: string;
      afterId?: string;
      copyMode?: boolean;
    },
  ) => Promise<void>;
  /**
   * Complete a file-source drag by dispatching to an entity target moniker.
   *
   * `targetMoniker` is the usual `type:id` form — for the
   * `attachment_onto_task` case that's `task:<id>`. Returns when the
   * Rust-side handler has finished writing the new attachment entity.
   */
  completeFileSession: (targetMoniker: string) => Promise<void>;
  /** Whether this window is the source of the active drag. */
  isSource: boolean;
}

const DragSessionContext = createContext<DragSessionContextValue>({
  session: null,
  startSession: async () => {},
  startFileSession: async () => {},
  cancelSession: async () => {},
  completeSession: async () => {},
  completeFileSession: async () => {},
  isSource: false,
});

/** Returns the current drag session state and control methods. Must be used within DragSessionProvider. */
export function useDragSession() {
  return useContext(DragSessionContext);
}

/** Subscribes to the drag lifecycle on the MCP bridge and keeps local state in sync. */
function useDragSessionEvents(
  setSession: (s: DragSession | null) => void,
  setIsSource: (b: boolean) => void,
) {
  useEffect(() => {
    const myLabel = getCurrentWindow().label;
    let cancelled = false;
    const unlisteners: UnlistenFn[] = [];
    const clear = () => {
      setSession(null);
      setIsSource(false);
    };
    // Register the bridge listeners via the statically-imported `listen` (NOT
    // the `subscribeDrag*` seams, which resolve `listen` through a dynamic
    // `import()` — a macrotask hop that registers a tick late and races the
    // event in the chromium test harness; same fix the focus provider applies).
    // `subscribeDrag*` remains the public plugin/MCP-client seam.
    const register = (event: string, handler: (payload: unknown) => void) => {
      listen(event, ({ payload }) => handler(payload)).then((fn) => {
        if (cancelled) fn();
        else unlisteners.push(fn);
      });
    };
    register(DRAG_STARTED_EVENT, (payload) => {
      // The started payload IS the session wire shape (`DragSession`).
      const session = payload as DragStarted as unknown as DragSession;
      setSession(session);
      // Source is identified by window label (not board path — multiple
      // windows can show the same board).
      setIsSource(session.source_window_label === myLabel);
    });
    register(DRAG_CANCELLED_EVENT, clear);
    register(DRAG_COMPLETED_EVENT, clear);
    return () => {
      cancelled = true;
      for (const fn of unlisteners) fn();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
}

/** Callback for starting a focus-chain (task) drag session. */
function useStartSession(
  dispatch: DispatchFn,
  setIsSource: (b: boolean) => void,
) {
  return useCallback(
    async (
      taskId: string,
      taskFields: Record<string, unknown>,
      copyMode: boolean,
    ) => {
      try {
        // Board path is derived from the scope chain's store:{path} moniker
        // by the Rust backend — no explicit boardPath arg needed.
        await dispatch("drag.start", {
          args: {
            taskId,
            taskFields,
            sourceWindowLabel: getCurrentWindow().label,
            copyMode,
          },
        });
        setIsSource(true);
      } catch (e) {
        console.error("Failed to start drag session:", e);
      }
    },
    [dispatch, setIsSource],
  );
}

/** Callback for starting a file-drop drag session. */
function useStartFileSession(
  dispatch: DispatchFn,
  setIsSource: (b: boolean) => void,
) {
  return useCallback(
    async (filePath: string, copyMode = false) => {
      try {
        // sourceKind="file" flips DragStartCmd onto the file-drag construction
        // path. The Rust side validates filePath is absolute before stashing
        // it in the DragSource::File variant.
        await dispatch("drag.start", {
          args: {
            sourceKind: "file",
            filePath,
            sourceWindowLabel: getCurrentWindow().label,
            copyMode,
          },
        });
        setIsSource(true);
      } catch (e) {
        console.error("Failed to start file drag session:", e);
      }
    },
    [dispatch, setIsSource],
  );
}

/** Callback for cancelling the active drag session. */
function useCancelSession(dispatch: DispatchFn) {
  return useCallback(async () => {
    try {
      await dispatch("drag.cancel");
    } catch (e) {
      console.error("Failed to cancel drag session:", e);
    }
  }, [dispatch]);
}

/** Drag-complete dispatch callbacks for focus-chain and file drags. */
function useDragCompleteCallbacks(dispatch: DispatchFn) {
  const completeSession = useCallback(
    async (
      targetColumn: string,
      options?: {
        dropIndex?: number;
        beforeId?: string;
        afterId?: string;
        copyMode?: boolean;
      },
    ) => {
      try {
        // Target board path is derived from the scope chain's store:{path}
        // moniker by the Rust backend — no explicit targetBoardPath arg needed.
        await dispatch("drag.complete", {
          args: {
            targetColumn,
            dropIndex: options?.dropIndex ?? null,
            beforeId: options?.beforeId ?? null,
            afterId: options?.afterId ?? null,
            copyMode: options?.copyMode ?? false,
          },
        });
      } catch (e) {
        console.error("Failed to complete drag session:", e);
      }
    },
    [dispatch],
  );

  const completeFileSession = useCallback(
    async (targetMoniker: string) => {
      try {
        // `drag.complete` reads the active `DragSource::File` session out of
        // UIState and dispatches via the PasteMatrix keyed on
        // `(attachment, <target_type>)`. The target moniker picks the
        // specific drop destination (typically `task:<id>`).
        await dispatch("drag.complete", { target: targetMoniker });
      } catch (e) {
        console.error("Failed to complete file drag session:", e);
      }
    },
    [dispatch],
  );

  return { completeSession, completeFileSession };
}

/** Provides drag session state to component tree. Manages cross-window drag sessions via Tauri events. */
export function DragSessionProvider({ children }: { children: ReactNode }) {
  const dispatch = useDispatchCommand();
  const [session, setSession] = useState<DragSession | null>(null);
  const [isSource, setIsSource] = useState(false);

  useDragSessionEvents(setSession, setIsSource);
  const startSession = useStartSession(dispatch, setIsSource);
  const startFileSession = useStartFileSession(dispatch, setIsSource);
  const cancelSession = useCancelSession(dispatch);
  const { completeSession, completeFileSession } =
    useDragCompleteCallbacks(dispatch);

  return (
    <DragSessionContext.Provider
      value={{
        session,
        startSession,
        startFileSession,
        cancelSession,
        completeSession,
        completeFileSession,
        isSource,
      }}
    >
      {children}
    </DragSessionContext.Provider>
  );
}
