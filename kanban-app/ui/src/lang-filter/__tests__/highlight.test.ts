/**
 * Verify that syntax highlighting classes are applied to correct node types.
 *
 * Uses the parser + highlightTree to confirm that each atom type and operator
 * receives the expected highlight tag from the styleTags configuration.
 *
 * Tags and mentions are NOT syntax-highlighted — they get their visual styling
 * from the mention decoration system (colored pills), not from Lezer tags.
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
  it("does NOT highlight tags (decoration system handles them)", () => {
    const classes = getHighlightClasses("#bug");
    expect(classes.some((c) => c.includes("tok-typeName") && c.includes("#bug"))).toBe(false);
  });

  it("does NOT highlight mentions (decoration system handles them)", () => {
    const classes = getHighlightClasses("@alice");
    expect(classes.some((c) => c.includes("tok-variableName") && c.includes("@alice"))).toBe(false);
  });

  it("does NOT highlight projects (decoration system handles them)", () => {
    // Projects, like Tags and Mentions, must not receive a styleTag mapping —
    // defaultHighlightStyle would otherwise override their pill decoration color.
    const classes = getHighlightClasses("$auth");
    const projectClass = classes.find((c) => c.endsWith(":$auth"));
    // The project token should carry no highlight class (decoration system owns it).
    expect(projectClass).toBeUndefined();
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

  it("complex expression: operators highlighted, tags/mentions not", () => {
    const classes = getHighlightClasses("#bug && @will || !#done");
    expect(classes.some((c) => c.includes("tok-typeName"))).toBe(false);
    expect(classes.some((c) => c.includes("tok-variableName"))).toBe(false);
    expect(classes.some((c) => c.includes("tok-operator"))).toBe(true);
  });
});
