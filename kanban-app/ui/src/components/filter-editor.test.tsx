import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, act } from "@testing-library/react";

// Mock Tauri APIs before importing any modules that use them.
// eslint-disable-next-line @typescript-eslint/no-explicit-any
const mockInvoke = vi.fn((..._args: any[]) => Promise.resolve(null));
vi.mock("@tauri-apps/api/core", () => ({
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  invoke: (...args: any[]) => mockInvoke(...args),
}));

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

// Mock UIState context for keymap mode — mutable so vim tests can switch.
let mockKeymapMode = "cua";
vi.mock("@/lib/ui-state-context", () => ({
  useUIState: () => ({ keymap_mode: mockKeymapMode }),
}));

// Mock schema and entity store for useMentionExtensions (used by filter editor).
vi.mock("@/lib/schema-context", () => ({
  useSchema: () => ({ mentionableTypes: [] }),
}));

vi.mock("@/lib/entity-store-context", () => ({
  useEntityStore: () => ({ getEntities: () => [] }),
}));

// Mock board data context — provides virtual tag metadata from the backend.
vi.mock("@/components/window-container", () => ({
  useBoardData: () => ({
    virtualTagMeta: [
      { slug: "READY", color: "0e8a16", description: "No unmet deps" },
      { slug: "BLOCKED", color: "e36209", description: "Has unmet deps" },
      { slug: "BLOCKING", color: "d73a4a", description: "Others depend on this" },
    ],
  }),
}));

import { FilterEditor } from "./filter-editor";

describe("FilterEditor", () => {
  // onClose is optional — formula bar usage doesn't need a close callback
  const defaultProps = {
    filter: "",
    perspectiveId: "p1",
  };

  beforeEach(() => {
    vi.clearAllMocks();
    mockKeymapMode = "cua";
  });

  it("renders the filter editor", () => {
    render(<FilterEditor {...defaultProps} />);

    expect(screen.getByTestId("filter-editor")).toBeDefined();
  });

  it("renders a CM6 editor", () => {
    const { container } = render(<FilterEditor {...defaultProps} />);

    // CM6 editor DOM node should be present
    expect(container.querySelector(".cm-editor")).toBeTruthy();
  });

  it("does not show clear button when filter is empty", () => {
    render(<FilterEditor {...defaultProps} filter="" />);

    expect(screen.queryByLabelText("Clear filter")).toBeNull();
  });

  it("shows clear button when filter is non-empty", () => {
    render(<FilterEditor {...defaultProps} filter="#bug && @will" />);

    expect(screen.getByLabelText("Clear filter")).toBeDefined();
  });

  it("dispatches clearFilter command when clear button is clicked", () => {
    render(<FilterEditor filter="#bug && @will" perspectiveId="p1" />);

    fireEvent.click(screen.getByLabelText("Clear filter"));

    expect(mockInvoke).toHaveBeenCalledWith(
      "dispatch_command",
      expect.objectContaining({
        cmd: "perspective.clearFilter",
        args: { perspective_id: "p1" },
      }),
    );
  });

  it("calls onClose when clear is clicked and onClose is provided", () => {
    const onClose = vi.fn();
    render(
      <FilterEditor
        filter="#bug && @will"
        perspectiveId="p1"
        onClose={onClose}
      />,
    );

    fireEvent.click(screen.getByLabelText("Clear filter"));
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  // =========================================================================
  // Autosave — debounced dispatch on every change
  // =========================================================================

  describe("autosave", () => {
    /** Get the CM6 EditorView from the rendered filter editor. */
    async function getEditorView(container: HTMLElement) {
      const cmEditor = container.querySelector(".cm-editor") as HTMLElement;
      const { EditorView } = await import("@codemirror/view");
      const view = EditorView.findFromDOM(cmEditor);
      expect(view).toBeTruthy();
      return view!;
    }

    it("dispatches filter after typing valid expression (debounced)", async () => {
      const { container } = render(
        <FilterEditor filter="" perspectiveId="p1" />,
      );
      const view = await getEditorView(container);

      await act(async () => {
        view.dispatch({
          changes: { from: 0, to: view.state.doc.length, insert: "#bug" },
        });
        await new Promise((r) => setTimeout(r, 400));
      });

      expect(mockInvoke).toHaveBeenCalledWith(
        "dispatch_command",
        expect.objectContaining({
          cmd: "perspective.filter",
          args: { filter: "#bug", perspective_id: "p1" },
        }),
      );
    });

    it("does not dispatch when expression is invalid", async () => {
      const { container } = render(
        <FilterEditor filter="" perspectiveId="p1" />,
      );
      const view = await getEditorView(container);

      await act(async () => {
        view.dispatch({
          changes: { from: 0, to: view.state.doc.length, insert: "#bug &&" },
        });
        await new Promise((r) => setTimeout(r, 400));
      });

      expect(mockInvoke).not.toHaveBeenCalledWith(
        "dispatch_command",
        expect.objectContaining({ cmd: "perspective.filter" }),
      );
    });

    it("dispatches clearFilter when text becomes empty", async () => {
      const { container } = render(
        <FilterEditor filter="#existing" perspectiveId="p1" />,
      );
      mockInvoke.mockClear();
      const view = await getEditorView(container);

      await act(async () => {
        view.dispatch({
          changes: { from: 0, to: view.state.doc.length, insert: "" },
        });
        await new Promise((r) => setTimeout(r, 400));
      });

      expect(mockInvoke).toHaveBeenCalledWith(
        "dispatch_command",
        expect.objectContaining({
          cmd: "perspective.clearFilter",
          args: { perspective_id: "p1" },
        }),
      );
    });

    it("dispatches filter for implicit AND expression (#paper #READY)", async () => {
      const { container } = render(
        <FilterEditor filter="" perspectiveId="p1" />,
      );
      const view = await getEditorView(container);

      await act(async () => {
        view.dispatch({
          changes: { from: 0, to: view.state.doc.length, insert: "#paper #READY" },
        });
        await new Promise((r) => setTimeout(r, 400));
      });

      expect(mockInvoke).toHaveBeenCalledWith(
        "dispatch_command",
        expect.objectContaining({
          cmd: "perspective.filter",
          args: { filter: "#paper #READY", perspective_id: "p1" },
        }),
      );
    });

    it("vim Escape from insert mode dispatches filter immediately (save-in-place)", async () => {
      mockKeymapMode = "vim";
      const { container } = render(
        <FilterEditor filter="" perspectiveId="p1" />,
      );
      const view = await getEditorView(container);

      // Set vim to insert mode — same pattern as cm-submit-cancel.test.ts
      const { getCM } = await import("@replit/codemirror-vim");
      const cm = getCM(view);
      expect(cm).toBeTruthy();
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      if (cm) (cm as any).state.vim.insertMode = true;

      // Type a filter expression while in insert mode
      await act(async () => {
        view.dispatch({
          changes: { from: 0, to: view.state.doc.length, insert: "#bug" },
        });
        await new Promise((r) => setTimeout(r, 20));
      });

      mockInvoke.mockClear();

      // Press Escape to exit insert mode — triggers saveInPlace via bubble phase
      await act(async () => {
        view.dom.dispatchEvent(
          new KeyboardEvent("keydown", {
            key: "Escape",
            bubbles: true,
            cancelable: true,
          }),
        );
        // Wait for setTimeout(0) in buildVimEscapeExtension bubble phase,
        // but NOT long enough for the 300ms autosave debounce.
        await new Promise((r) => setTimeout(r, 50));
      });

      // Should dispatch immediately via onCommit (not after 300ms debounce)
      expect(mockInvoke).toHaveBeenCalledWith(
        "dispatch_command",
        expect.objectContaining({
          cmd: "perspective.filter",
          args: { filter: "#bug", perspective_id: "p1" },
        }),
      );
    });
  });
});
