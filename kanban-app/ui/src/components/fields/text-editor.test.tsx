import { describe, it, expect, vi, afterEach } from "vitest";
import { render, cleanup, act } from "@testing-library/react";
import { createElement, type ReactNode } from "react";
import { TextEditor } from "./text-editor";

// ---------------------------------------------------------------------------
// Mocks — Tauri + UIStateProvider
// ---------------------------------------------------------------------------

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(() =>
    Promise.resolve({
      palette_open: false,
      palette_mode: "command",
      keymap_mode: "cua",
      scope_chain: [],
      open_boards: [],
      windows: {},
      recent_boards: [],
    }),
  ),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

import { UIStateProvider } from "@/lib/ui-state-context";

/** Wrap component in UIStateProvider so useUIState() works. */
function Wrapper({ children }: { children: ReactNode }) {
  return createElement(UIStateProvider, null, children);
}

afterEach(cleanup);

// ---------------------------------------------------------------------------
// Smoke tests — the stripped-down TextEditor accepts only string-editing
// primitives: value, onChange, extensions, languageExtension, placeholder,
// singleLine, autoFocus. All commit/cancel/submit/blur policy lives in
// callers (see filter-editor.tsx, markdown.tsx, perspective-tab-bar.tsx,
// quick-capture.tsx).
// ---------------------------------------------------------------------------

describe("TextEditor smoke tests", () => {
  it("renders with minimal props (value)", () => {
    expect(() =>
      render(
        <Wrapper>
          <TextEditor value="hello" />
        </Wrapper>,
      ),
    ).not.toThrow();
  });

  it("renders with placeholder and onChange", () => {
    expect(() =>
      render(
        <Wrapper>
          <TextEditor
            value=""
            placeholder="Type here..."
            onChange={() => {}}
          />
        </Wrapper>,
      ),
    ).not.toThrow();
  });

  it("renders with singleLine flag", () => {
    expect(() =>
      render(
        <Wrapper>
          <TextEditor value="" singleLine />
        </Wrapper>,
      ),
    ).not.toThrow();
  });
});

// ---------------------------------------------------------------------------
// Behavioral tests
// ---------------------------------------------------------------------------

describe("TextEditor behavior", () => {
  it("mounts a CodeMirror editor in the DOM", () => {
    const { container } = render(
      <Wrapper>
        <TextEditor value="hello world" />
      </Wrapper>,
    );
    const cmEditor = container.querySelector(".cm-editor");
    expect(cmEditor).toBeTruthy();
  });

  it("displays the initial value in the editor", () => {
    const { container } = render(
      <Wrapper>
        <TextEditor value="test content" />
      </Wrapper>,
    );
    const cmContent = container.querySelector(".cm-content");
    expect(cmContent?.textContent).toContain("test content");
  });

  it("fires onChange when the document changes", async () => {
    const onChange = vi.fn();
    const { container } = render(
      <Wrapper>
        <TextEditor value="" onChange={onChange} />
      </Wrapper>,
    );
    const cmEditor = container.querySelector(".cm-editor") as HTMLElement;
    const { EditorView } = await import("@codemirror/view");
    const view = EditorView.findFromDOM(cmEditor);
    expect(view).toBeTruthy();

    await act(async () => {
      view!.dispatch({
        changes: { from: 0, to: 0, insert: "hi" },
      });
      await new Promise((r) => setTimeout(r, 20));
    });
    expect(onChange).toHaveBeenCalledWith("hi");
  });

  it("does not reset the document when parent passes new value prop", async () => {
    // Core invariant: once mounted, the CM6 buffer is the source of truth.
    // Parent re-renders with a different `value` must NOT clobber typed text.
    const { container, rerender } = render(
      <Wrapper>
        <TextEditor value="initial" />
      </Wrapper>,
    );
    const cmEditor = container.querySelector(".cm-editor") as HTMLElement;
    const { EditorView } = await import("@codemirror/view");
    const view = EditorView.findFromDOM(cmEditor)!;

    await act(async () => {
      view.dispatch({
        changes: { from: 0, to: view.state.doc.length, insert: "typed text" },
      });
      await new Promise((r) => setTimeout(r, 20));
    });

    rerender(
      <Wrapper>
        <TextEditor value="totally different parent value" />
      </Wrapper>,
    );

    expect(view.state.doc.toString()).toBe("typed text");
  });
});
