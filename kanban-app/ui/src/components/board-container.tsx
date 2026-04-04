/**
 * BoardContainer owns board-level context and commands.
 *
 * Renders a loading spinner when data is being fetched, a "no board loaded"
 * placeholder when no board is active, and wraps children with board data
 * when a board is loaded.
 *
 * Owns:
 * - CommandScopeProvider moniker="board:{boardId}"
 * - FileDropProvider
 * - DragSessionProvider
 * - Conditional rendering: loading / no-board / board-active
 * - BoardContext with useBoardContext() hook
 *
 * Does NOT own:
 * - AppShell (belongs in WindowContainer)
 * - Inspector panel state (stays in AppContent)
 * - Entity state (owned by RustEngineContainer)
 *
 * Hierarchy:
 * ```
 * WindowContainer (window:main)
 *   └─ RustEngineContainer (engine)
 *        └─ StoreContainer (store:/path/to/.kanban)
 *             └─ BoardContainer (board:b1)  ← this component
 *                  └─ children
 * ```
 */

import { createContext, useContext, useMemo, type ReactNode } from "react";
import { CommandScopeProvider } from "@/lib/command-scope";
import { FileDropProvider } from "@/lib/file-drop-context";
import { DragSessionProvider } from "@/lib/drag-session-context";
import {
  useBoardData,
  useWindowLoading,
  useActiveBoardPath,
} from "@/components/window-container";
import { Loader2 } from "lucide-react";
import type { BoardData } from "@/types/kanban";

// ---------------------------------------------------------------------------
// BoardContext — expose board data to descendants
// ---------------------------------------------------------------------------

interface BoardContextValue {
  /** The active board data. Always non-null inside BoardContainer children. */
  board: BoardData;
  /** The filesystem path of the active board. */
  boardPath: string;
}

const BoardContext = createContext<BoardContextValue | null>(null);

/**
 * Returns the board context provided by BoardContainer.
 * Throws if called outside of a BoardContainer with an active board.
 */
export function useBoardContext(): BoardContextValue {
  const ctx = useContext(BoardContext);
  if (!ctx) {
    throw new Error(
      "useBoardContext must be used inside BoardContainer with an active board",
    );
  }
  return ctx;
}

// ---------------------------------------------------------------------------
// BoardContainer
// ---------------------------------------------------------------------------

interface BoardContainerProps {
  children: ReactNode;
}

/**
 * Board-level container that conditionally renders children based on
 * the board loading state. When a board is active, wraps children with
 * CommandScopeProvider, FileDropProvider, DragSessionProvider, and
 * BoardContext.
 */
export function BoardContainer({ children }: BoardContainerProps) {
  const board = useBoardData();
  const loading = useWindowLoading();
  const activeBoardPath = useActiveBoardPath();

  const boardId = board?.board?.id ?? "unknown";
  const moniker = useMemo(() => `board:${boardId}`, [boardId]);

  const contextValue = useMemo<BoardContextValue | null>(
    () =>
      board && activeBoardPath ? { board, boardPath: activeBoardPath } : null,
    [board, activeBoardPath],
  );

  // Loading state — show spinner
  if (loading) {
    return (
      <main role="status" className="flex-1 flex items-center justify-center">
        <Loader2 className="h-8 w-8 text-muted-foreground/50 animate-spin [animation-delay:200ms] [animation-fill-mode:backwards]" />
      </main>
    );
  }

  // No board loaded — show placeholder
  if (!board || !activeBoardPath) {
    return (
      <main className="flex-1 flex items-center justify-center">
        <div className="text-center space-y-3">
          <p className="text-muted-foreground text-lg">No board loaded</p>
          <div className="text-sm text-muted-foreground/70 space-y-1">
            <p>
              <kbd className="px-1.5 py-0.5 rounded bg-muted text-xs font-mono">
                Cmd+N
              </kbd>{" "}
              New Board
            </p>
            <p>
              <kbd className="px-1.5 py-0.5 rounded bg-muted text-xs font-mono">
                Cmd+O
              </kbd>{" "}
              Open Board
            </p>
          </div>
        </div>
      </main>
    );
  }

  // Board active — wrap children with providers and context
  return (
    <CommandScopeProvider commands={[]} moniker={moniker}>
      <FileDropProvider>
        <DragSessionProvider>
          <BoardContext.Provider value={contextValue}>
            {children}
          </BoardContext.Provider>
        </DragSessionProvider>
      </FileDropProvider>
    </CommandScopeProvider>
  );
}
