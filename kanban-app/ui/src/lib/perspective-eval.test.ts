import { describe, it, expect, vi } from "vitest";
import type { Entity } from "@/types/kanban";
import type { PerspectiveSortEntry } from "@/types/kanban";
import {
  evaluateFilter,
  evaluateSort,
  clearFilterCache,
} from "./perspective-eval";

/** Helper to build a minimal Entity with given fields. */
function entity(id: string, fields: Record<string, unknown>): Entity {
  return { entity_type: "task", id, fields };
}

describe("evaluateFilter", () => {
  afterEach(() => clearFilterCache());

  it("returns all entities when filter is undefined", () => {
    const entities = [entity("1", { Status: "open" })];
    expect(evaluateFilter(undefined, entities)).toEqual(entities);
  });

  it("returns all entities when filter is empty string", () => {
    const entities = [entity("1", { Status: "open" })];
    expect(evaluateFilter("", entities)).toEqual(entities);
  });

  it("filters entities using a JS expression", () => {
    const entities = [
      entity("1", { Status: "open" }),
      entity("2", { Status: "closed" }),
      entity("3", { Status: "open" }),
    ];
    const result = evaluateFilter('Status === "open"', entities);
    expect(result).toHaveLength(2);
    expect(result.map((e) => e.id)).toEqual(["1", "3"]);
  });

  it("supports numeric comparisons", () => {
    const entities = [
      entity("1", { priority: 1 }),
      entity("2", { priority: 5 }),
      entity("3", { priority: 3 }),
    ];
    const result = evaluateFilter("priority > 2", entities);
    expect(result).toHaveLength(2);
    expect(result.map((e) => e.id)).toEqual(["2", "3"]);
  });

  it("supports compound expressions", () => {
    const entities = [
      entity("1", { Status: "open", priority: 1 }),
      entity("2", { Status: "open", priority: 5 }),
      entity("3", { Status: "closed", priority: 5 }),
    ];
    const result = evaluateFilter(
      'Status === "open" && priority > 2',
      entities,
    );
    expect(result).toHaveLength(1);
    expect(result[0].id).toBe("2");
  });

  it("returns all entities on malformed expression (graceful failure)", () => {
    const warnSpy = vi.spyOn(console, "warn").mockImplementation(() => {});
    const entities = [entity("1", { Status: "open" })];
    const result = evaluateFilter("this is not valid js !!!!", entities);
    expect(result).toEqual(entities);
    expect(warnSpy).toHaveBeenCalled();
    warnSpy.mockRestore();
  });

  it("returns all entities when expression throws at runtime", () => {
    const warnSpy = vi.spyOn(console, "warn").mockImplementation(() => {});
    const entities = [entity("1", { Status: "open" })];
    const result = evaluateFilter("foo.bar.baz", entities);
    expect(result).toEqual(entities);
    expect(warnSpy).toHaveBeenCalled();
    warnSpy.mockRestore();
  });

  it("caches compiled functions (same expression reused)", () => {
    const entities = [
      entity("1", { Status: "open" }),
      entity("2", { Status: "closed" }),
    ];
    const result1 = evaluateFilter('Status === "open"', entities);
    const result2 = evaluateFilter('Status === "open"', entities);
    expect(result1).toEqual(result2);
  });

  it("handles missing fields as undefined", () => {
    const entities = [entity("1", { Status: "open" }), entity("2", {})];
    const result = evaluateFilter('Status === "open"', entities);
    expect(result).toHaveLength(1);
    expect(result[0].id).toBe("1");
  });
});

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
    // closed < open, then by priority within same status
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
    // Missing values sort before strings (empty string < "Apple")
    const result = evaluateSort(sort, entities);
    expect(result[0].id).toBe("2"); // missing field sorts first
  });
});
