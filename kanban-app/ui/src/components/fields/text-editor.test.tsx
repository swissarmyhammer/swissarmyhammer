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

const noop = () => {};

// ---------------------------------------------------------------------------
// Smoke tests — these catch missing props, bad imports, and render crashes
// ---------------------------------------------------------------------------

describe("TextEditor smoke tests", () => {
  it("renders with minimal props (value, onCommit, onCancel)", () => {
    expect(() =>
      render(
        <Wrapper>
          <TextEditor value="hello" onCommit={noop} onCancel={noop} />
        </Wrapper>,
      ),
    ).not.toThrow();
  });

  it("renders with onSubmit (compact/board card mode)", () => {
    expect(() =>
      render(
        <Wrapper>
          <TextEditor
            value="hello"
            onCommit={noop}
            onCancel={noop}
            onSubmit={noop}
          />
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
            onCommit={noop}
            onCancel={noop}
            placeholder="Type here..."
            onChange={noop}
          />
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
        <TextEditor value="hello world" onCommit={noop} onCancel={noop} />
      </Wrapper>,
    );
    const cmEditor = container.querySelector(".cm-editor");
    expect(cmEditor).toBeTruthy();
  });

  it("displays the initial value in the editor", () => {
    const { container } = render(
      <Wrapper>
        <TextEditor value="test content" onCommit={noop} onCancel={noop} />
      </Wrapper>,
    );
    const cmContent = container.querySelector(".cm-content");
    expect(cmContent?.textContent).toContain("test content");
  });

  it("calls onChange on blur", async () => {
    const onChange = vi.fn();
    render(
      <Wrapper>
        <TextEditor
          value="blur test"
          onCommit={noop}
          onCancel={noop}
          onChange={onChange}
        />
      </Wrapper>,
    );
    // CM6 manages focus internally. Call blur() on the contenteditable
    // element so CM6's DOMObserver detects the focus loss.
    const cmContent = document.querySelector(".cm-content") as HTMLElement;
    expect(cmContent).toBeTruthy();
    await act(async () => {
      cmContent.blur();
      // CM6's DOMObserver polls focus state — give it a tick
      await new Promise((r) => setTimeout(r, 50));
    });
    expect(onChange).toHaveBeenCalledWith("blur test");
  });
});

// ---------------------------------------------------------------------------
// singleLine mode
// ---------------------------------------------------------------------------

describe("TextEditor singleLine mode", () => {
  it("renders without error", () => {
    expect(() =>
      render(
        <Wrapper>
          <TextEditor
            value="hello"
            onCommit={noop}
            onCancel={noop}
            singleLine
          />
        </Wrapper>,
      ),
    ).not.toThrow();
  });

  it("non-singleLine blur calls onChange, not onCommit", async () => {
    const onCommit = vi.fn();
    const onChange = vi.fn();
    render(
      <Wrapper>
        <TextEditor
          value="keep editing"
          onCommit={onCommit}
          onCancel={noop}
          onChange={onChange}
        />
      </Wrapper>,
    );
    const cmContent = document.querySelector(".cm-content") as HTMLElement;
    expect(cmContent).toBeTruthy();
    await act(async () => {
      cmContent.blur();
      await new Promise((r) => setTimeout(r, 50));
    });
    // Non-singleLine: blur fires onChange, NOT onCommit
    expect(onChange).toHaveBeenCalledWith("keep editing");
    expect(onCommit).not.toHaveBeenCalled();
  });

  it("singleLine CUA: Enter commits", async () => {
    const onCommit = vi.fn();
    render(
      <Wrapper>
        <TextEditor
          value="tab name"
          onCommit={onCommit}
          onCancel={noop}
          singleLine
        />
      </Wrapper>,
    );
    const cmContent = document.querySelector(".cm-content") as HTMLElement;
    expect(cmContent).toBeTruthy();
    await act(async () => {
      cmContent.dispatchEvent(
        new KeyboardEvent("keydown", { key: "Enter", bubbles: true }),
      );
      await new Promise((r) => setTimeout(r, 50));
    });
    expect(onCommit).toHaveBeenCalledWith("tab name");
  });

  it("singleLine CUA: Escape cancels", async () => {
    const onCancel = vi.fn();
    render(
      <Wrapper>
        <TextEditor
          value="tab name"
          onCommit={noop}
          onCancel={onCancel}
          singleLine
        />
      </Wrapper>,
    );
    const cmContent = document.querySelector(".cm-content") as HTMLElement;
    expect(cmContent).toBeTruthy();
    await act(async () => {
      cmContent.dispatchEvent(
        new KeyboardEvent("keydown", { key: "Escape", bubbles: true }),
      );
      await new Promise((r) => setTimeout(r, 50));
    });
    expect(onCancel).toHaveBeenCalled();
  });

});
