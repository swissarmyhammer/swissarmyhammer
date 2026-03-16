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
    mockInvoke.mockImplementation((cmd: string) => {
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
    mockInvoke.mockImplementation((cmd: string) => {
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
          summary: { total_tasks: 0, total_actors: 0, ready_tasks: 0, blocked_tasks: 0, done_tasks: 0, percent_complete: 0 },
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
  });

  it("returns open boards even when list_entities fails", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
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
          summary: { total_tasks: 0, total_actors: 0, ready_tasks: 0, blocked_tasks: 0, done_tasks: 0, percent_complete: 0 },
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
