import { arrayMove } from "@dnd-kit/sortable";

export interface ColumnOrderUpdate {
  id: string;
  order: number;
}

/**
 * Given the current column IDs in display order, compute the order updates
 * needed after moving a column from `fromIndex` to `toIndex`.
 *
 * Returns an array of { id, order } for every column whose order changed.
 * Returns empty array if fromIndex === toIndex (no-op).
 */
export function reorderColumns(
  columnIds: string[],
  fromIndex: number,
  toIndex: number
): ColumnOrderUpdate[] {
  if (fromIndex === toIndex) return [];

  const reordered = arrayMove(columnIds, fromIndex, toIndex);
  return reordered.map((id, index) => ({ id, order: index }));
}
