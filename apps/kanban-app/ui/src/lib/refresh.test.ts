import { describe, it, expect, vi, beforeEach } from "vitest";

import {
  wrapMcpDispatch,
  type LegacyDispatcher,
} from "@/test/mcp-invoke-translator";

const mockInvoke = vi.fn((..._args: unknown[]) => Promise.resolve({}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

import { refreshBoards } from "./refresh";

/**
 * Install a legacy `(cmd, args)` dispatcher behind the MCP translator.
 *
 * `refreshBoards` reaches `list_open_boards` / `get_board_data` through the
 * `app` MCP server (`invoke("command_tool_call", …)`), so the legacy verb
 * bodies below run only once the translator unwraps the envelope and records
 * the synthetic `["list_open_boards", …]` / `["get_board_data", { boardPath }]`
 * call on `mockInvoke.mock.calls` — keeping the legacy-shape assertions intact.
 */
function installLegacy(legacy: LegacyDispatcher): void {
  const wrapped = wrapMcpDispatch(mockInvoke, legacy);
  mockInvoke.mockImplementation(async (...args: unknown[]) => {
    const result = await wrapped(args[0] as string, args[1]);
    return (result ?? {}) as Record<string, unknown>;
  });
}

describe("refreshBoards", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("returns open boards even when get_board_data fails", async () => {
    // Simulate: list_open_boards succeeds with 2 boards,
    // but get_board_data fails (new board not fully ready).
    installLegacy((cmd) => {
      if (cmd === "list_open_boards") {
        return Promise.resolve([
          { path: "/a/.kanban", is_active: false, name: "Board A" },
          { path: "/b/.kanban", is_active: true, name: "Board B" },
        ]);
      }
      if (cmd === "get_board_data") {
        return Promise.reject(new Error("No active board"));
      }
      if (cmd === "list_entities") {
        return Promise.resolve({ entities: [], count: 0 });
      }
      return Promise.resolve({});
    });

    const result = await refreshBoards();

    expect(result.openBoards).toHaveLength(2);
    expect(result.openBoards[0].name).toBe("Board A");
    expect(result.openBoards[1].name).toBe("Board B");
    // Board data should be null since get_board_data failed
    expect(result.boardData).toBeNull();
  });

  it("surfaces a boardError when get_board_data fails, not a silent null", async () => {
    // Defense 3: a malformed board whose `get_board_data` rejects (e.g.
    // "entity not found: board/board") must surface an ERROR STATE for that
    // board so the window can degrade / fall back — never swallow into a
    // silent null that blanks the window forever.
    installLegacy((cmd) => {
      if (cmd === "list_open_boards") {
        return Promise.resolve([
          { path: "/good/.kanban", is_active: false, name: "Good Board" },
          { path: "/bad/.kanban", is_active: true, name: "Bad Board" },
        ]);
      }
      if (cmd === "get_board_data") {
        return Promise.reject(new Error("entity not found: board/board"));
      }
      if (cmd === "list_entities") {
        return Promise.resolve({ entities: [], count: 0 });
      }
      return Promise.resolve({});
    });

    const result = await refreshBoards("/bad/.kanban");

    expect(result.boardData).toBeNull();
    expect(result.boardError).toBeTruthy();
    expect(result.boardError).toContain("board/board");
    // The other open boards remain available for the window to fall back to.
    expect(result.openBoards).toHaveLength(2);
  });

  it("reports no boardError on a healthy refresh", async () => {
    installLegacy((cmd) => {
      if (cmd === "list_open_boards") {
        return Promise.resolve([
          { path: "/a/.kanban", is_active: true, name: "Board A" },
        ]);
      }
      if (cmd === "get_board_data") {
        return Promise.resolve({
          board: { id: "board", entity_type: "board", name: "Board A" },
          columns: [],
          tags: [],
          summary: {
            total_tasks: 0,
            total_actors: 0,
            ready_tasks: 0,
            blocked_tasks: 0,
            done_tasks: 0,
            percent_complete: 0,
          },
        });
      }
      if (cmd === "list_entities") {
        return Promise.resolve({ entities: [], count: 0 });
      }
      return Promise.resolve({});
    });

    const result = await refreshBoards();

    expect(result.boardData).not.toBeNull();
    expect(result.boardError).toBeNull();
  });

  it("returns all data when everything succeeds", async () => {
    installLegacy((cmd) => {
      if (cmd === "list_open_boards") {
        return Promise.resolve([
          { path: "/a/.kanban", is_active: true, name: "Board A" },
        ]);
      }
      if (cmd === "get_board_data") {
        return Promise.resolve({
          board: { id: "board", entity_type: "board", name: "Board A" },
          columns: [],

          tags: [],
          summary: {
            total_tasks: 0,
            total_actors: 0,
            ready_tasks: 0,
            blocked_tasks: 0,
            done_tasks: 0,
            percent_complete: 0,
          },
        });
      }
      if (cmd === "list_entities") {
        return Promise.resolve({ entities: [], count: 0 });
      }
      return Promise.resolve({});
    });

    const result = await refreshBoards();

    expect(result.openBoards).toHaveLength(1);
    expect(result.boardData).not.toBeNull();
    // All entity types from board data should be in entitiesByType
    expect(result.entitiesByType).not.toBeNull();
    expect(result.entitiesByType!.board).toHaveLength(1);
    expect(result.entitiesByType!.board[0].entity_type).toBe("board");
    expect(result.entitiesByType!.column).toHaveLength(0);
    expect(result.entitiesByType!.tag).toHaveLength(0);
    expect(result.entitiesByType!.task).toHaveLength(0);
    expect(result.entitiesByType!.actor).toHaveLength(0);
    expect(result.entitiesByType!.project).toHaveLength(0);
  });

  it("passes boardPath to get_board_data and list_entities when provided", async () => {
    installLegacy((cmd) => {
      if (cmd === "list_open_boards") {
        return Promise.resolve([
          { path: "/a/.kanban", is_active: true, name: "Board A" },
          { path: "/b/.kanban", is_active: false, name: "Board B" },
        ]);
      }
      if (cmd === "get_board_data") {
        return Promise.resolve({
          board: { id: "board", entity_type: "board", name: "Board B" },
          columns: [],

          tags: [],
          summary: {
            total_tasks: 0,
            total_actors: 0,
            ready_tasks: 0,
            blocked_tasks: 0,
            done_tasks: 0,
            percent_complete: 0,
          },
        });
      }
      if (cmd === "list_entities") {
        return Promise.resolve({ entities: [], count: 0 });
      }
      return Promise.resolve({});
    });

    await refreshBoards("/b/.kanban");

    // get_board_data should receive boardPath
    const boardDataCall = mockInvoke.mock.calls.find(
      (c) => c[0] === "get_board_data",
    );
    expect(boardDataCall).toBeDefined();
    expect(boardDataCall![1]).toEqual({ boardPath: "/b/.kanban" });

    // list_entities calls should receive boardPath
    const entityCalls = mockInvoke.mock.calls.filter(
      (c) => c[0] === "list_entities",
    );
    for (const call of entityCalls) {
      expect(call[1]).toMatchObject({ boardPath: "/b/.kanban" });
    }
  });

  it("does not pass boardPath when omitted", async () => {
    installLegacy((cmd) => {
      if (cmd === "list_open_boards") {
        return Promise.resolve([
          { path: "/a/.kanban", is_active: true, name: "Board A" },
        ]);
      }
      if (cmd === "get_board_data") {
        return Promise.resolve({
          board: { id: "board", entity_type: "board", name: "Board A" },
          columns: [],

          tags: [],
          summary: {
            total_tasks: 0,
            total_actors: 0,
            ready_tasks: 0,
            blocked_tasks: 0,
            done_tasks: 0,
            percent_complete: 0,
          },
        });
      }
      if (cmd === "list_entities") {
        return Promise.resolve({ entities: [], count: 0 });
      }
      return Promise.resolve({});
    });

    await refreshBoards();

    // get_board_data should NOT have boardPath
    const boardDataCall = mockInvoke.mock.calls.find(
      (c) => c[0] === "get_board_data",
    );
    expect(boardDataCall).toBeDefined();
    expect(boardDataCall![1]).toEqual({});
  });

  it("keeps board data when only list_entities fails", async () => {
    // Defense 3 — decoupling: a `list_entities` failure (a degraded entity
    // list) must NOT take down `get_board_data`. The board still renders from
    // its board data; only the entity store is left empty. This is the split
    // that breaks the old single Promise.all where one entity-list rejection
    // nulled the whole board.
    installLegacy((cmd) => {
      if (cmd === "list_open_boards") {
        return Promise.resolve([
          { path: "/a/.kanban", is_active: false, name: "Board A" },
          { path: "/b/.kanban", is_active: true, name: "Board B" },
        ]);
      }
      if (cmd === "get_board_data") {
        return Promise.resolve({
          board: { id: "board", entity_type: "board", name: "Board B" },
          columns: [],

          tags: [],
          summary: {
            total_tasks: 0,
            total_actors: 0,
            ready_tasks: 0,
            blocked_tasks: 0,
            done_tasks: 0,
            percent_complete: 0,
          },
        });
      }
      if (cmd === "list_entities") {
        return Promise.reject(new Error("entity dir not found"));
      }
      return Promise.resolve({});
    });

    const result = await refreshBoards();

    // Open boards should always be populated
    expect(result.openBoards).toHaveLength(2);
    // Board data SURVIVES a list_entities failure — the board still renders.
    expect(result.boardData).not.toBeNull();
    // get_board_data itself succeeded, so there is no board-level error.
    expect(result.boardError).toBeNull();
  });
});
