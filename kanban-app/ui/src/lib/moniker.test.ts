import { describe, it, expect } from "vitest";
import {
  columnHeaderMoniker,
  moniker,
  fieldMoniker,
  parseMoniker,
  parseFieldMoniker,
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

describe("columnHeaderMoniker", () => {
  it("builds column-header:<fieldName> string", () => {
    expect(columnHeaderMoniker("title")).toBe("column-header:title");
  });

  it("parses round-trip through parseMoniker", () => {
    // Uses the generic `moniker()` helper under the hood, so its output
    // must round-trip cleanly — no field portion since the whole payload
    // is the column/field name.
    expect(parseMoniker(columnHeaderMoniker("status"))).toEqual({
      type: "column-header",
      id: "status",
    });
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
