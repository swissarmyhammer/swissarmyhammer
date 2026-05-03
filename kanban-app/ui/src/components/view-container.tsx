/**
 * ViewContainer owns the active view routing and view-level command scope.
 *
 * Reads the active view from ViewsProvider and renders the appropriate
 * view component (BoardView, GridView, or a placeholder for unknown kinds).
 *
 * Owns:
 * - CommandScopeProvider moniker="view:{activeViewId}"
 * - Active view routing (BoardView / GridView / placeholder)
 *
 * Hierarchy:
 * ```
 * ViewsContainer
 *   └─ ViewContainer              ← this component
 *        └─ BoardView | GridView | placeholder
 * ```
 *
 * Spatial nav: ViewContainer does NOT register a spatial zone of its own.
 * The inner view (BoardView, GridView, …) already registers its own
 * viewport-sized chrome zone (`ui:board`, `ui:grid`, …) for the same
 * rect, so an outer view zone would be a redundant graph hop. The view's
 * parent zone in the spatial graph is therefore `ui:perspective`.
 */

import { useMemo, type ReactNode } from "react";
import { useViews } from "@/lib/views-context";
import { CommandScopeProvider } from "@/lib/command-scope";
import { GroupedBoardView } from "@/components/grouped-board-view";
import { GridView } from "@/components/grid-view";
import { useBoardData } from "@/components/window-container";
import { useEntitiesByType } from "@/components/rust-engine-container";
import type { BoardData, Entity } from "@/types/kanban";

// ---------------------------------------------------------------------------
// ViewContainer
// ---------------------------------------------------------------------------

interface ViewContainerProps {
  children?: ReactNode;
}

/**
 * Routes to the correct view component based on the active view kind.
 *
 * Wraps the rendered view in a CommandScopeProvider with a moniker that
 * identifies the active view, enabling view-scoped command resolution.
 *
 * @param children - Optional children rendered after the view (e.g. probes in tests).
 */
export function ViewContainer({ children }: ViewContainerProps) {
  const { activeView } = useViews();
  const board = useBoardData();
  const entitiesByType = useEntitiesByType();

  const viewId = activeView?.id ?? "default";
  const moniker = useMemo(() => `view:${viewId}`, [viewId]);

  return (
    <CommandScopeProvider moniker={moniker}>
      <ActiveViewRenderer
        activeView={activeView}
        board={board!}
        tasks={entitiesByType.task ?? []}
      />
      {children}
    </CommandScopeProvider>
  );
}

// ---------------------------------------------------------------------------
// ActiveViewRenderer — internal routing component
// ---------------------------------------------------------------------------

interface ActiveViewRendererProps {
  activeView: import("@/types/kanban").ViewDef | null;
  board: BoardData;
  tasks: Entity[];
}

/**
 * Renders the currently active view based on its kind.
 *
 * - null or "board" kind: renders BoardView (board path from scope chain)
 * - "grid" kind: renders GridView
 * - anything else: renders a placeholder message
 */
function ActiveViewRenderer({
  activeView,
  board,
  tasks,
}: ActiveViewRendererProps) {
  if (!activeView || activeView.kind === "board") {
    return <GroupedBoardView board={board} tasks={tasks} />;
  }

  if (activeView.kind === "grid") {
    return <GridView view={activeView} />;
  }

  return (
    <main className="flex-1 flex items-center justify-center">
      <p className="text-muted-foreground">
        {activeView.name} view ({activeView.kind}) is not yet implemented.
      </p>
    </main>
  );
}
