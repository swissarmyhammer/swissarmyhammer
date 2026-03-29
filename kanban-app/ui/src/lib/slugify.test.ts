import { describe, it, expect } from "vitest";
import { slugify } from "./slugify";

describe("slugify", () => {
  it("lowercases and hyphenates spaces", () => {
    expect(slugify("Fix Login Bug")).toBe("fix-login-bug");
  });

  it("is idempotent on existing slugs", () => {
    expect(slugify("fix-login-bug")).toBe("fix-login-bug");
  });

  it("collapses multiple non-alnum chars", () => {
    expect(slugify("hello   world")).toBe("hello-world");
    expect(slugify("a--b")).toBe("a-b");
  });

  it("strips leading and trailing hyphens", () => {
    expect(slugify("--hello--")).toBe("hello");
    expect(slugify("  spaced  ")).toBe("spaced");
  });

  it("handles special characters", () => {
    expect(slugify("Task: Do (something) #1")).toBe("task-do-something-1");
  });

  it("handles empty string", () => {
    expect(slugify("")).toBe("");
  });

  it("handles single word", () => {
    expect(slugify("hello")).toBe("hello");
  });
});
