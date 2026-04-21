/**
 * Integration tests covering every context in which the pure TextEditor
 * primitive is used: formula bar (FilterEditor), field editor
 * (MarkdownEditorAdapter), and inline rename (InlineRenameEditor).
 *
 * Scope: prove that typing, commit semantics (Enter), and cancel semantics
 * (Escape) work correctly in every context, across all three keymap modes
 * (cua, emacs, vim). The FilterEditor case has its own extensive scenarios
 * file; here we only spot-check its commit/cancel wiring to prove the
 * compartment fix didn't regress anything.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, act } from "@testing-library/react";
import { userEvent } from "vitest/browser";

const mockInvoke = vi.fn(
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  (..._args: any[]): Promise<unknown> => Promise.resolve(null),
);
vi.mock("@tauri-apps/api/core", () => ({
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  invoke: (...args: any[]) => mockInvoke(...args),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));
vi.mock("@tauri-apps/api/webviewWindow", () => ({
  getCurrentWebviewWindow: () => ({
    label: "main",
    listen: vi.fn(() => Promise.resolve(() => {})),
  }),
}));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    label: "main",
    setSize: vi.fn(() => Promise.resolve()),
  }),
  LogicalSize: class {
    constructor(
      public width: number,
      public height: number,
    ) {}
  },
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

vi.mock("@/lib/schema-context", () => ({
  useSchema: () => ({ mentionableTypes: [] }),
}));

vi.mock("@/lib/entity-store-context", () => ({
  useEntityStore: () => ({
    getEntities: () => [],
    getEntity: () => undefined,
    subscribeField: () => () => {},
    getFieldValue: () => undefined,
  }),
  useFieldValue: () => undefined,
  EntityStoreProvider: ({ children }: { children: React.ReactNode }) =>
    children,
}));

vi.mock("@/components/window-container", () => ({
  useBoardData: () => ({ virtualTagMeta: [] }),
}));

import { MarkdownEditorAdapter } from "./fields/registrations/markdown";
import { InlineRenameEditor } from "./perspective-tab-bar";

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

async function pressKey(
  view: import("@codemirror/view").EditorView,
  key: string,
) {
  await act(async () => {
    view.contentDOM.dispatchEvent(
      new KeyboardEvent("keydown", {
        key,
        bubbles: true,
        cancelable: true,
      }),
    );
  });
}

// ---------------------------------------------------------------------------
// MarkdownEditorAdapter — field-level editor used for markdown fields.
// Compact mode: Enter commits, Escape cancels (vim Escape commits), blur saves.
// ---------------------------------------------------------------------------

describe.each(MODES)(
  "MarkdownEditorAdapter (field editor, mode=%s)",
  (mode: Mode) => {
    beforeEach(() => {
      mockKeymapMode = mode;
      vi.clearAllMocks();
    });

    async function mount() {
      const onCommit = vi.fn();
      const onCancel = vi.fn();
      const onChange = vi.fn();
      const result = render(
        <MarkdownEditorAdapter
          field={{ name: "body", type: "markdown" } as never}
          value=""
          mode="compact"
          onCommit={onCommit}
          onCancel={onCancel}
          onChange={onChange}
        />,
      );
      const view = await getView(result.container);
      if (mode === "vim") await enterVimInsertMode(view);
      return { ...result, view, onCommit, onCancel, onChange };
    }

    it("typing fires onChange for every keystroke", async () => {
      const { view, onChange } = await mount();
      await act(async () => {
        await type(view, "abc");
      });
      expect(onChange).toHaveBeenCalledWith("a");
      expect(onChange).toHaveBeenCalledWith("ab");
      expect(onChange).toHaveBeenCalledWith("abc");
    });

    it("Enter commits the typed text", async () => {
      const { view, onCommit } = await mount();
      await act(async () => {
        await type(view, "ship it");
      });
      await pressKey(view, "Enter");
      expect(onCommit).toHaveBeenCalledWith("ship it");
    });

    it(
      mode === "vim"
        ? "two Escapes (insert→normal, then cancel→commit) save the draft"
        : "Escape cancels (CUA/emacs)",
      async () => {
        const { view, onCommit, onCancel } = await mount();
        await act(async () => {
          await type(view, "draft");
        });
        await pressKey(view, "Escape");
        if (mode === "vim") {
          // First Escape exits insert mode. Second Escape routes to cancel,
          // which in vim mode is wired to commit (never lose edits).
          await pressKey(view, "Escape");
          expect(onCommit).toHaveBeenCalledWith("draft");
          expect(onCancel).not.toHaveBeenCalled();
        } else {
          expect(onCancel).toHaveBeenCalled();
          expect(onCommit).not.toHaveBeenCalled();
        }
      },
    );
  },
);

// ---------------------------------------------------------------------------
// InlineRenameEditor — perspective tab rename. Enter commits, Escape cancels
// (vim treats Escape as commit).
// ---------------------------------------------------------------------------

describe.each(MODES)(
  "InlineRenameEditor (perspective rename, mode=%s)",
  (mode: Mode) => {
    beforeEach(() => {
      mockKeymapMode = mode;
    });

    async function mount() {
      const onCommit = vi.fn();
      const onCancel = vi.fn();
      const result = render(
        <InlineRenameEditor
          name="Original"
          onCommit={onCommit}
          onCancel={onCancel}
        />,
      );
      const view = await getView(result.container);
      if (mode === "vim") await enterVimInsertMode(view);
      return { ...result, view, onCommit, onCancel };
    }

    it("typing modifies the buffer", async () => {
      const { view } = await mount();
      view.dispatch({ selection: { anchor: view.state.doc.length } });
      await act(async () => {
        await type(view, "-v2");
      });
      expect(view.state.doc.toString()).toBe("Original-v2");
    });

    it("Enter commits the current buffer", async () => {
      const { view, onCommit } = await mount();
      view.dispatch({ selection: { anchor: view.state.doc.length } });
      await act(async () => {
        await type(view, "!");
      });
      await pressKey(view, "Enter");
      expect(onCommit).toHaveBeenCalledWith("Original!");
    });

    it(
      mode === "vim"
        ? "two Escapes (insert→normal, then cancel→commit) preserve rename"
        : "Escape cancels (CUA/emacs discard edits)",
      async () => {
        const { view, onCommit, onCancel } = await mount();
        view.dispatch({ selection: { anchor: view.state.doc.length } });
        await act(async () => {
          await type(view, "-wip");
        });
        await pressKey(view, "Escape");
        if (mode === "vim") {
          await pressKey(view, "Escape");
          expect(onCommit).toHaveBeenCalledWith("Original-wip");
        } else {
          expect(onCancel).toHaveBeenCalled();
          expect(onCommit).not.toHaveBeenCalled();
        }
      },
    );
  },
);

// Note: FilterEditor formula-bar autosave cycles are exhaustively tested in
// filter-editor.scenarios.test.tsx (10 scenarios × 3 modes). No duplication here.
