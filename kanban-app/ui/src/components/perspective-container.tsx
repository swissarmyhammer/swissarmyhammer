/**
 * PerspectiveContainer owns the active perspective application.
 *
 * Reads the active perspective from PerspectivesContext and provides:
 * - `applySort(entities)` — applies the perspective's sort entries
 * - `groupField` — the perspective's group-by field name (if any)
 * - `activePerspective` — the full active PerspectiveDef
 *
 * Filter evaluation runs entirely server-side as part of the
 * `perspective.switch` command (see 01KP3ERHEDP86C2JYYR7NM1593): the
 * backend writes the matching task ids into `UIState.filtered_task_ids`
 * atomically with the new `active_perspective_id`, and the views
 * (`view-container.tsx`) intersect that id list with the canonical task
 * list. The frontend no longer fetches via `list_entities(filter=...)`
 * on perspective change — the active perspective's filter is already
 * baked into the UIState snapshot the view consumes.
 *
 * Owns a CommandScopeProvider with moniker `perspective:{activePerspectiveId}`.
 *
 * Hierarchy:
 * ```
 * PerspectivesContainer
 *   └─ PerspectiveContainer            ← this component
 *        └─ [BoardView | GridView]
 * ```
 */

import {
  createContext,
  useCallback,
  useContext,
  useMemo,
  type ReactNode,
} from "react";
import { usePerspectives } from "@/lib/perspective-context";
import { evaluateSort } from "@/lib/perspective-eval";
import { CommandScopeProvider } from "@/lib/command-scope";
import { FocusScope } from "@/components/focus-scope";
import { useOptionalEnclosingLayerFq } from "@/components/layer-fq-context";
import { useOptionalSpatialFocusActions } from "@/lib/spatial-focus-context";
import { asSegment } from "@/types/spatial";
import type { Entity, PerspectiveDef } from "@/types/kanban";

// ---------------------------------------------------------------------------
// Context
// ---------------------------------------------------------------------------

interface ActivePerspectiveContextValue {
  /** The active PerspectiveDef, or null when none is selected. */
  activePerspective: PerspectiveDef | null;
  /** Apply the active perspective's sort entries to an entity array. */
  applySort: (entities: Entity[]) => Entity[];
  /** The active perspective's group-by field name, or undefined. */
  groupField: string | undefined;
}

const ActivePerspectiveContext =
  createContext<ActivePerspectiveContextValue | null>(null);

// ---------------------------------------------------------------------------
// Hook
// ---------------------------------------------------------------------------

/**
 * Read the active perspective context.
 *
 * Must be called inside a PerspectiveContainer. Provides the active
 * perspective plus `applySort` and `groupField` helpers so views don't
 * need to import perspective-eval directly.
 */
export function useActivePerspective(): ActivePerspectiveContextValue {
  const ctx = useContext(ActivePerspectiveContext);
  if (!ctx) {
    throw new Error(
      "useActivePerspective must be used within PerspectiveContainer",
    );
  }
  return ctx;
}

// ---------------------------------------------------------------------------
// Container
// ---------------------------------------------------------------------------

/** Props for the PerspectiveContainer — wraps children with active perspective context and scope. */
interface PerspectiveContainerProps {
  children: ReactNode;
}

/**
 * Applies the active perspective's sort/group and provides the results via
 * context. Wraps children in a CommandScopeProvider with a `perspective:{id}`
 * moniker for command routing.
 */
export function PerspectiveContainer({ children }: PerspectiveContainerProps) {
  const { activePerspective } = usePerspectives();

  const perspectiveId = activePerspective?.id ?? "default";
  const scopeMoniker = useMemo(
    () => `perspective:${perspectiveId}`,
    [perspectiveId],
  );

  // Filter evaluation is server-side: `perspective.switch` pre-computes the
  // matching task ids into `UIState.filtered_task_ids` and the view layer
  // (see `view-container.tsx`) intersects that list with the canonical
  // tasks. No `refreshEntities` roundtrip is needed when the perspective
  // (or its filter) changes — the UIState snapshot already carries the
  // result. See 01KP3ERHEDP86C2JYYR7NM1593 for the migration that retired
  // the prior frontend-side `useEffect` filter-fetch.

  const sortEntries = activePerspective?.sort;
  const groupField = activePerspective?.group;

  const applySort = useCallback(
    (entities: Entity[]) => evaluateSort(sortEntries ?? [], entities),
    [sortEntries],
  );

  const value = useMemo<ActivePerspectiveContextValue>(
    () => ({
      activePerspective,
      applySort,
      groupField,
    }),
    [activePerspective, applySort, groupField],
  );

  return (
    <CommandScopeProvider moniker={scopeMoniker}>
      <ActivePerspectiveContext.Provider value={value}>
        <PerspectiveSpatialZone>{children}</PerspectiveSpatialZone>
      </ActivePerspectiveContext.Provider>
    </CommandScopeProvider>
  );
}

/**
 * Wrap the active perspective body in a `<FocusZone moniker={asSegment("ui:perspective")}>`
 * when the surrounding tree mounts the spatial-nav stack.
 *
 * `<FocusZone>` enforces a strict contract — it throws when no `<FocusLayer>`
 * ancestor is present. That contract is correct for the production tree
 * (`App.tsx` always mounts the providers) but would force every
 * `PerspectiveContainer` unit test that doesn't care about spatial nav to
 * set up the providers. Conditionally rendering the zone when both context
 * lookups succeed keeps the strict contract intact for direct
 * `<FocusZone>` usage while letting the existing test suite keep its narrow
 * provider tree.
 *
 * The zone preserves the `flex flex-col flex-1 min-h-0 min-w-0` chain so the
 * nested view body (BoardView / GridView and its own viewport-sized
 * `ui:board` / `ui:grid` chrome zone) can keep filling the available space
 * when the spatial-nav stack is present.
 */
function PerspectiveSpatialZone({ children }: { children: ReactNode }) {
  const layerKey = useOptionalEnclosingLayerFq();
  const actions = useOptionalSpatialFocusActions();
  if (!layerKey || !actions) {
    return <>{children}</>;
  }
  return (
    <FocusScope
      moniker={asSegment("ui:perspective")}
      // Viewport-sized chrome zone — a visible focus bar around the entire
      // perspective body would frame the whole window and add no signal.
      // The zone exists so the navigator can drill *into* it from the bar
      // and remember a last-focused inner leaf; the leaves it contains
      // (board columns, grid cells, etc.) render their own indicator.
      // showFocus=false: viewport-sized chrome; inner board / grid leaves own focus.
      showFocus={false}
      className="flex flex-col flex-1 min-h-0 min-w-0"
    >
      {children}
    </FocusScope>
  );
}
