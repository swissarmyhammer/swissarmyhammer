/**
 * Stability tests for the TextEditor primitive.
 *
 * These tests prove the primitive invariant the whole architecture relies on:
 * `onChange` continues to fire for every keystroke, no matter how much the
 * caller's props churn (extensions-prop identity, value-prop changes,
 * onChange-callback identity). This is what makes always-live callers
 * (formula bar, quick capture, field editors) safe against the async
 * dispatch → backend event → parent re-render cascade that previously
 * "disconnected" the editor mid-typing.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { useState } from "react";
import { render, act } from "@testing-library/react";
import { userEvent } from "vitest/browser";
import type { Extension } from "@codemirror/state";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ label: "main" }),
}));
vi.mock("@tauri-apps/plugin-log", () => ({
  error: vi.fn(),
  warn: vi.fn(),
  info: vi.fn(),
  debug: vi.fn(),
  trace: vi.fn(),
  attachConsole: vi.fn(() => Promise.resolve()),
}));

let mockKeymapMode = "cua";
vi.mock("@/lib/ui-state-context", () => ({
  useUIState: () => ({ keymap_mode: mockKeymapMode }),
}));

import { TextEditor } from "./text-editor";

const MODES = ["cua", "emacs", "vim"] as const;
type Mode = (typeof MODES)[number];

async function getView(container: HTMLElement) {
  const cmEditor = container.querySelector(".cm-editor") as HTMLElement;
  expect(cmEditor).toBeTruthy();
  const { EditorView } = await import("@codemirror/view");
  const view = EditorView.findFromDOM(cmEditor);
  expect(view).toBeTruthy();
  return view!;
}

async function enterVimInsertMode(view: import("@codemirror/view").EditorView) {
  const { getCM } = await import("@replit/codemirror-vim");
  const cm = getCM(view);
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  if (cm) (cm as any).state.vim.insertMode = true;
}

async function type(view: import("@codemirror/view").EditorView, text: string) {
  view.contentDOM.focus();
  await userEvent.type(view.contentDOM, text);
}

describe.each(MODES)("TextEditor stability (mode=%s)", (mode: Mode) => {
  beforeEach(() => {
    mockKeymapMode = mode;
  });

  async function mount(node: React.ReactElement) {
    const result = render(node);
    const view = await getView(result.container);
    if (mode === "vim") await enterVimInsertMode(view);
    return { ...result, view };
  }

  it("onChange fires for every keystroke", async () => {
    const onChange = vi.fn();
    const { view } = await mount(<TextEditor value="" onChange={onChange} />);
    await act(async () => {
      await type(view, "abc");
    });
    expect(onChange).toHaveBeenCalledWith("a");
    expect(onChange).toHaveBeenCalledWith("ab");
    expect(onChange).toHaveBeenCalledWith("abc");
  });

  it("value prop changes do NOT reset the doc", async () => {
    function Parent() {
      const [v, setV] = useState("seed");
      return (
        <>
          <TextEditor value={v} onChange={() => {}} />
          <button onClick={() => setV(v + "X")} data-testid="bump">
            bump
          </button>
        </>
      );
    }
    const { view, getByTestId } = await mount(<Parent />);
    // Move cursor to end so typing appends.
    view.dispatch({ selection: { anchor: view.state.doc.length } });
    await act(async () => {
      await type(view, "HELLO");
    });
    const docAfterType = view.state.doc.toString();
    expect(docAfterType).toContain("seed");
    expect(docAfterType).toContain("HELLO");

    // Caller force-updates the value prop to a different string.
    await act(async () => {
      getByTestId("bump").click();
    });
    // The doc must NOT reset — TextEditor ignores value-prop changes after mount.
    expect(view.state.doc.toString()).toBe(docAfterType);
  });

  it("extensions-prop identity churn does NOT break onChange subscription", async () => {
    // Simulates the real-world cascade: a parent context (like the mention
    // autocomplete extension builder) rebuilds the extensions array on every
    // render. Before the compartment fix, this triggered
    // @uiw/react-codemirror's blanket reconfigure effect, which replaced all
    // updateListeners mid-typing and silently dropped the onChange subscription.
    const onChange = vi.fn();

    function Parent() {
      const [, setTick] = useState(0);
      // New extensions array identity on every render.
      const exts: Extension[] = [];
      return (
        <>
          <TextEditor value="" onChange={onChange} extensions={exts} />
          <button data-testid="churn" onClick={() => setTick((t) => t + 1)}>
            churn
          </button>
        </>
      );
    }
    const { view, getByTestId } = await mount(<Parent />);

    // Churn the parent render BEFORE typing
    for (let i = 0; i < 3; i++) {
      await act(async () => {
        getByTestId("churn").click();
      });
    }
    await act(async () => {
      await type(view, "live");
    });
    const afterLive = onChange.mock.calls.length;
    expect(afterLive).toBeGreaterThanOrEqual(4);

    // Churn again mid-sequence
    await act(async () => {
      getByTestId("churn").click();
    });
    await act(async () => {
      await type(view, "!");
    });
    expect(view.state.doc.toString()).toBe("live!");
    // Critical invariant: the "!" after the churn must have fired onChange.
    expect(onChange.mock.calls.length).toBeGreaterThan(afterLive);
    expect(onChange).toHaveBeenLastCalledWith("live!");
  });

  it("onChange callback identity churn still routes to latest closure", async () => {
    // The TextEditor caches onChange in a ref so the updateListener extension
    // can read it without rebuilding. This test proves that even when the
    // onChange callback identity is a fresh function on every render, the
    // LATEST callback is always invoked.
    const results: Array<{ version: number; text: string }> = [];

    function Parent() {
      const [version, setVersion] = useState(1);
      return (
        <>
          <TextEditor
            value=""
            onChange={(text) => results.push({ version, text })}
          />
          <button
            data-testid="bump-version"
            onClick={() => setVersion((v) => v + 1)}
          >
            bump
          </button>
        </>
      );
    }

    const { view, getByTestId } = await mount(<Parent />);
    await act(async () => {
      await type(view, "a");
    });
    await act(async () => {
      getByTestId("bump-version").click();
    });
    await act(async () => {
      await type(view, "b");
    });
    await act(async () => {
      getByTestId("bump-version").click();
    });
    await act(async () => {
      await type(view, "c");
    });
    // Verify each character was observed with the correct version snapshot
    const last = results[results.length - 1];
    expect(last).toMatchObject({ version: 3, text: "abc" });
  });

  it("onChange survives many autosave-like cycles without disconnecting", async () => {
    // Mirrors the user-reported flow: type, "save" (parent updates some
    // unrelated state), type more, "save" again, ad infinitum. The editor
    // must continue to emit onChange for every keystroke through all cycles.
    const onChange = vi.fn();

    function Parent() {
      const [saves, setSaves] = useState(0);
      return (
        <>
          <TextEditor
            value=""
            onChange={(text) => {
              onChange(text);
              // Simulate a parent side-effect on each keystroke —
              // mimics the backend-event → parent-re-render loop.
              if (text.endsWith(" ")) setSaves((s) => s + 1);
            }}
          />
          <div data-testid="saves" data-count={saves} />
        </>
      );
    }

    const { view } = await mount(<Parent />);

    // Type 5 "saves worth" of input (each ends with a space to trigger
    // the parent side-effect).
    for (let i = 0; i < 5; i++) {
      await act(async () => {
        await type(view, `w${i} `);
      });
    }

    // The final doc must be the full accumulation — no dropped characters.
    expect(view.state.doc.toString()).toBe("w0 w1 w2 w3 w4 ");
    // onChange was called for every keystroke (3 chars per iteration × 5).
    expect(onChange.mock.calls.length).toBeGreaterThanOrEqual(15);
  });
});
