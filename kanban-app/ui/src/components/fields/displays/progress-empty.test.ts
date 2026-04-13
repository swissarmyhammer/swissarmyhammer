import { describe, it, expect } from "vitest";
import { isProgressEmpty } from "./progress-empty";

describe("isProgressEmpty", () => {
  it("treats total: 0 as empty", () => {
    expect(isProgressEmpty({ total: 0, completed: 0, percent: 0 })).toBe(true);
  });

  it("treats board shape with total: 0 as empty", () => {
    expect(isProgressEmpty({ total: 0, done: 0, percent: 0 })).toBe(true);
  });

  it("treats positive total as non-empty (task shape)", () => {
    expect(isProgressEmpty({ total: 4, completed: 2, percent: 50 })).toBe(
      false,
    );
  });

  it("treats positive total as non-empty (board shape)", () => {
    expect(isProgressEmpty({ total: 5, done: 3, percent: 60 })).toBe(false);
  });

  it("treats null as empty", () => {
    expect(isProgressEmpty(null)).toBe(true);
  });

  it("treats undefined as empty", () => {
    expect(isProgressEmpty(undefined)).toBe(true);
  });

  it("treats a non-object value as empty", () => {
    expect(isProgressEmpty(42)).toBe(true);
    expect(isProgressEmpty("50%")).toBe(true);
    expect(isProgressEmpty(true)).toBe(true);
  });

  it("treats an object without total as empty", () => {
    expect(isProgressEmpty({})).toBe(true);
    expect(isProgressEmpty({ percent: 50 })).toBe(true);
  });

  it("treats an object with non-numeric total as empty", () => {
    expect(isProgressEmpty({ total: "4" })).toBe(true);
    expect(isProgressEmpty({ total: null })).toBe(true);
  });
});
