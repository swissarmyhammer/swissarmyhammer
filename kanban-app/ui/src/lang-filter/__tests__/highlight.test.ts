/**
 * Verify that syntax highlighting classes are applied to correct node types.
 *
 * Uses the parser + highlightTree to confirm that each atom type and operator
 * receives the expected highlight tag from the styleTags configuration.
 */

import { describe, it, expect } from "vitest";
import { highlightTree, classHighlighter } from "@lezer/highlight";
import { parser } from "../parser";

/** Collect all highlight class+text pairs applied to an input string. */
function getHighlightClasses(input: string): string[] {
  const tree = parser.parse(input);
  const classes: string[] = [];
  highlightTree(tree, classHighlighter, (from, to, cls) => {
    classes.push(`${cls}:${input.slice(from, to)}`);
  });
  return classes;
}

describe("filter grammar highlighting", () => {
  it("highlights tags with tok-typeName class", () => {
    const classes = getHighlightClasses("#bug");
    expect(classes.some((c) => c.includes("tok-typeName") && c.includes("#bug"))).toBe(true);
  });

  it("highlights mentions with tok-variableName class", () => {
    const classes = getHighlightClasses("@alice");
    expect(classes.some((c) => c.includes("tok-variableName") && c.includes("@alice"))).toBe(true);
  });

  it("highlights refs with tok-link class", () => {
    const classes = getHighlightClasses("^01ABC");
    expect(classes.some((c) => c.includes("tok-link") && c.includes("^01ABC"))).toBe(true);
  });

  it("highlights && with tok-operator class", () => {
    const classes = getHighlightClasses("#a && #b");
    expect(classes.some((c) => c.includes("tok-operator") && c.includes("&&"))).toBe(true);
  });

  it("highlights || with tok-operator class", () => {
    const classes = getHighlightClasses("#a || #b");
    expect(classes.some((c) => c.includes("tok-operator") && c.includes("||"))).toBe(true);
  });

  it("highlights ! with tok-operator class", () => {
    const classes = getHighlightClasses("!#a");
    expect(classes.some((c) => c.includes("tok-operator") && c.includes("!"))).toBe(true);
  });

  it("highlights keyword operators with tok-keyword class", () => {
    const classes = getHighlightClasses("not #a and #b or #c");
    expect(classes.some((c) => c.includes("tok-keyword") && c.includes("not"))).toBe(true);
    expect(classes.some((c) => c.includes("tok-keyword") && c.includes("and"))).toBe(true);
    expect(classes.some((c) => c.includes("tok-keyword") && c.includes("or"))).toBe(true);
  });

  it("applies distinct classes to all parts of a complex expression", () => {
    const classes = getHighlightClasses("#bug && @will || !#done");
    expect(classes.some((c) => c.includes("tok-typeName"))).toBe(true);
    expect(classes.some((c) => c.includes("tok-variableName"))).toBe(true);
    expect(classes.some((c) => c.includes("tok-operator"))).toBe(true);
  });
});
