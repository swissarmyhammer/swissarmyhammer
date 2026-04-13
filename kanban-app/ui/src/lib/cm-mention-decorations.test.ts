/**
 * Tests for the cm-mention-decorations module.
 *
 * Instantiates a real CM6 EditorView with the mention decoration extension,
 * feeds a document containing known mentions, and verifies decorations are
 * emitted at the correct positions with the correct color attributes.
 *
 * Proves the metaFacet wires MentionMeta.color through to mark decorations.
 */

import { describe, it, expect } from "vitest";
import { EditorView } from "@codemirror/view";
import { EditorState } from "@codemirror/state";
import { createMentionDecorations } from "./cm-mention-decorations";
import type { MentionMeta } from "./mention-meta";

/** Create a minimal CM6 EditorView with given extensions and doc text. */
function createEditor(extensions: import("@codemirror/state").Extension[], doc = "") {
  const parent = document.createElement("div");
  document.body.appendChild(parent);
  const view = new EditorView({
    state: EditorState.create({ doc, extensions }),
    parent,
  });
  return { view, parent };
}

describe("createMentionDecorations", () => {
  it("decorates a known mention with the correct color from metaFacet", () => {
    const { extension } = createMentionDecorations("#", "cm-tag-pill", "--tag-color");

    const meta = new Map<string, MentionMeta>([
      ["bug", { color: "ff0000", displayName: "bug" }],
    ]);

    const { view, parent } = createEditor([extension(meta)], "#bug");

    const pill = parent.querySelector(".cm-tag-pill");
    expect(pill).toBeTruthy();
    expect(pill?.textContent).toBe("#bug");
    // The inline style should set the color variable
    const style = (pill as HTMLElement)?.getAttribute("style") ?? "";
    expect(style).toContain("--tag-color");
    expect(style).toContain("#ff0000");

    view.destroy();
    parent.remove();
  });

  it("applies default mark class when slug has no color in meta", () => {
    const { extension } = createMentionDecorations("#", "cm-tag-pill", "--tag-color");

    // Provide an entry with empty color to test the default path
    const meta = new Map<string, MentionMeta>([
      ["bug", { color: "", displayName: "bug" }],
    ]);

    const { view, parent } = createEditor([extension(meta)], "#bug");

    const pill = parent.querySelector(".cm-tag-pill");
    expect(pill).toBeTruthy();

    view.destroy();
    parent.remove();
  });

  it("does not decorate mentions inside fenced code blocks", () => {
    const { extension } = createMentionDecorations("#", "cm-tag-pill", "--tag-color");

    const meta = new Map<string, MentionMeta>([
      ["bug", { color: "ff0000", displayName: "bug" }],
    ]);

    const doc = "```\n#bug\n```";
    const { view, parent } = createEditor([extension(meta)], doc);

    const pill = parent.querySelector(".cm-tag-pill");
    expect(pill).toBeFalsy();

    view.destroy();
    parent.remove();
  });

  it("does not decorate mentions in markdown headings", () => {
    const { extension } = createMentionDecorations("#", "cm-tag-pill", "--tag-color");

    const meta = new Map<string, MentionMeta>([
      ["bug", { color: "ff0000", displayName: "bug" }],
    ]);

    const doc = "## #bug";
    const { view, parent } = createEditor([extension(meta)], doc);

    const pill = parent.querySelector(".cm-tag-pill");
    expect(pill).toBeFalsy();

    view.destroy();
    parent.remove();
  });

  it("decorates multiple mentions on separate lines", () => {
    const { extension } = createMentionDecorations("#", "cm-tag-pill", "--tag-color");

    const meta = new Map<string, MentionMeta>([
      ["bug", { color: "ff0000", displayName: "bug" }],
      ["feature", { color: "00ff00", displayName: "feature" }],
    ]);

    // Pad start so cursor at 0 is away from both mentions
    const doc = "hello\n#bug\n#feature";
    const { view, parent } = createEditor([extension(meta)], doc);

    // Both should render as widgets when cursor is away
    const widgets = parent.querySelectorAll(".cm-mention-pill");
    expect(widgets.length).toBe(2);

    view.destroy();
    parent.remove();
  });

  it("exposes metaFacet for external consumption", () => {
    const infra = createMentionDecorations("#", "cm-tag-pill", "--tag-color");
    // The returned object should have a metaFacet property (not colorsFacet)
    expect(infra.metaFacet).toBeDefined();
    expect(typeof infra.extension).toBe("function");
  });

  // ── Widget pipeline tests ─────────────────────────────────────────

  it("renders a widget with the display name when meta is available", () => {
    const { extension } = createMentionDecorations("#", "cm-tag-pill", "--tag-color");

    const meta = new Map<string, MentionMeta>([
      ["bug", { color: "ff0000", displayName: "Bug Report" }],
    ]);

    // Pad start so default cursor at 0 is not adjacent to the mention
    const { view, parent } = createEditor([extension(meta)], "hello #bug");

    // The widget should replace the slug text with the display name
    const widget = parent.querySelector(".cm-mention-pill");
    expect(widget).toBeTruthy();
    expect(widget?.textContent).toBe("#Bug Report");

    view.destroy();
    parent.remove();
  });

  it("shows raw slug with mark when cursor is inside the mention range", () => {
    const { extension } = createMentionDecorations("#", "cm-tag-pill", "--tag-color");

    const meta = new Map<string, MentionMeta>([
      ["bug", { color: "ff0000", displayName: "Bug Report" }],
    ]);

    // Document: "#bug" (positions 0-4)
    const { view, parent } = createEditor([extension(meta)], "#bug");

    // Move cursor inside the mention (position 2 = between 'b' and 'u')
    view.dispatch({ selection: { anchor: 2 } });

    // The widget should be gone, replaced with a mark decoration on the raw slug
    const widget = parent.querySelector(".cm-mention-pill");
    expect(widget).toBeFalsy();

    // The raw slug text should be visible with a mark-based pill class
    const pill = parent.querySelector(".cm-tag-pill");
    expect(pill).toBeTruthy();
    expect(pill?.textContent).toBe("#bug");

    view.destroy();
    parent.remove();
  });

  it("re-applies widget when cursor moves away from the mention", () => {
    const { extension } = createMentionDecorations("#", "cm-tag-pill", "--tag-color");

    const meta = new Map<string, MentionMeta>([
      ["bug", { color: "ff0000", displayName: "Bug Report" }],
    ]);

    // Document: "hello #bug world" — mention at positions 6-10
    // Start cursor away from mention so widget renders initially
    const { view, parent } = createEditor([extension(meta)], "hello #bug world");
    view.dispatch({ selection: { anchor: 0 } });

    // Widget should be present initially (cursor far from mention)
    expect(parent.querySelector(".cm-mention-pill")).toBeTruthy();

    // Move cursor inside the mention
    view.dispatch({ selection: { anchor: 8 } });
    expect(parent.querySelector(".cm-mention-pill")).toBeFalsy();
    expect(parent.querySelector(".cm-tag-pill")).toBeTruthy();

    // Now move cursor away (to position 0)
    view.dispatch({ selection: { anchor: 0 } });

    // Widget should be back
    const widget = parent.querySelector(".cm-mention-pill");
    expect(widget).toBeTruthy();
    expect(widget?.textContent).toBe("#Bug Report");

    view.destroy();
    parent.remove();
  });

  it("renders stale slug (no valid color) as muted mark, not widget", () => {
    const { extension } = createMentionDecorations("#", "cm-tag-pill", "--tag-color");

    // A slug whose metadata lacks a valid color is considered stale —
    // findMentionsInText only looks at keys in the meta map, so this is
    // the closest we get to "not in metaMap" (the slug is known but
    // without usable color info).
    const meta = new Map<string, MentionMeta>([
      ["bug", { color: "ff0000", displayName: "Bug Report" }],
      ["stale-tag", { color: "", displayName: "Stale Tag" }],
    ]);

    // Pad start so default cursor at 0 is not adjacent to the mentions
    const { view, parent } = createEditor(
      [extension(meta)],
      "hello #bug #stale-tag",
    );

    // The known mention should render as a widget with the display name
    const widget = parent.querySelector(".cm-mention-pill");
    expect(widget).toBeTruthy();
    expect(widget?.textContent).toBe("#Bug Report");

    // The stale mention should render as a mark showing the raw slug —
    // no widget, just the plain `#stale-tag` text with pill class.
    const marks = parent.querySelectorAll(".cm-tag-pill");
    expect(marks.length).toBeGreaterThanOrEqual(1);
    const staleTexts = Array.from(marks).map((m) => m.textContent);
    expect(staleTexts).toContain("#stale-tag");

    view.destroy();
    parent.remove();
  });
});
