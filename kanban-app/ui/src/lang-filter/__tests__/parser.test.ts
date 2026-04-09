/**
 * Parse tree assertions for the filter DSL grammar.
 *
 * Verifies that representative expressions produce the expected node structure
 * and that error recovery handles incomplete input gracefully.
 */

import { describe, it, expect } from "vitest";
import { parser } from "../parser";

/** Parse an expression and return the tree's string representation. */
function parseTree(input: string): string {
  return parser.parse(input).toString();
}

/** Return true if the parse tree contains an error node. */
function hasError(input: string): boolean {
  const tree = parser.parse(input);
  let found = false;
  tree.iterate({
    enter(node) {
      if (node.type.isError) found = true;
    },
  });
  return found;
}

describe("filter grammar parser", () => {
  // ── Atoms ────────────────────────────────────────────────────────

  it("parses a tag atom", () => {
    const tree = parseTree("#bug");
    expect(tree).toContain("Tag");
  });

  it("parses a mention atom", () => {
    const tree = parseTree("@alice");
    expect(tree).toContain("Mention");
  });

  it("parses a ref atom", () => {
    const tree = parseTree("^01ABC");
    expect(tree).toContain("Ref");
  });

  it("parses tags with hyphens and dots", () => {
    const tree = parseTree("#bug-fix");
    expect(tree).toContain("Tag");
    const tree2 = parseTree("#v2.0");
    expect(tree2).toContain("Tag");
  });

  // ── Operators ────────────────────────────────────────────────────

  it("parses && operator", () => {
    const tree = parseTree("#bug && @will");
    expect(tree).toContain("And");
    expect(tree).toContain("AmpAmp");
  });

  it("parses || operator", () => {
    const tree = parseTree("#bug || #feature");
    expect(tree).toContain("Or");
    expect(tree).toContain("PipePipe");
  });

  it("parses ! operator", () => {
    const tree = parseTree("!#done");
    expect(tree).toContain("Not");
    expect(tree).toContain("Bang");
  });

  // ── Keyword operators ────────────────────────────────────────────

  it("parses 'and' keyword", () => {
    const tree = parseTree("#a and #b");
    expect(tree).toContain("And");
    expect(tree).toContain("and");
  });

  it("parses 'AND' keyword", () => {
    const tree = parseTree("#a AND #b");
    expect(tree).toContain("And");
    expect(tree).toContain("and");
  });

  it("parses 'or' keyword", () => {
    const tree = parseTree("#a or #b");
    expect(tree).toContain("Or");
    expect(tree).toContain("or");
  });

  it("parses 'OR' keyword", () => {
    const tree = parseTree("#a OR #b");
    expect(tree).toContain("Or");
    expect(tree).toContain("or");
  });

  it("parses 'not' keyword", () => {
    const tree = parseTree("not #done");
    expect(tree).toContain("Not");
    expect(tree).toContain("not");
  });

  it("parses 'NOT' keyword", () => {
    const tree = parseTree("NOT #done");
    expect(tree).toContain("Not");
    expect(tree).toContain("not");
  });

  // ── Grouping ─────────────────────────────────────────────────────

  it("parses grouped expressions", () => {
    const tree = parseTree("(#a || #b) && #c");
    expect(tree).toContain("Group");
    expect(tree).toContain("And");
    expect(tree).toContain("Or");
  });

  // ── Complex expression ───────────────────────────────────────────

  it("parses a complex expression with all features", () => {
    const tree = parseTree("#bug && @will || !#done");
    expect(tree).toContain("Tag");
    expect(tree).toContain("Mention");
    expect(tree).toContain("And");
    expect(tree).toContain("Or");
    expect(tree).toContain("Not");
  });

  // ── Error recovery ───────────────────────────────────────────────

  it("recovers from incomplete expression", () => {
    const tree = parseTree("#bug &&");
    // The tree should contain Tag even though the expression is incomplete
    expect(tree).toContain("Tag");
    expect(hasError("#bug &&")).toBe(true);
  });

  it("recovers from unmatched paren", () => {
    expect(hasError("(#bug")).toBe(true);
    // Tag should still be recognized
    expect(parseTree("(#bug")).toContain("Tag");
  });

  it("valid expressions have no error nodes", () => {
    expect(hasError("#bug")).toBe(false);
    expect(hasError("#bug && @will")).toBe(false);
    expect(hasError("#a || #b")).toBe(false);
    expect(hasError("!#done")).toBe(false);
    expect(hasError("(#a || #b) && #c")).toBe(false);
    expect(hasError("not #done and @will or #bug")).toBe(false);
  });
});
