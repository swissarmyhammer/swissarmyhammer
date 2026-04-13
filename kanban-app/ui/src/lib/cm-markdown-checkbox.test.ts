/**
 * Tests for the cm-markdown-checkbox module.
 *
 * Instantiates a real CM6 EditorView with the checkbox extension, feeds
 * a document containing task-list checkboxes, and verifies the widget
 * replacement and the onToggle facet callback wiring.
 */

import { describe, it, expect, vi } from "vitest";
import { EditorView } from "@codemirror/view";
import { EditorState, type Extension } from "@codemirror/state";
import {
  createMarkdownCheckboxPlugin,
  checkboxToggleFacet,
} from "./cm-markdown-checkbox";

/** Create a minimal CM6 EditorView with given extensions and doc text. */
function createEditor(extensions: Extension[], doc = "") {
  const parent = document.createElement("div");
  document.body.appendChild(parent);
  const view = new EditorView({
    state: EditorState.create({ doc, extensions }),
    parent,
  });
  return { view, parent };
}

describe("createMarkdownCheckboxPlugin", () => {
  it("replaces `- [ ]` and `- [x]` with checkbox inputs", () => {
    const plugin = createMarkdownCheckboxPlugin();
    const doc = "- [ ] todo\n- [x] done";
    const { view, parent } = createEditor([plugin], doc);

    const checkboxes = parent.querySelectorAll<HTMLInputElement>(
      'input[type="checkbox"]',
    );
    expect(checkboxes).toHaveLength(2);
    expect(checkboxes[0].checked).toBe(false);
    expect(checkboxes[1].checked).toBe(true);

    view.destroy();
    parent.remove();
  });

  it("fires onToggle with sourceIndex=1 when the second checkbox is clicked", () => {
    const onToggle = vi.fn();
    const plugin = createMarkdownCheckboxPlugin();
    const doc = "- [ ] todo\n- [x] done";
    const { view, parent } = createEditor(
      [plugin, checkboxToggleFacet.of(onToggle)],
      doc,
    );

    const checkboxes = parent.querySelectorAll<HTMLInputElement>(
      'input[type="checkbox"]',
    );
    expect(checkboxes).toHaveLength(2);

    // Click the second checkbox
    checkboxes[1].click();

    expect(onToggle).toHaveBeenCalledTimes(1);
    expect(onToggle).toHaveBeenCalledWith(1);

    view.destroy();
    parent.remove();
  });

  it("fires onToggle with sourceIndex=0 when the first checkbox is clicked", () => {
    const onToggle = vi.fn();
    const plugin = createMarkdownCheckboxPlugin();
    const doc = "- [ ] todo\n- [x] done";
    const { view, parent } = createEditor(
      [plugin, checkboxToggleFacet.of(onToggle)],
      doc,
    );

    const checkboxes = parent.querySelectorAll<HTMLInputElement>(
      'input[type="checkbox"]',
    );
    checkboxes[0].click();

    expect(onToggle).toHaveBeenCalledWith(0);

    view.destroy();
    parent.remove();
  });

  it("handles five checkboxes with correct indices", () => {
    const onToggle = vi.fn();
    const plugin = createMarkdownCheckboxPlugin();
    const doc = ["- [ ] a", "- [ ] b", "- [ ] c", "- [x] d", "- [ ] e"].join(
      "\n",
    );
    const { view, parent } = createEditor(
      [plugin, checkboxToggleFacet.of(onToggle)],
      doc,
    );

    const checkboxes = parent.querySelectorAll<HTMLInputElement>(
      'input[type="checkbox"]',
    );
    expect(checkboxes).toHaveLength(5);

    checkboxes[2].click();
    expect(onToggle).toHaveBeenCalledWith(2);

    checkboxes[4].click();
    expect(onToggle).toHaveBeenCalledWith(4);

    view.destroy();
    parent.remove();
  });

  it("does not throw when no facet is provided", () => {
    const plugin = createMarkdownCheckboxPlugin();
    const doc = "- [ ] todo";
    const { view, parent } = createEditor([plugin], doc);

    const checkbox = parent.querySelector<HTMLInputElement>(
      'input[type="checkbox"]',
    );
    expect(checkbox).toBeTruthy();

    // Should not throw even without onToggle wired up
    expect(() => checkbox!.click()).not.toThrow();

    view.destroy();
    parent.remove();
  });

  it("renders checked attribute matching the markdown source", () => {
    const plugin = createMarkdownCheckboxPlugin();
    // Uppercase X should also count as checked
    const doc = "- [ ] a\n- [x] b\n- [X] c";
    const { view, parent } = createEditor([plugin], doc);

    const checkboxes = parent.querySelectorAll<HTMLInputElement>(
      'input[type="checkbox"]',
    );
    expect(checkboxes).toHaveLength(3);
    expect(checkboxes[0].checked).toBe(false);
    expect(checkboxes[1].checked).toBe(true);
    expect(checkboxes[2].checked).toBe(true);

    view.destroy();
    parent.remove();
  });
});
