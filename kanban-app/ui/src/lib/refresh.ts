/**
 * Board refresh logic extracted for testability.
 *
 * The key invariant: `list_open_boards` is fetched independently of board
 * data. If `get_board_data` or `list_entities` fails (e.g. newly created
 * board not fully ready), the open boards list still updates. This prevents
 * the board selector from losing previously-open boards when a new board
 * is opened.
 */

import { invoke } from "@tauri-apps/api/core";
import type {
  OpenBoard,
  BoardDataResponse,
  EntityListResponse,
} from "@/types/kanban";
import { entityFromBag, parseBoardData } from "@/types/kanban";
import type { BoardData, Entity } from "@/types/kanban";

export interface RefreshResult {
  openBoards: OpenBoard[];
  boardData: BoardData | null;
  entitiesByType: Record<string, Entity[]> | null;
}

/**
 * Fetch board state from the backend. The open boards list is always
 * populated even if other data fetches fail.
 *
 * @param boardPath — optional board path to scope queries to a specific board
 *   (for multi-window support). When omitted, the backend uses the global active board.
 */
export async function refreshBoards(
  boardPath?: string,
): Promise<RefreshResult> {
  // Always fetch open boards independently — this must not be coupled
  // to get_board_data or list_entities via Promise.all.
  let openBoards: OpenBoard[] = [];
  try {
    openBoards = await invoke<OpenBoard[]>("list_open_boards");
  } catch (error) {
    console.error("Failed to list open boards:", error);
  }

  // Fetch board data and entities — may fail for newly created boards.
  let boardData: BoardData | null = null;
  let entitiesByType: Record<string, Entity[]> | null = null;
  try {
    const bp = boardPath ? { boardPath } : {};
    const [bd, taskData, actorData] = await Promise.all([
      invoke<BoardDataResponse>("get_board_data", bp),
      invoke<EntityListResponse>("list_entities", {
        entityType: "task",
        ...bp,
      }),
      invoke<EntityListResponse>("list_entities", {
        entityType: "actor",
        ...bp,
      }),
    ]);
    boardData = parseBoardData(bd);
    entitiesByType = {
      board: [entityFromBag(bd.board)],
      column: bd.columns.map(entityFromBag),
      tag: bd.tags.map(entityFromBag),
      task: taskData.entities.map(entityFromBag),
      actor: actorData.entities.map(entityFromBag),
    };
  } catch (error) {
    console.error("Failed to load board data:", error);
  }

  return { openBoards, boardData, entitiesByType };
}
