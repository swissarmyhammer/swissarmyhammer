import { describe, it, expect } from "vitest";
import { reorderColumns } from "./column-reorder";

describe("reorderColumns", () => {
  it("moves a column from index 0 to index 2", () => {
    const ids = ["todo", "doing", "done"];
    const result = reorderColumns(ids, 0, 2);
    // arrayMove: ["todo","doing","done"] -> ["doing","done","todo"]
    expect(result).toEqual([
      { id: "doing", order: 0 },
      { id: "done", order: 1 },
      { id: "todo", order: 2 },
    ]);
  });

  it("moves a column from index 2 to index 0", () => {
    const ids = ["todo", "doing", "done"];
    const result = reorderColumns(ids, 2, 0);
    // arrayMove: ["todo","doing","done"] -> ["done","todo","doing"]
    expect(result).toEqual([
      { id: "done", order: 0 },
      { id: "todo", order: 1 },
      { id: "doing", order: 2 },
    ]);
  });

  it("returns empty array when from equals to", () => {
    const ids = ["todo", "doing", "done"];
    const result = reorderColumns(ids, 1, 1);
    expect(result).toEqual([]);
  });

  it("handles two columns", () => {
    const ids = ["todo", "done"];
    const result = reorderColumns(ids, 0, 1);
    // arrayMove: ["todo","done"] -> ["done","todo"]
    expect(result).toEqual([
      { id: "done", order: 0 },
      { id: "todo", order: 1 },
    ]);
  });
});
