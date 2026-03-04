import { describe, it, expect } from "vitest";
import { fuzzyMatch } from "./fuzzy-filter";

describe("fuzzyMatch", () => {
  it("matches when all query characters appear in order", () => {
    const result = fuzzyMatch("abc", "aXbXc");
    expect(result.match).toBe(true);
  });

  it("does not match when characters are missing", () => {
    const result = fuzzyMatch("abc", "aXbX");
    expect(result.match).toBe(false);
  });

  it("does not match when characters appear out of order", () => {
    const result = fuzzyMatch("abc", "cba");
    expect(result.match).toBe(false);
  });

  it("is case-insensitive", () => {
    const result = fuzzyMatch("ABC", "aXbXc");
    expect(result.match).toBe(true);
  });

  it("matches empty query against any target", () => {
    const result = fuzzyMatch("", "anything");
    expect(result.match).toBe(true);
    expect(result.score).toBe(0);
  });

  it("does not match non-empty query against empty target", () => {
    const result = fuzzyMatch("a", "");
    expect(result.match).toBe(false);
  });

  it("matches empty query against empty target", () => {
    const result = fuzzyMatch("", "");
    expect(result.match).toBe(true);
  });

  it("scores exact match better than spread-out match", () => {
    const exact = fuzzyMatch("abc", "abc");
    const spread = fuzzyMatch("abc", "aXXbXXc");
    expect(exact.score).toBeLessThan(spread.score);
  });

  it("scores prefix match better than suffix match", () => {
    const prefix = fuzzyMatch("ab", "abcdef");
    const suffix = fuzzyMatch("ab", "XXXXab");
    expect(prefix.score).toBeLessThan(suffix.score);
  });

  it("scores consecutive characters better than scattered ones", () => {
    const consecutive = fuzzyMatch("abc", "Xabc");
    const scattered = fuzzyMatch("abc", "XaXbXc");
    expect(consecutive.score).toBeLessThan(scattered.score);
  });

  it("handles repeated characters correctly", () => {
    const result = fuzzyMatch("aa", "abca");
    expect(result.match).toBe(true);
  });

  it("does not match when not enough repeated characters exist", () => {
    const result = fuzzyMatch("aaa", "aXa");
    expect(result.match).toBe(false);
  });
});
