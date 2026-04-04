/**
 * PerspectiveContainer owns the active perspective application.
 *
 * Reads the active perspective from PerspectivesContext and provides:
 * - `applyFilter(entities)` — applies the perspective's filter expression
 * - `applySort(entities)` — applies the perspective's sort entries
 * - `groupField` — the perspective's group-by field name (if any)
 * - `activePerspective` — the full active PerspectiveDef
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
import { evaluateFilter, evaluateSort } from "@/lib/perspective-eval";
import { CommandScopeProvider } from "@/lib/command-scope";
import type { Entity, PerspectiveDef } from "@/types/kanban";

// ---------------------------------------------------------------------------
// Context
// ---------------------------------------------------------------------------

interface ActivePerspectiveContextValue {
  /** The active PerspectiveDef, or null when none is selected. */
  activePerspective: PerspectiveDef | null;
  /** Apply the active perspective's filter expression to an entity array. */
  applyFilter: (entities: Entity[]) => Entity[];
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
 * perspective plus `applyFilter`, `applySort`, and `groupField` helpers
 * so views don't need to import perspective-eval directly.
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

interface PerspectiveContainerProps {
  children: ReactNode;
}

/**
 * Applies the active perspective's filter/sort/group and provides the
 * results via context. Wraps children in a CommandScopeProvider with a
 * `perspective:{id}` moniker for command routing.
 */
export function PerspectiveContainer({ children }: PerspectiveContainerProps) {
  const { activePerspective } = usePerspectives();

  const perspectiveId = activePerspective?.id ?? "default";
  const scopeMoniker = useMemo(
    () => `perspective:${perspectiveId}`,
    [perspectiveId],
  );

  const filter = activePerspective?.filter;
  const sortEntries = activePerspective?.sort;
  const groupField = activePerspective?.group;

  const applyFilter = useCallback(
    (entities: Entity[]) => evaluateFilter(filter, entities),
    [filter],
  );

  const applySort = useCallback(
    (entities: Entity[]) => evaluateSort(sortEntries ?? [], entities),
    [sortEntries],
  );

  const value = useMemo<ActivePerspectiveContextValue>(
    () => ({
      activePerspective,
      applyFilter,
      applySort,
      groupField,
    }),
    [activePerspective, applyFilter, applySort, groupField],
  );

  return (
    <CommandScopeProvider commands={[]} moniker={scopeMoniker}>
      <ActivePerspectiveContext.Provider value={value}>
        {children}
      </ActivePerspectiveContext.Provider>
    </CommandScopeProvider>
  );
}
