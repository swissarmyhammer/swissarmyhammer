import { describe, it, expect, vi, afterEach } from "vitest";
import { EditorView } from "@codemirror/view";
import { EditorState } from "@codemirror/state";
import { vim, getCM } from "@replit/codemirror-vim";
import { buildSubmitCancelExtensions } from "./cm-submit-cancel";

// Suppress console.log from debug logging in the module under test
vi.spyOn(console, "log").mockImplementation(() => {});

/** Create a minimal CM6 EditorView with the given extensions and initial doc. */
function createEditor(extensions: import("@codemirror/state").Extension[], doc = "") {
  const parent = document.createElement("div");
  document.body.appendChild(parent);
  const view = new EditorView({
    state: EditorState.create({ doc, extensions }),
    parent,
  });
  return { view, parent, cleanup: () => { view.destroy(); parent.remove(); } };
}

/** Simulate a keydown event on the editor's DOM element. */
function simulateKeydown(target: HTMLElement, key: string, opts?: KeyboardEventInit) {
  const event = new KeyboardEvent("keydown", {
    key,
    bubbles: true,
    cancelable: true,
    ...opts,
  });
  target.dispatchEvent(event);
  return event;
}

describe("buildSubmitCancelExtensions", () => {
  const makeRefs = () => ({
    onSubmitRef: { current: vi.fn() as (() => void) | null },
    onCancelRef: { current: vi.fn() as (() => void) | null },
    saveInPlaceRef: { current: vi.fn() as (() => void) | null },
  });

  // --- Structural tests ---

  it("returns non-empty extensions for vim mode", () => {
    const exts = buildSubmitCancelExtensions({ mode: "vim", ...makeRefs() });
    expect(Array.isArray(exts)).toBe(true);
    expect(exts.length).toBeGreaterThan(0);
  });

  it("returns non-empty extensions for cua mode", () => {
    const exts = buildSubmitCancelExtensions({ mode: "cua", ...makeRefs() });
    expect(Array.isArray(exts)).toBe(true);
    expect(exts.length).toBeGreaterThan(0);
  });

  it("returns non-empty extensions for emacs mode", () => {
    const exts = buildSubmitCancelExtensions({ mode: "emacs", ...makeRefs() });
    expect(Array.isArray(exts)).toBe(true);
    expect(exts.length).toBeGreaterThan(0);
  });

  it("works without saveInPlaceRef", () => {
    const exts = buildSubmitCancelExtensions({
      mode: "vim",
      onSubmitRef: { current: () => {} },
      onCancelRef: { current: () => {} },
    });
    expect(exts.length).toBeGreaterThan(0);
  });

  it("handles null ref values gracefully", () => {
    const exts = buildSubmitCancelExtensions({
      mode: "cua",
      onSubmitRef: { current: null },
      onCancelRef: { current: null },
    });
    expect(exts.length).toBeGreaterThan(0);
  });

  it("vim mode returns 2 extensions when singleLine (default)", () => {
    const exts = buildSubmitCancelExtensions({ mode: "vim", ...makeRefs() });
    // ViewPlugin for Enter + domEventHandlers for Escape
    expect(exts.length).toBe(2);
  });

  it("vim mode returns 1 extension when singleLine=false", () => {
    const exts = buildSubmitCancelExtensions({ mode: "vim", ...makeRefs(), singleLine: false });
    // Only domEventHandlers for Escape (no Enter handler)
    expect(exts.length).toBe(1);
  });

  it("cua mode returns 1 extension (Prec.highest keymap)", () => {
    const exts = buildSubmitCancelExtensions({ mode: "cua", ...makeRefs() });
    expect(exts.length).toBe(1);
  });

  // --- Vim mode behavioral tests with real CM6 EditorView ---

  describe("vim mode with real EditorView", () => {
    let cleanup: () => void;

    afterEach(() => {
      cleanup?.();
    });

    it("Enter in normal mode calls onSubmitRef when doc has content", () => {
      const refs = makeRefs();
      const extensions = [
        vim(),
        ...buildSubmitCancelExtensions({ mode: "vim", ...refs }),
      ];
      const { view, cleanup: c } = createEditor(extensions, "hello");
      cleanup = c;

      // Ensure vim is in normal mode (not insert)
      const cm = getCM(view);
      expect(cm).toBeTruthy();
      expect(cm!.state.vim?.insertMode).toBeFalsy();

      // Simulate Enter keydown on the cm-editor element (capture phase)
      simulateKeydown(view.dom, "Enter");

      expect(refs.onSubmitRef.current).toHaveBeenCalledOnce();
    });

    it("Enter in normal mode does nothing when doc is empty", () => {
      const refs = makeRefs();
      const extensions = [
        vim(),
        ...buildSubmitCancelExtensions({ mode: "vim", ...refs }),
      ];
      const { view, cleanup: c } = createEditor(extensions, "");
      cleanup = c;

      simulateKeydown(view.dom, "Enter");

      expect(refs.onSubmitRef.current).not.toHaveBeenCalled();
    });

    it("Enter in insert mode does NOT call onSubmitRef", () => {
      const refs = makeRefs();
      const extensions = [
        vim(),
        ...buildSubmitCancelExtensions({ mode: "vim", ...refs }),
      ];
      const { view, cleanup: c } = createEditor(extensions, "hello");
      cleanup = c;

      // Enter insert mode
      const cm = getCM(view);
      if (cm) {
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        (cm as any).state.vim.insertMode = true;
      }

      simulateKeydown(view.dom, "Enter");

      expect(refs.onSubmitRef.current).not.toHaveBeenCalled();
    });

    it("Escape in normal mode calls onCancelRef", () => {
      const refs = makeRefs();
      const extensions = [
        vim(),
        ...buildSubmitCancelExtensions({ mode: "vim", ...refs }),
      ];
      const { view, cleanup: c } = createEditor(extensions, "hello");
      cleanup = c;

      // Ensure normal mode
      const cm = getCM(view);
      expect(cm!.state.vim?.insertMode).toBeFalsy();

      simulateKeydown(view.contentDOM, "Escape");

      expect(refs.onCancelRef.current).toHaveBeenCalledOnce();
    });

    it("Escape in insert mode does NOT call onCancelRef", () => {
      const refs = makeRefs();
      const extensions = [
        vim(),
        ...buildSubmitCancelExtensions({ mode: "vim", ...refs }),
      ];
      const { view, cleanup: c } = createEditor(extensions, "hello");
      cleanup = c;

      const cm = getCM(view);
      if (cm) {
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        (cm as any).state.vim.insertMode = true;
      }

      simulateKeydown(view.contentDOM, "Escape");

      expect(refs.onCancelRef.current).not.toHaveBeenCalled();
    });

    it("singleLine=false skips Enter handler entirely", () => {
      const refs = makeRefs();
      const extensions = [
        vim(),
        ...buildSubmitCancelExtensions({ mode: "vim", ...refs, singleLine: false }),
      ];
      const { view, cleanup: c } = createEditor(extensions, "hello");
      cleanup = c;

      simulateKeydown(view.dom, "Enter");

      expect(refs.onSubmitRef.current).not.toHaveBeenCalled();
    });
  });

  // --- CUA/emacs mode behavioral tests ---

  describe("CUA mode with real EditorView", () => {
    let cleanup: () => void;

    afterEach(() => {
      cleanup?.();
    });

    it("Enter calls onSubmitRef", () => {
      const refs = makeRefs();
      const extensions = [
        ...buildSubmitCancelExtensions({ mode: "cua", ...refs }),
      ];
      const { view, cleanup: c } = createEditor(extensions, "hello");
      cleanup = c;

      // CM6 keymap handlers fire via the view's key dispatch
      view.contentDOM.dispatchEvent(
        new KeyboardEvent("keydown", { key: "Enter", bubbles: true, cancelable: true })
      );

      expect(refs.onSubmitRef.current).toHaveBeenCalledOnce();
    });

    it("Escape calls onCancelRef", () => {
      const refs = makeRefs();
      const extensions = [
        ...buildSubmitCancelExtensions({ mode: "cua", ...refs }),
      ];
      const { view, cleanup: c } = createEditor(extensions, "hello");
      cleanup = c;

      view.contentDOM.dispatchEvent(
        new KeyboardEvent("keydown", { key: "Escape", bubbles: true, cancelable: true })
      );

      expect(refs.onCancelRef.current).toHaveBeenCalledOnce();
    });

    it("singleLine=false skips Enter binding", () => {
      const refs = makeRefs();
      const extensions = [
        ...buildSubmitCancelExtensions({ mode: "cua", ...refs, singleLine: false }),
      ];
      const { view, cleanup: c } = createEditor(extensions, "hello");
      cleanup = c;

      view.contentDOM.dispatchEvent(
        new KeyboardEvent("keydown", { key: "Enter", bubbles: true, cancelable: true })
      );

      expect(refs.onSubmitRef.current).not.toHaveBeenCalled();
    });
  });
});
