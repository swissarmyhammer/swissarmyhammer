import { describe, it, expect } from "vitest";
import {
  moniker,
  fieldMoniker,
  gridCellMoniker,
  parseMoniker,
  parseFieldMoniker,
  parseGridCellMoniker,
} from "./moniker";

describe("moniker", () => {
  it("builds type:id string", () => {
    expect(moniker("task", "abc")).toBe("task:abc");
  });

  it("handles empty-ish inputs", () => {
    expect(moniker("", "abc")).toBe(":abc");
    expect(moniker("task", "")).toBe("task:");
  });
});

describe("fieldMoniker", () => {
  it("builds field:type:id.field string", () => {
    expect(fieldMoniker("task", "abc", "title")).toBe("field:task:abc.title");
  });
});

describe("parseMoniker", () => {
  it("splits correctly", () => {
    expect(parseMoniker("tag:xyz")).toEqual({ type: "tag", id: "xyz" });
  });

  it("handles id with colons", () => {
    expect(parseMoniker("task:foo:bar")).toEqual({
      type: "task",
      id: "foo:bar",
    });
  });

  it("parses field-level moniker with field: prefix", () => {
    expect(parseMoniker("field:task:abc.title")).toEqual({
      type: "field",
      id: "task:abc",
      field: "title",
    });
  });

  it("parses old-style field-level moniker", () => {
    expect(parseMoniker("task:abc.title")).toEqual({
      type: "task",
      id: "abc",
      field: "title",
    });
  });

  it("field-level with colons in id", () => {
    expect(parseMoniker("task:foo:bar.title")).toEqual({
      type: "task",
      id: "foo:bar",
      field: "title",
    });
  });

  it("throws on no colon", () => {
    expect(() => parseMoniker("badstring")).toThrow("no colon");
  });

  it("throws on empty type", () => {
    expect(() => parseMoniker(":abc")).toThrow("empty type");
  });

  it("throws on empty id", () => {
    expect(() => parseMoniker("task:")).toThrow("empty id");
  });
});

describe("gridCellMoniker", () => {
  it("builds grid_cell:row:colKey string", () => {
    expect(gridCellMoniker(0, "title")).toBe("grid_cell:0:title");
    expect(gridCellMoniker(7, "status")).toBe("grid_cell:7:status");
  });

  it("preserves underscores in colKey", () => {
    expect(gridCellMoniker(2, "due_date")).toBe("grid_cell:2:due_date");
  });
});

describe("parseGridCellMoniker", () => {
  it("parses a valid grid_cell moniker", () => {
    expect(parseGridCellMoniker("grid_cell:1:title")).toEqual({
      row: 1,
      colKey: "title",
    });
    expect(parseGridCellMoniker("grid_cell:0:status")).toEqual({
      row: 0,
      colKey: "status",
    });
  });

  it("preserves colKey with underscores", () => {
    expect(parseGridCellMoniker("grid_cell:5:due_date")).toEqual({
      row: 5,
      colKey: "due_date",
    });
  });

  it("returns null for non-grid_cell monikers", () => {
    expect(parseGridCellMoniker("ui:navbar")).toBeNull();
    expect(parseGridCellMoniker("task:abc")).toBeNull();
    expect(parseGridCellMoniker("field:task:abc.title")).toBeNull();
  });

  it("returns null when row or colKey is missing", () => {
    expect(parseGridCellMoniker("grid_cell:")).toBeNull();
    expect(parseGridCellMoniker("grid_cell:1:")).toBeNull();
    expect(parseGridCellMoniker("grid_cell::title")).toBeNull();
  });

  it("returns null when row is not a non-negative integer", () => {
    expect(parseGridCellMoniker("grid_cell:abc:title")).toBeNull();
    expect(parseGridCellMoniker("grid_cell:-1:title")).toBeNull();
    expect(parseGridCellMoniker("grid_cell:1.5:title")).toBeNull();
  });
});

describe("parseFieldMoniker", () => {
  it("extracts entityType, entityId, and field", () => {
    expect(parseFieldMoniker("field:task:abc.title")).toEqual({
      entityType: "task",
      entityId: "abc",
      field: "title",
    });
  });

  it("handles description field", () => {
    expect(parseFieldMoniker("field:task:01ABC.description")).toEqual({
      entityType: "task",
      entityId: "01ABC",
      field: "description",
    });
  });

  it("throws on non-field moniker", () => {
    expect(() => parseFieldMoniker("task:abc")).toThrow("not a field moniker");
  });

  it("throws on missing field", () => {
    expect(() => parseFieldMoniker("field:task:abc")).toThrow("no field");
  });
});
