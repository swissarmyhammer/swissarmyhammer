/**
 * PerspectiveContainer owns the active perspective application.
 *
 * Reads the active perspective from PerspectivesContext and provides:
 * - `applySort(entities)` — applies the perspective's sort entries
 * - `groupField` — the perspective's group-by field name (if any)
 * - `activePerspective` — the full active PerspectiveDef
 *
 * Filter evaluation is handled server-side by `list_entities` — the frontend
 * no longer applies filters client-side.
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
  useEffect,
  useMemo,
  type ReactNode,
} from "react";
import { usePerspectives } from "@/lib/perspective-context";
import { evaluateSort } from "@/lib/perspective-eval";
import { CommandScopeProvider, useActiveBoardPath } from "@/lib/command-scope";
import { useRefreshEntities } from "@/components/rust-engine-container";
import { FocusZone } from "@/components/focus-zone";
import { useOptionalLayerKey } from "@/components/focus-layer";
import { useOptionalSpatialFocusActions } from "@/lib/spatial-focus-context";
import { asMoniker } from "@/types/spatial";
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
  const boardPath = useActiveBoardPath();
  const refreshEntities = useRefreshEntities();

  const perspectiveId = activePerspective?.id ?? "default";
  const activeFilter = activePerspective?.filter;
  const scopeMoniker = useMemo(
    () => `perspective:${perspectiveId}`,
    [perspectiveId],
  );

  // Re-fetch tasks when the active perspective's filter changes.
  // Fires on mount (if a filtered perspective is active) and whenever the
  // filter value changes (typing, switching perspectives, clearing).
  useEffect(() => {
    if (!boardPath) return;
    refreshEntities(boardPath, activeFilter).catch(console.error);
  }, [activeFilter, boardPath, refreshEntities]);

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
 * Wrap the active perspective body in a `<FocusZone moniker={asMoniker("ui:perspective")}>`
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
 * nested `<ui:view>` zone (and the BoardView/GridView inside it) can keep
 * filling the available space when the spatial-nav stack is present.
 */
function PerspectiveSpatialZone({ children }: { children: ReactNode }) {
  const layerKey = useOptionalLayerKey();
  const actions = useOptionalSpatialFocusActions();
  if (!layerKey || !actions) {
    return <>{children}</>;
  }
  return (
    <FocusZone
      moniker={asMoniker("ui:perspective")}
      showFocusBar={false}
      className="flex flex-col flex-1 min-h-0 min-w-0"
    >
      {children}
    </FocusZone>
  );
}
