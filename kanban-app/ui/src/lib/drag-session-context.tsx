/**
 * Cross-window drag session context.
 *
 * Listens to Tauri drag-session events and exposes the active session
 * (if any) plus helpers for starting/cancelling/completing sessions.
 */

import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useState,
  type ReactNode,
} from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useDispatchCommand } from "@/lib/command-scope";

/** Payload emitted by `drag-session-active`. */
export interface DragSession {
  session_id: string;
  source_board_path: string;
  source_window_label: string;
  task_id: string;
  task_fields: Record<string, unknown>;
  copy_mode: boolean;
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
  /** Whether this window is the source of the active drag. */
  isSource: boolean;
}

const DragSessionContext = createContext<DragSessionContextValue>({
  session: null,
  startSession: async () => {},
  cancelSession: async () => {},
  completeSession: async () => {},
  isSource: false,
});

export function useDragSession() {
  return useContext(DragSessionContext);
}

export function DragSessionProvider({ children }: { children: ReactNode }) {
  const dispatch = useDispatchCommand();

  const [session, setSession] = useState<DragSession | null>(null);
  const [isSource, setIsSource] = useState(false);

  // Listen to Tauri drag session events
  useEffect(() => {
    const myLabel = getCurrentWindow().label;
    const unlisteners = [
      listen<DragSession>("drag-session-active", (event) => {
        setSession(event.payload);
        // We are the source if our window label matches (not board path,
        // since multiple windows can show the same board)
        setIsSource(event.payload.source_window_label === myLabel);
      }),
      listen<{ session_id: string }>("drag-session-cancelled", () => {
        setSession(null);
        setIsSource(false);
      }),
      listen<{ session_id: string; success: boolean }>(
        "drag-session-completed",
        () => {
          setSession(null);
          setIsSource(false);
        },
      ),
    ];
    return () => {
      for (const p of unlisteners) {
        p.then((fn) => fn());
      }
    };
  }, []);

  const startSession = useCallback(
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
    [dispatch],
  );

  const cancelSession = useCallback(async () => {
    try {
      await dispatch("drag.cancel");
    } catch (e) {
      console.error("Failed to cancel drag session:", e);
    }
  }, [dispatch]);

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

  return (
    <DragSessionContext.Provider
      value={{
        session,
        startSession,
        cancelSession,
        completeSession,
        isSource,
      }}
    >
      {children}
    </DragSessionContext.Provider>
  );
}
