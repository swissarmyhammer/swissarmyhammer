import { describe, it, expect } from "vitest";
import type { Entity, FieldDef } from "@/types/kanban";
import { computeGroups } from "./group-utils";

/** Helper to build a minimal Entity for testing. */
function makeTask(id: string, fields: Record<string, unknown>): Entity {
  return { entity_type: "task", id, moniker: `task:${id}`, fields };
}

/** Helper to build a minimal FieldDef. */
function makeFieldDef(
  id: string,
  kind: string,
  overrides: Partial<FieldDef> = {},
): FieldDef {
  return { id, name: id, type: { kind }, ...overrides };
}

describe("computeGroups", () => {
  it("returns empty array when given no tasks", () => {
    const result = computeGroups([], "project", [
      makeFieldDef("project", "string"),
    ]);
    expect(result).toEqual([]);
  });

  it("groups tasks by a single-value string field", () => {
    const tasks = [
      makeTask("1", { project: "alpha" }),
      makeTask("2", { project: "beta" }),
      makeTask("3", { project: "alpha" }),
    ];
    const fieldDefs = [makeFieldDef("project", "string")];

    const groups = computeGroups(tasks, "project", fieldDefs);

    expect(groups).toHaveLength(2);
    expect(groups[0]).toEqual({
      value: "alpha",
      label: "alpha",
      tasks: [tasks[0], tasks[2]],
    });
    expect(groups[1]).toEqual({
      value: "beta",
      label: "beta",
      tasks: [tasks[1]],
    });
  });

  it("sorts groups alphabetically by value", () => {
    const tasks = [
      makeTask("1", { color: "red" }),
      makeTask("2", { color: "blue" }),
      makeTask("3", { color: "green" }),
    ];
    const fieldDefs = [makeFieldDef("color", "string")];

    const groups = computeGroups(tasks, "color", fieldDefs);

    expect(groups.map((g) => g.value)).toEqual(["blue", "green", "red"]);
  });

  it("preserves task order within each group", () => {
    const tasks = [
      makeTask("1", { project: "alpha" }),
      makeTask("2", { project: "alpha" }),
      makeTask("3", { project: "alpha" }),
    ];
    const fieldDefs = [makeFieldDef("project", "string")];

    const groups = computeGroups(tasks, "project", fieldDefs);

    expect(groups[0].tasks.map((t) => t.id)).toEqual(["1", "2", "3"]);
  });

  it("places ungrouped tasks (null/undefined/empty) in '(ungrouped)' bucket at the end", () => {
    const tasks = [
      makeTask("1", { project: "alpha" }),
      makeTask("2", { project: null }),
      makeTask("3", {}),
      makeTask("4", { project: "" }),
      makeTask("5", { project: "beta" }),
    ];
    const fieldDefs = [makeFieldDef("project", "string")];

    const groups = computeGroups(tasks, "project", fieldDefs);

    expect(groups).toHaveLength(3);
    // alpha and beta come first, alphabetically
    expect(groups[0].value).toBe("alpha");
    expect(groups[1].value).toBe("beta");
    // ungrouped last
    expect(groups[2]).toEqual({
      value: "",
      label: "(ungrouped)",
      tasks: [tasks[1], tasks[2], tasks[3]],
    });
  });

  it("groups tasks by a multi-value array field, placing tasks in multiple groups", () => {
    const tasks = [
      makeTask("1", { tags: ["bug", "feature"] }),
      makeTask("2", { tags: ["feature"] }),
      makeTask("3", { tags: ["bug"] }),
    ];
    const fieldDefs = [makeFieldDef("tags", "string_list")];

    const groups = computeGroups(tasks, "tags", fieldDefs);

    expect(groups).toHaveLength(2);
    expect(groups[0]).toEqual({
      value: "bug",
      label: "bug",
      tasks: [tasks[0], tasks[2]],
    });
    expect(groups[1]).toEqual({
      value: "feature",
      label: "feature",
      tasks: [tasks[0], tasks[1]],
    });
  });

  it("puts tasks with empty arrays into ungrouped for multi-value fields", () => {
    const tasks = [
      makeTask("1", { tags: ["bug"] }),
      makeTask("2", { tags: [] }),
      makeTask("3", {}),
    ];
    const fieldDefs = [makeFieldDef("tags", "string_list")];

    const groups = computeGroups(tasks, "tags", fieldDefs);

    expect(groups).toHaveLength(2);
    expect(groups[0].value).toBe("bug");
    expect(groups[1]).toEqual({
      value: "",
      label: "(ungrouped)",
      tasks: [tasks[1], tasks[2]],
    });
  });

  it("detects array values at runtime even without field def kind hint", () => {
    const tasks = [
      makeTask("1", { labels: ["a", "b"] }),
      makeTask("2", { labels: ["b"] }),
    ];
    // kind is 'string' but value is actually an array — runtime detection should work
    const fieldDefs = [makeFieldDef("labels", "string")];

    const groups = computeGroups(tasks, "labels", fieldDefs);

    expect(groups).toHaveLength(2);
    expect(groups[0].value).toBe("a");
    expect(groups[1].value).toBe("b");
    // task 1 appears in both groups
    expect(groups[0].tasks.map((t) => t.id)).toEqual(["1"]);
    expect(groups[1].tasks.map((t) => t.id)).toEqual(["1", "2"]);
  });
});
