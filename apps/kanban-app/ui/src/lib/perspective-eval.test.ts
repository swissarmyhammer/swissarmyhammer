import { describe, it, expect } from "vitest";
import type { Entity } from "@/types/kanban";
import type { PerspectiveSortEntry } from "@/types/kanban";
import { evaluateSort } from "./perspective-eval";

/** Helper to build a minimal Entity with given fields. */
function entity(id: string, fields: Record<string, unknown>): Entity {
  return { entity_type: "task", id, moniker: `task:${id}`, fields };
}

describe("evaluateSort", () => {
  it("returns entities unchanged when sort is empty", () => {
    const entities = [entity("2", {}), entity("1", {})];
    const result = evaluateSort([], entities);
    expect(result.map((e) => e.id)).toEqual(["2", "1"]);
  });

  it("sorts by a single string field ascending", () => {
    const entities = [
      entity("1", { title: "Banana" }),
      entity("2", { title: "Apple" }),
      entity("3", { title: "Cherry" }),
    ];
    const sort: PerspectiveSortEntry[] = [{ field: "title", direction: "asc" }];
    const result = evaluateSort(sort, entities);
    expect(result.map((e) => e.id)).toEqual(["2", "1", "3"]);
  });

  it("sorts by a single string field descending", () => {
    const entities = [
      entity("1", { title: "Banana" }),
      entity("2", { title: "Apple" }),
      entity("3", { title: "Cherry" }),
    ];
    const sort: PerspectiveSortEntry[] = [
      { field: "title", direction: "desc" },
    ];
    const result = evaluateSort(sort, entities);
    expect(result.map((e) => e.id)).toEqual(["3", "1", "2"]);
  });

  it("sorts by numeric field", () => {
    const entities = [
      entity("1", { priority: 3 }),
      entity("2", { priority: 1 }),
      entity("3", { priority: 2 }),
    ];
    const sort: PerspectiveSortEntry[] = [
      { field: "priority", direction: "asc" },
    ];
    const result = evaluateSort(sort, entities);
    expect(result.map((e) => e.id)).toEqual(["2", "3", "1"]);
  });

  it("handles multi-level sort (ties broken by second entry)", () => {
    const entities = [
      entity("1", { Status: "open", priority: 3 }),
      entity("2", { Status: "closed", priority: 1 }),
      entity("3", { Status: "open", priority: 1 }),
    ];
    const sort: PerspectiveSortEntry[] = [
      { field: "Status", direction: "asc" },
      { field: "priority", direction: "asc" },
    ];
    const result = evaluateSort(sort, entities);
    expect(result.map((e) => e.id)).toEqual(["2", "3", "1"]);
  });

  it("does not mutate the original array", () => {
    const entities = [entity("2", { title: "B" }), entity("1", { title: "A" })];
    const original = [...entities];
    evaluateSort([{ field: "title", direction: "asc" }], entities);
    expect(entities.map((e) => e.id)).toEqual(original.map((e) => e.id));
  });

  it("handles missing field values gracefully", () => {
    const entities = [
      entity("1", { title: "Banana" }),
      entity("2", {}),
      entity("3", { title: "Apple" }),
    ];
    const sort: PerspectiveSortEntry[] = [{ field: "title", direction: "asc" }];
    const result = evaluateSort(sort, entities);
    expect(result[0].id).toBe("2");
  });
});
