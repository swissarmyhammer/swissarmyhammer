/**
 * Shared selector for the per-window perspective filter.
 *
 * The backend's `perspective.switch` command pre-evaluates the active
 * perspective's filter DSL and writes the matching task ids into
 * `UIState.windows[label].filtered_task_ids` atomically with the
 * perspective change. Both BoardView and GridView need to respect that
 * filter, so the intersection logic lives here once and is consumed by
 * both call sites — fixing the regression where GridView previously
 * bypassed the filter by reading directly from the entity store.
 *
 * The contract is tri-state (see also `WindowStateSnapshot` in
 * `ui-state-context.tsx`):
 *
 *   * `filtered_task_ids === undefined` — no `perspective.switch` has
 *     fired for this window yet (fresh launch, legacy snapshot, brand-
 *     new board). Treat as "no filter": pass every task through. The
 *     auto-select hook in `perspective-context.tsx` redispatches in
 *     this state so the field gets populated on the next tick.
 *   * `filtered_task_ids === []` — switch has fired but the active
 *     filter matched zero tasks. Honor it: return an empty list.
 *     (Falling back to "show all" here would silently disable
 *     filters.)
 *   * `filtered_task_ids` is a non-empty list — intersect.
 *
 * The filter is task-specific by construction (the DSL evaluates over
 * tasks). For non-task entity types this hook is a no-op pass-through.
 */

import { useMemo } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useUIState } from "@/lib/ui-state-context";
import type { Entity } from "@/types/kanban";

/** This window's label — stable for the lifetime of the window. */
const WINDOW_LABEL = getCurrentWindow().label;

/**
 * Filter a flat list of entities by the active perspective's filter.
 *
 * Only `entityType === "task"` is intersected — other entity types pass
 * through unchanged because the perspective DSL operates over tasks only.
 *
 * @param entities - Raw entity list from `useEntityStore().getEntities(entityType)`.
 * @param entityType - The entity type these entities belong to.
 * @returns The intersected list (or `entities` unchanged when not tasks
 *          or when no perspective filter is active).
 */
export function useFilteredEntities(
  entities: Entity[],
  entityType: string,
): Entity[] {
  const uiState = useUIState();
  const filteredIds = uiState.windows?.[WINDOW_LABEL]?.filtered_task_ids;
  return useMemo(() => {
    if (entityType !== "task") return entities;
    if (filteredIds === undefined) return entities;
    const allowed = new Set(filteredIds);
    return entities.filter((e) => allowed.has(e.id));
  }, [entities, entityType, filteredIds]);
}
