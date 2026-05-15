/**
 * Perspective sort evaluation.
 *
 * Sort entries use multi-level comparison with locale-aware string ordering.
 * Filter evaluation has moved to the backend (server-side DSL evaluation in
 * `list_entities`).
 *
 * All functions are pure — no React dependencies — so they can be called from
 * `useMemo` in view components.
 */

import type { Entity } from "@/types/kanban";
import type { PerspectiveSortEntry } from "@/types/kanban";

/**
 * Compare two field values for sorting.
 *
 * Handles string (locale-aware), number, and fallback toString comparison.
 * Missing/undefined values sort before defined values.
 */
function compareValues(a: unknown, b: unknown): number {
  // Both missing — equal
  if (a == null && b == null) return 0;
  // Missing sorts before defined
  if (a == null) return -1;
  if (b == null) return 1;

  // Number comparison
  if (typeof a === "number" && typeof b === "number") {
    return a - b;
  }

  // String comparison (locale-aware)
  const sa = String(a);
  const sb = String(b);
  return sa.localeCompare(sb);
}

/**
 * Sort entities by multiple fields with asc/desc direction.
 *
 * Returns a new array — does not mutate the input. Ties on the first sort
 * entry are broken by subsequent entries.
 */
export function evaluateSort(
  sort: readonly PerspectiveSortEntry[],
  entities: Entity[],
): Entity[] {
  if (sort.length === 0) return entities;

  return [...entities].sort((a, b) => {
    for (const entry of sort) {
      const va = a.fields[entry.field];
      const vb = b.fields[entry.field];
      let cmp = compareValues(va, vb);
      if (entry.direction === "desc") cmp = -cmp;
      if (cmp !== 0) return cmp;
    }
    return 0;
  });
}
