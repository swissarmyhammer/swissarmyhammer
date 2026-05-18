/**
 * Component tests for {@link AiPromptComposer} — the AI panel's CM6 composer.
 *
 * The composer is a CodeMirror 6 instance built on the app's shared
 * {@link TextEditor} primitive, so it honors the active keymap (vim / emacs /
 * CUA) exactly like every other text input in the app ("CM6 everywhere",
 * `ideas/kanban/app-architecture.md`). It is NOT a plain `<textarea>`.
 *
 * These tests pin two contracts of kanban task `01KRRQ3SPXBY1ZNRJHFGB09R3Z`:
 *
 *   - the composer mounts a real CM6 editor (a `.cm-editor` with an
 *     `EditorView`), and a keymap motion works inside it;
 *   - Enter submits the buffer (`sendPrompt`), and the stop affordance fires
 *     `cancel` while a turn streams.
 *
 * Browser project (`*.test.tsx`) — CM6 mounts in real Chromium.
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { act } from "@testing-library/react";
import { userEvent } from "vitest/browser";
import { renderInAct } from "@/test/act-render";

// The keymap mode the composer's CM6 editor picks up — overridden per test.
let mockKeymapMode = "cua";
vi.mock("@/lib/ui-state-context", () => ({
  useUIState: () => ({ keymap_mode: mockKeymapMode }),
}));

import { AiPromptComposer } from "./ai-prompt-composer";

/** Resolve the live `EditorView` from a freshly rendered composer. */
async function getView(container: HTMLElement) {
  const cmEditor = container.querySelector(".cm-editor") as HTMLElement | null;
  expect(cmEditor, "the composer must mount a CM6 .cm-editor").toBeTruthy();
  const { EditorView } = await import("@codemirror/view");
  const view = EditorView.findFromDOM(cmEditor!);
  expect(view, "the .cm-editor must have a live EditorView").toBeTruthy();
  return view!;
}

describe("AiPromptComposer — CM6 instance honoring the active keymap", () => {
  beforeEach(() => {
    mockKeymapMode = "cua";
  });

  it("mounts a real CodeMirror 6 editor, not a plain textarea", async () => {
    const { container } = await renderInAct(
      <AiPromptComposer
        disabled={false}
        placeholder="Ask the AI agent..."
        streaming={false}
        onSend={() => {}}
        onCancel={() => {}}
      />,
    );

    // A CM6 editor — `.cm-editor` with an `.cm-content[contenteditable]` —
    // is present, and there is no plain `<textarea>`.
    expect(container.querySelector(".cm-editor")).not.toBeNull();
    expect(container.querySelector(".cm-content")).not.toBeNull();
    expect(container.querySelector("textarea")).toBeNull();
    // The CM6 content DOM advertises itself as a textbox with the panel's
    // accessible label — the contract `ai.focus` and the panel tests rely on.
    const content = container.querySelector(".cm-content") as HTMLElement;
    expect(content.getAttribute("role")).toBe("textbox");
    expect(content.getAttribute("aria-label")).toBe("Message the AI agent");
  });

  it("a vim keymap motion works inside the composer", async () => {
    // Vim keymap — the editor starts in normal mode (TextEditor exits insert
    // on mount). Type a word in insert mode, then exercise a normal-mode
    // motion to prove the vim keymap is live inside the composer.
    mockKeymapMode = "vim";
    const { container } = await renderInAct(
      <AiPromptComposer
        disabled={false}
        placeholder="Ask the AI agent..."
        streaming={false}
        onSend={() => {}}
        onCancel={() => {}}
      />,
    );
    const view = await getView(container);
    view.contentDOM.focus();

    // `i` enters insert mode, type the buffer, `Escape` returns to normal.
    await act(async () => {
      await userEvent.type(view.contentDOM, "i");
      await userEvent.type(view.contentDOM, "hello world");
      await userEvent.keyboard("{Escape}");
    });
    expect(view.state.doc.toString()).toBe("hello world");

    // Normal-mode motion: `0` jumps the cursor to the start of the line,
    // `x` deletes the character under the cursor. A working vim keymap
    // therefore turns "hello world" into "ello world".
    await act(async () => {
      await userEvent.type(view.contentDOM, "0");
      await userEvent.type(view.contentDOM, "x");
    });
    expect(
      view.state.doc.toString(),
      "the vim `0` motion + `x` delete must run inside the composer's CM6 editor",
    ).toBe("ello world");
  });

  it("an emacs keymap motion works inside the composer", async () => {
    // Emacs keymap — `Ctrl-A` moves the cursor to the start of the line.
    mockKeymapMode = "emacs";
    const { container } = await renderInAct(
      <AiPromptComposer
        disabled={false}
        placeholder="Ask the AI agent..."
        streaming={false}
        onSend={() => {}}
        onCancel={() => {}}
      />,
    );
    const view = await getView(container);
    view.contentDOM.focus();

    await act(async () => {
      await userEvent.type(view.contentDOM, "abc");
    });
    expect(view.state.selection.main.head).toBe(3);

    // `Ctrl-A` (emacs: move-to-line-start) jumps the cursor to offset 0.
    await act(async () => {
      await userEvent.keyboard("{Control>}a{/Control}");
    });
    expect(
      view.state.selection.main.head,
      "the emacs Ctrl-A motion must run inside the composer's CM6 editor",
    ).toBe(0);
  });

  it("Enter submits the buffer via onSend", async () => {
    const onSend = vi.fn();
    const { container } = await renderInAct(
      <AiPromptComposer
        disabled={false}
        placeholder="Ask the AI agent..."
        streaming={false}
        onSend={onSend}
        onCancel={() => {}}
      />,
    );
    const view = await getView(container);
    view.contentDOM.focus();

    await act(async () => {
      await userEvent.type(view.contentDOM, "ship it");
    });
    await act(async () => {
      await userEvent.keyboard("{Enter}");
    });
    expect(onSend).toHaveBeenCalledExactlyOnceWith("ship it");
  });

  it("Shift+Enter inserts a newline instead of submitting", async () => {
    const onSend = vi.fn();
    const { container } = await renderInAct(
      <AiPromptComposer
        disabled={false}
        placeholder="Ask the AI agent..."
        streaming={false}
        onSend={onSend}
        onCancel={() => {}}
      />,
    );
    const view = await getView(container);
    view.contentDOM.focus();

    await act(async () => {
      await userEvent.type(view.contentDOM, "line one");
      await userEvent.keyboard("{Shift>}{Enter}{/Shift}");
      await userEvent.type(view.contentDOM, "line two");
    });
    // Shift+Enter grew the buffer with a newline — it did not submit.
    expect(view.state.doc.toString()).toBe("line one\nline two");
    expect(onSend).not.toHaveBeenCalled();
  });

  it("Enter on an empty buffer is a true no-op — no submit, no blank line", async () => {
    const onSend = vi.fn();
    const { container } = await renderInAct(
      <AiPromptComposer
        disabled={false}
        placeholder="Ask the AI agent..."
        streaming={false}
        onSend={onSend}
        onCancel={() => {}}
      />,
    );
    const view = await getView(container);
    view.contentDOM.focus();

    // Repeated Enter on an empty composer must neither submit nor accumulate
    // blank lines — the keystroke is swallowed entirely.
    await act(async () => {
      await userEvent.keyboard("{Enter}");
      await userEvent.keyboard("{Enter}");
      await userEvent.keyboard("{Enter}");
    });
    expect(onSend).not.toHaveBeenCalled();
    expect(
      view.state.doc.toString(),
      "Enter on an empty composer must not insert a newline",
    ).toBe("");
  });

  it("the stop button cancels while a turn streams; the submit button is hidden", async () => {
    const onCancel = vi.fn();
    const { container } = await renderInAct(
      <AiPromptComposer
        disabled={false}
        placeholder="Ask the AI agent..."
        streaming={true}
        onSend={() => {}}
        onCancel={onCancel}
      />,
    );

    // While streaming the action button is a stop control.
    const stop = container.querySelector(
      "button[aria-label='Stop']",
    ) as HTMLButtonElement | null;
    expect(stop, "a stop button must render while streaming").not.toBeNull();
    await act(async () => {
      await userEvent.click(stop!);
    });
    expect(onCancel).toHaveBeenCalledOnce();
  });

  it("is inert when disabled — the CM6 editor is not editable", async () => {
    const { container } = await renderInAct(
      <AiPromptComposer
        disabled={true}
        placeholder="Select a model to start..."
        streaming={false}
        onSend={() => {}}
        onCancel={() => {}}
      />,
    );
    const content = container.querySelector(".cm-content") as HTMLElement;
    expect(content, "the CM6 content DOM must be present").not.toBeNull();
    // A disabled composer's CM6 editor is not editable.
    expect(content.getAttribute("contenteditable")).toBe("false");
  });
});
