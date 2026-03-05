import { describe, it, expect } from "vitest";
import { moniker, parseMoniker } from "./moniker";

describe("moniker", () => {
  it("builds type:id string", () => {
    expect(moniker("task", "abc")).toBe("task:abc");
  });

  it("handles empty-ish inputs", () => {
    expect(moniker("", "abc")).toBe(":abc");
    expect(moniker("task", "")).toBe("task:");
  });
});

describe("parseMoniker", () => {
  it("splits correctly", () => {
    expect(parseMoniker("tag:xyz")).toEqual({ type: "tag", id: "xyz" });
  });

  it("handles id with colons", () => {
    expect(parseMoniker("task:foo:bar")).toEqual({ type: "task", id: "foo:bar" });
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
