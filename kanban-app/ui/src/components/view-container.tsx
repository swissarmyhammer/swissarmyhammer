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
 */

import { useMemo, type ReactNode } from "react";
import { useViews } from "@/lib/views-context";
import { CommandScopeProvider } from "@/lib/command-scope";
import { GroupedBoardView } from "@/components/grouped-board-view";
import { GridView } from "@/components/grid-view";
import { useBoardData } from "@/components/window-container";
import { useEntitiesByType } from "@/components/rust-engine-container";
import { FocusZone } from "@/components/focus-zone";
import { useOptionalEnclosingLayerFq } from "@/components/layer-fq-context";
import { useOptionalSpatialFocusActions } from "@/lib/spatial-focus-context";
import { asSegment } from "@/types/spatial";
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
      <ViewSpatialZone>
        <ActiveViewRenderer
          activeView={activeView}
          board={board!}
          tasks={entitiesByType.task ?? []}
        />
        {children}
      </ViewSpatialZone>
    </CommandScopeProvider>
  );
}

/**
 * Wrap the rendered view in a `<FocusZone moniker={asSegment("ui:view")}>`
 * when the surrounding tree mounts the spatial-nav stack.
 *
 * `<FocusZone>` enforces a strict contract — it throws when no `<FocusLayer>`
 * ancestor is present. That contract is correct for the production tree
 * (`App.tsx` always mounts the providers) but would force every
 * `ViewContainer` unit test that doesn't care about spatial nav to set up
 * the providers. Conditionally rendering the zone when both context lookups
 * succeed keeps the strict contract intact for direct `<FocusZone>` usage
 * while letting the existing test suite keep its narrow provider tree.
 *
 * The zone preserves the `flex-1 flex flex-col min-h-0 min-w-0` chain so the
 * inner BoardView / GridView can keep filling the available space when the
 * spatial-nav stack is present.
 */
function ViewSpatialZone({ children }: { children: ReactNode }) {
  const layerKey = useOptionalEnclosingLayerFq();
  const actions = useOptionalSpatialFocusActions();
  if (!layerKey || !actions) {
    return <>{children}</>;
  }
  return (
    <FocusZone
      moniker={asSegment("ui:view")}
      // Viewport-sized chrome zone — a visible focus bar around the entire
      // active view (board, grid, …) would surround the whole content
      // region and add no information. Drilling into the view advances
      // focus to the body's first leaf; that leaf renders the indicator.
      // `data-focused` still flips on the wrapper so drill-out tests can
      // observe focus landing on the view zone.
      showFocusBar={false}
      className="flex-1 flex flex-col min-h-0 min-w-0"
    >
      {children}
    </FocusZone>
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
