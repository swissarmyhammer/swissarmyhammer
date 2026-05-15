import { describe, it, expect, afterEach } from "vitest";
import { render, cleanup, act } from "@testing-library/react";
import { useState } from "react";
import { Decoration, type DecorationSet, ViewPlugin } from "@codemirror/view";
import type { EditorView } from "@codemirror/view";
import { TextViewer } from "./text-viewer";

afterEach(cleanup);

describe("TextViewer", () => {
  it("renders plain text in a CM6 editor", () => {
    const { container } = render(<TextViewer text="hello world" />);
    const cmContent = container.querySelector(".cm-content");
    expect(cmContent).toBeTruthy();
    expect(cmContent?.textContent).toContain("hello world");
  });

  it("renders nothing when text is empty", () => {
    const { container } = render(<TextViewer text="" />);
    const cmEditor = container.querySelector(".cm-editor");
    expect(cmEditor).toBeNull();
  });

  it("applies caller-provided CM6 extensions", () => {
    // Create a decoration that marks the first 5 characters with a CSS class.
    const mark = Decoration.mark({ class: "test-highlight" });
    const plugin = ViewPlugin.fromClass(
      class {
        decorations: DecorationSet;
        constructor(view: EditorView) {
          const end = Math.min(5, view.state.doc.length);
          this.decorations = Decoration.set([mark.range(0, end)]);
        }
        update() {}
      },
      { decorations: (v) => v.decorations },
    );

    const { container } = render(
      <TextViewer text="hello world" extensions={[plugin]} />,
    );
    const highlighted = container.querySelector(".test-highlight");
    expect(highlighted).toBeTruthy();
  });

  it("preserves CM6 DOM across re-renders with same props (memoized)", async () => {
    // Wrapper that forces a parent re-render via counter state, while
    // keeping TextViewer props identical.
    let triggerRerender: (() => void) | undefined;
    function Parent() {
      const [count, setCount] = useState(0);
      triggerRerender = () => setCount((c) => c + 1);
      return (
        <div data-count={count}>
          <TextViewer text="stable content" />
        </div>
      );
    }

    const { container } = render(<Parent />);
    const editorBefore = container.querySelector(".cm-editor");
    expect(editorBefore).toBeTruthy();

    // Force a parent re-render — TextViewer props haven't changed
    await act(async () => {
      triggerRerender!();
    });

    const editorAfter = container.querySelector(".cm-editor");
    expect(editorAfter).toBeTruthy();
    // Same DOM node means CM6 was not remounted
    expect(editorAfter).toBe(editorBefore);
  });
});
