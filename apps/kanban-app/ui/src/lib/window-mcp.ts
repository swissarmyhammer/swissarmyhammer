/**
 * Typed wrappers over the in-process `window` MCP server's board-management
 * reads.
 *
 * The Rust-side `WindowService`
 * (`crates/swissarmyhammer-window-service/src/service.rs`) advertises one
 * operation tool named `window` and dispatches on the `op` verb. Alongside the
 * window-manager / OS-file / board-lifecycle verbs it hosts the two multi-board
 * management reads:
 *
 *   - `list open boards` — enumerate the open-board set, marking the active one.
 *   - `get board data` — project one board's aggregate summary.
 *
 * These reads ride the `window` server because it already owns the full
 * open/close/new/switch board lifecycle and is AppHandle-backed — the read
 * counterparts belong alongside the writes. (The per-board `entity` server
 * cannot host them: they span the whole open set / resolve a handle across it.)
 * These wrappers are the single seam the React tree uses to reach them —
 * components never build a raw `command_tool_call` payload themselves, and never
 * `invoke("list_open_boards", …)` / `invoke("get_board_data", …)` directly (the
 * `no-direct-invoke` guardrail enforces this).
 */

import { callMcpTool } from "@/lib/mcp-transport";
import type { BoardDataResponse, OpenBoard } from "@/types/kanban";

/** The MCP tool name (and module id) for the window server. */
export const WINDOW_TOOL = "window" as const;

/** Verb constant for the window server's `list open boards` op. */
export const LIST_OPEN_BOARDS_OP = "list open boards" as const;

/** Verb constant for the window server's `get board data` op. */
export const GET_BOARD_DATA_OP = "get board data" as const;

/** Envelope shape returned by the `window` server's `list open boards` op. */
interface ListOpenBoardsResult {
  ok: boolean;
  boards: OpenBoard[];
}

/**
 * Enumerate the currently open boards from the in-process `window` MCP server.
 *
 * Routes `tools/call("window", { op: "list open boards" })` through the generic
 * MCP transport and unwraps the `{ ok, boards }` envelope so callers receive
 * the raw `OpenBoard[]` — behaviorally identical to the legacy
 * `invoke<OpenBoard[]>("list_open_boards")` Tauri command this replaces.
 *
 * @returns The open boards, each marked with its `is_active` flag.
 */
export async function listOpenBoards(): Promise<OpenBoard[]> {
  const result = await callMcpTool<ListOpenBoardsResult | null>(
    WINDOW_TOOL,
    LIST_OPEN_BOARDS_OP,
  );
  // Tolerate null/undefined envelopes from test stubs that didn't model the
  // `{ ok, boards }` wrap shape — return an empty list so callers can branch
  // on "no boards" rather than crash on a null array.
  return result?.boards ?? [];
}

/**
 * Project one board's aggregate summary from the in-process `window` MCP server.
 *
 * Routes `tools/call("window", { op: "get board data", board_path })` through
 * the generic MCP transport. The server merges `ok: true` into the projection,
 * so the returned object is the same `{ board, columns, tags, virtual_tag_meta,
 * summary }` shape the legacy `invoke<BoardDataResponse>("get_board_data", …)`
 * Tauri command produced.
 *
 * @param boardPath - The board to summarize. When omitted, the server resolves
 *   the active board (matching the original `resolve_handle(None)` fallback).
 * @returns The board's aggregate summary projection.
 */
export async function getBoardData(
  boardPath?: string,
): Promise<BoardDataResponse> {
  return callMcpTool<BoardDataResponse>(
    WINDOW_TOOL,
    GET_BOARD_DATA_OP,
    boardPath ? { board_path: boardPath } : {},
  );
}
