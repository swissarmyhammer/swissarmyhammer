import { describe, it, expect, vi, beforeEach } from "vitest";

const mockInvoke = vi.fn((..._args: unknown[]) => Promise.resolve({}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

import { refreshBoards } from "./refresh";

describe("refreshBoards", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("returns open boards even when get_board_data fails", async () => {
    // Simulate: list_open_boards succeeds with 2 boards,
    // but get_board_data fails (new board not fully ready).
    mockInvoke.mockImplementation((...args: unknown[]) => {
      const cmd = args[0] as string;
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

  it("returns all data when everything succeeds", async () => {
    mockInvoke.mockImplementation((...args: unknown[]) => {
      const cmd = args[0] as string;
      if (cmd === "list_open_boards") {
        return Promise.resolve([
          { path: "/a/.kanban", is_active: true, name: "Board A" },
        ]);
      }
      if (cmd === "get_board_data") {
        return Promise.resolve({
          board: { id: "board", entity_type: "board", name: "Board A" },
          columns: [],
          swimlanes: [],
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
    expect(result.entitiesByType!.swimlane).toHaveLength(0);
    expect(result.entitiesByType!.tag).toHaveLength(0);
    expect(result.entitiesByType!.task).toHaveLength(0);
    expect(result.entitiesByType!.actor).toHaveLength(0);
  });

  it("passes boardPath to get_board_data and list_entities when provided", async () => {
    mockInvoke.mockImplementation((...args: unknown[]) => {
      const cmd = args[0] as string;
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
          swimlanes: [],
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
    mockInvoke.mockImplementation((...args: unknown[]) => {
      const cmd = args[0] as string;
      if (cmd === "list_open_boards") {
        return Promise.resolve([
          { path: "/a/.kanban", is_active: true, name: "Board A" },
        ]);
      }
      if (cmd === "get_board_data") {
        return Promise.resolve({
          board: { id: "board", entity_type: "board", name: "Board A" },
          columns: [],
          swimlanes: [],
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

  it("returns open boards even when list_entities fails", async () => {
    mockInvoke.mockImplementation((...args: unknown[]) => {
      const cmd = args[0] as string;
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
          swimlanes: [],
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
    // Board data should be null because list_entities failed
    expect(result.boardData).toBeNull();
  });
});
