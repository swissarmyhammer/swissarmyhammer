/**
 * Board refresh logic extracted for testability.
 *
 * Three independence invariants, each defended by its own try/catch so one
 * failure never cascades into the others:
 *
 * 1. `list_open_boards` is fetched independently of board data — if board
 *    data fails, the open-boards list still updates, so the board selector
 *    never loses previously-open boards.
 * 2. `get_board_data` is fetched independently of `list_entities` — a
 *    degraded entity list (e.g. a missing entity dir) must NOT null the
 *    board data; the board still renders and only the entity store is empty.
 * 3. A `get_board_data` FAILURE surfaces as `boardError` (a board-level
 *    error state), not a silent `null`. A malformed board whose
 *    `get_board_data` rejects with "entity not found: board/board" would
 *    otherwise blank the window forever; the error state lets the caller
 *    fall back to another open board instead of swallowing it.
 */

import { invoke } from "@tauri-apps/api/core";
import { getBoardData, listOpenBoards } from "@/lib/window-mcp";
import type { OpenBoard, EntityListResponse } from "@/types/kanban";
import { entityFromBag, parseBoardData } from "@/types/kanban";
import type { BoardData, BoardDataResponse, Entity } from "@/types/kanban";

/** Structured result from a board state refresh operation. */
export interface RefreshResult {
  openBoards: OpenBoard[];
  boardData: BoardData | null;
  entitiesByType: Record<string, Entity[]> | null;
  /**
   * Human-readable error when `get_board_data` itself FAILED for the
   * requested board (distinct from a `null` board with no error, which means
   * "no board was requested"). The caller surfaces this as a per-board error
   * state and may fall back to another open board. `null` on success.
   */
  boardError: string | null;
}

/**
 * Fetch board state from the backend. The open boards list is always
 * populated even if other data fetches fail.
 *
 * @param boardPath — optional board path to scope queries to a specific board
 *   (for multi-window support). When omitted, the backend uses the global active board.
 * @param taskFilter — optional filter DSL expression to apply server-side when
 *   listing tasks (e.g. `"#bug && @will"`). When omitted, all tasks are returned.
 */
export async function refreshBoards(
  boardPath?: string,
  taskFilter?: string,
): Promise<RefreshResult> {
  // Always fetch open boards independently — this must not be coupled
  // to get_board_data or list_entities via Promise.all.
  let openBoards: OpenBoard[] = [];
  try {
    openBoards = await listOpenBoards();
  } catch (error) {
    console.error("Failed to list open boards:", error);
  }

  const bp = boardPath ? { boardPath } : {};

  // Fetch board data independently of the entity lists. A `get_board_data`
  // failure is the malformed-board signal — it must surface as `boardError`
  // so the window can fall back, not blank silently. Crucially this is NOT
  // coupled to `list_entities` via a single Promise.all: a degraded entity
  // list must not null the board data.
  let bd: BoardDataResponse | null = null;
  let boardData: BoardData | null = null;
  let boardError: string | null = null;
  try {
    bd = await getBoardData(boardPath);
    boardData = parseBoardData(bd);
  } catch (error) {
    console.error("Failed to load board data:", error);
    boardError = error instanceof Error ? error.message : String(error);
  }

  // Fetch the entity lists in parallel, independently of board data. A
  // failure here leaves the board rendering from its board data with an empty
  // entity store rather than nulling the whole board.
  let entitiesByType: Record<string, Entity[]> | null = null;
  if (bd) {
    try {
      const [taskData, actorData, projectData] = await Promise.all([
        invoke<EntityListResponse>("list_entities", {
          entityType: "task",
          ...(taskFilter ? { filter: taskFilter } : {}),
          ...bp,
        }),
        invoke<EntityListResponse>("list_entities", {
          entityType: "actor",
          ...bp,
        }),
        invoke<EntityListResponse>("list_entities", {
          entityType: "project",
          ...bp,
        }),
      ]);
      entitiesByType = {
        board: [entityFromBag(bd.board)],
        column: bd.columns.map(entityFromBag),
        tag: bd.tags.map(entityFromBag),
        task: taskData.entities.map(entityFromBag),
        actor: actorData.entities.map(entityFromBag),
        project: projectData.entities.map(entityFromBag),
      };
    } catch (error) {
      console.error("Failed to load board entities:", error);
    }
  }

  return { openBoards, boardData, entitiesByType, boardError };
}
