// @vitest-environment node
import { describe, it, expect } from "vitest";
import { patchBoardData } from "./board-data-patch";
import type { BoardData, Entity } from "@/types/kanban";

/** Helper to build a minimal Entity. */
function makeEntity(
  type: string,
  id: string,
  fields: Record<string, unknown> = {},
): Entity {
  return { entity_type: type, id, moniker: `${type}:${id}`, fields };
}

/** Helper to build a minimal BoardData. */
function makeBoardData(overrides: Partial<BoardData> = {}): BoardData {
  return {
    board: makeEntity("board", "board", { name: "My Board" }),
    columns: [
      makeEntity("column", "col-1", { name: "To Do", order: 0 }),
      makeEntity("column", "col-2", { name: "Done", order: 1 }),
    ],
    tags: [makeEntity("tag", "tag-1", { name: "bug" })],
    virtualTagMeta: [],
    summary: {
      total_tasks: 0,
      total_actors: 0,
      ready_tasks: 0,
      blocked_tasks: 0,
      done_tasks: 0,
      percent_complete: 0,
    },
    ...overrides,
  };
}

describe("patchBoardData", () => {
  it("patches board entity when entity_type is 'board'", () => {
    const board = makeBoardData();
    const updated = makeEntity("board", "board", { name: "Renamed Board" });

    const result = patchBoardData(board, "board", "board", updated);

    expect(result).not.toBe(null);
    expect(result!.board.fields.name).toBe("Renamed Board");
  });

  it("patches a column in BoardData.columns by id", () => {
    const board = makeBoardData();
    const updated = makeEntity("column", "col-1", {
      name: "Backlog",
      order: 0,
    });

    const result = patchBoardData(board, "column", "col-1", updated);

    expect(result).not.toBe(null);
    const col = result!.columns.find((c) => c.id === "col-1");
    expect(col).toBeDefined();
    expect(col!.fields.name).toBe("Backlog");
  });

  it("returns null for non-structural entity types (e.g. task)", () => {
    const board = makeBoardData();
    const task = makeEntity("task", "task-1", { title: "Fix bug" });

    const result = patchBoardData(board, "task", "task-1", task);

    expect(result).toBe(null);
  });

  it("returns null when board is null", () => {
    const task = makeEntity("column", "col-1", { name: "Backlog" });

    const result = patchBoardData(null, "column", "col-1", task);

    expect(result).toBe(null);
  });

  it("preserves other columns when patching one", () => {
    const board = makeBoardData();
    const updated = makeEntity("column", "col-1", {
      name: "Backlog",
      order: 0,
    });

    const result = patchBoardData(board, "column", "col-1", updated);

    expect(result!.columns).toHaveLength(2);
    const col2 = result!.columns.find((c) => c.id === "col-2");
    expect(col2!.fields.name).toBe("Done");
  });

  it("preserves board identity when patching a column", () => {
    const board = makeBoardData();
    const updated = makeEntity("column", "col-1", {
      name: "Backlog",
      order: 0,
    });

    const result = patchBoardData(board, "column", "col-1", updated);

    expect(result!.board).toBe(board.board);
    expect(result!.tags).toBe(board.tags);
    expect(result!.summary).toBe(board.summary);
  });
});
