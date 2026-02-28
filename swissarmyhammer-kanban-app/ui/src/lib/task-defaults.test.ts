import { describe, it, expect } from "vitest";
import { defaultTaskTitle } from "./task-defaults";

describe("defaultTaskTitle", () => {
  it("generates a title with the column name", () => {
    const title = defaultTaskTitle("To Do");
    expect(title).toBe("New task");
  });

  it("always returns the same default", () => {
    expect(defaultTaskTitle("Doing")).toBe("New task");
    expect(defaultTaskTitle("Done")).toBe("New task");
  });
});
