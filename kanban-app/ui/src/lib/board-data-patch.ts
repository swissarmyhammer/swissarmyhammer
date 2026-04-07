import type { BoardData, Entity } from "@/types/kanban";

/**
 * Patch BoardData in response to an entity-field-changed event for structural
 * entity types (board, column, swimlane).
 *
 * Returns a new BoardData with the updated entity, or null if the entity type
 * is not structural (i.e. doesn't belong in BoardData).
 *
 * This keeps board-view in sync without a full refresh when column names,
 * column order, board names, or swimlane properties change.
 */
export function patchBoardData(
  board: BoardData | null,
  entityType: string,
  id: string,
  entity: Entity,
): BoardData | null {
  if (!board) return null;

  const replaceById = (entities: Entity[]) =>
    entities.map((e) => (e.id === id ? entity : e));

  if (entityType === "board") {
    return { ...board, board: entity };
  }
  if (entityType === "column") {
    return { ...board, columns: replaceById(board.columns) };
  }
  if (entityType === "swimlane") {
    return { ...board, swimlanes: replaceById(board.swimlanes) };
  }

  return null;
}
