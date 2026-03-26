import { describe, it, expect } from "vitest";
import { moniker, fieldMoniker, parseMoniker } from "./moniker";

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
  it("builds type:id.field string", () => {
    expect(fieldMoniker("task", "abc", "title")).toBe("task:abc.title");
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

  it("parses field-level moniker", () => {
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
