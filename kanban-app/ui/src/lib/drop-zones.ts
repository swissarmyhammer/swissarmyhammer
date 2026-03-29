/**
 * Pure, testable drop zone computation for drag-and-drop reordering.
 *
 * Given the sorted task IDs in a column, returns an array of drop zone
 * descriptors. Each descriptor carries the exact placement data the
 * backend needs — no runtime computation at drop time.
 */

/** Describes a single drop zone between (or around) cards. */
export interface DropZoneDescriptor {
  /** Unique key for React rendering. */
  key: string;
  /** Board path for cross-board drops. */
  boardPath: string;
  /** Target column ID. */
  columnId: string;
  /** Place the dropped task before this ID (mutually exclusive with afterId). */
  beforeId?: string;
  /** Place the dropped task after this ID (mutually exclusive with beforeId). */
  afterId?: string;
}

/**
 * Compute drop zone descriptors for a column.
 *
 * For N tasks returns N+1 zones: one "before" zone per task, plus one
 * "after" zone for the last task. For an empty column returns a single
 * zone with no placement (backend will append).
 */
export function computeDropZones(
  taskIds: string[],
  columnId: string,
  boardPath: string,
): DropZoneDescriptor[] {
  if (taskIds.length === 0) {
    return [{ key: "empty", boardPath, columnId }];
  }

  const zones: DropZoneDescriptor[] = [];

  for (const id of taskIds) {
    zones.push({ key: `before-${id}`, boardPath, columnId, beforeId: id });
  }

  const lastId = taskIds[taskIds.length - 1];
  zones.push({ key: `after-${lastId}`, boardPath, columnId, afterId: lastId });

  return zones;
}
