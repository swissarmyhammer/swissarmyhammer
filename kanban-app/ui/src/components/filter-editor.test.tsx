import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, act } from "@testing-library/react";
import { pickedCompletion } from "@codemirror/autocomplete";

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
      {
        slug: "BLOCKING",
        color: "d73a4a",
        description: "Others depend on this",
      },
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
          changes: {
            from: 0,
            to: view.state.doc.length,
            insert: "#paper #READY",
          },
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

    it("placeholder advertises $project sigil", () => {
      const { container } = render(
        <FilterEditor filter="" perspectiveId="p1" />,
      );

      const placeholder = container.querySelector(".cm-placeholder");
      expect(placeholder).toBeTruthy();
      expect(placeholder?.textContent ?? "").toContain("$");
    });

    it("dispatches perspective.filter for $project expression", async () => {
      const { container } = render(
        <FilterEditor filter="" perspectiveId="p1" />,
      );
      const view = await getEditorView(container);

      await act(async () => {
        view.dispatch({
          changes: {
            from: 0,
            to: view.state.doc.length,
            insert: "$spatial-nav",
          },
        });
        await new Promise((r) => setTimeout(r, 400));
      });

      expect(mockInvoke).toHaveBeenCalledWith(
        "dispatch_command",
        expect.objectContaining({
          cmd: "perspective.filter",
          args: { filter: "$spatial-nav", perspective_id: "p1" },
        }),
      );
    });

    // =======================================================================
    // Enter/Escape commit tests — these MUST NEVER be removed or weakened.
    // They guard the primary save paths for the filter bar.
    // =======================================================================

    it("Enter key dispatches perspective.filter immediately (not debounced)", async () => {
      const { container } = render(
        <FilterEditor filter="" perspectiveId="p1" />,
      );
      const view = await getEditorView(container);

      // Type a filter
      await act(async () => {
        view.dispatch({
          changes: { from: 0, to: view.state.doc.length, insert: "#bug" },
        });
        await new Promise((r) => setTimeout(r, 20));
      });

      mockInvoke.mockClear();

      // Press Enter on the CM6 content area
      const cmContent = container.querySelector(".cm-content") as HTMLElement;
      await act(async () => {
        cmContent.dispatchEvent(
          new KeyboardEvent("keydown", {
            key: "Enter",
            bubbles: true,
            cancelable: true,
          }),
        );
        await new Promise((r) => setTimeout(r, 50));
      });

      // Must dispatch immediately — not after 300ms debounce
      expect(mockInvoke).toHaveBeenCalledWith(
        "dispatch_command",
        expect.objectContaining({
          cmd: "perspective.filter",
          args: { filter: "#bug", perspective_id: "p1" },
        }),
      );
    });

    it("Enter key dispatches perspective.clearFilter when text is empty", async () => {
      const { container } = render(
        <FilterEditor filter="#existing" perspectiveId="p1" />,
      );
      const view = await getEditorView(container);

      // Clear the text
      await act(async () => {
        view.dispatch({
          changes: { from: 0, to: view.state.doc.length, insert: "" },
        });
        await new Promise((r) => setTimeout(r, 20));
      });

      mockInvoke.mockClear();

      const cmContent = container.querySelector(".cm-content") as HTMLElement;
      await act(async () => {
        cmContent.dispatchEvent(
          new KeyboardEvent("keydown", {
            key: "Enter",
            bubbles: true,
            cancelable: true,
          }),
        );
        await new Promise((r) => setTimeout(r, 50));
      });

      expect(mockInvoke).toHaveBeenCalledWith(
        "dispatch_command",
        expect.objectContaining({
          cmd: "perspective.clearFilter",
          args: { perspective_id: "p1" },
        }),
      );
    });

    it("CUA Escape dispatches onClose callback", async () => {
      const onClose = vi.fn();
      const { container } = render(
        <FilterEditor filter="" perspectiveId="p1" onClose={onClose} />,
      );
      const view = await getEditorView(container);

      await act(async () => {
        view.dispatch({
          changes: { from: 0, to: view.state.doc.length, insert: "#bug" },
        });
        await new Promise((r) => setTimeout(r, 20));
      });

      const cmContent = container.querySelector(".cm-content") as HTMLElement;
      await act(async () => {
        cmContent.dispatchEvent(
          new KeyboardEvent("keydown", {
            key: "Escape",
            bubbles: true,
            cancelable: true,
          }),
        );
        await new Promise((r) => setTimeout(r, 50));
      });

      expect(onClose).toHaveBeenCalled();
    });

    // =======================================================================
     // Flush-on-accept / flush-on-unmount — guards the perspective-toggle race.
     // When the 300ms debounce is pending and the user either accepts an
     // autocomplete completion or the editor unmounts, the pending save must
     // fire so the user's last action is persisted.
     // =======================================================================

    it("flushes immediately when a completion is accepted", async () => {
      const { container } = render(
        <FilterEditor filter="" perspectiveId="p1" />,
      );
      const view = await getEditorView(container);

      mockInvoke.mockClear();

      // Dispatch a completion-accept transaction: inserts `#BLOCKING` and
      // carries the `pickedCompletion` annotation just like CM6 would if the
      // user had pressed Enter on the dropdown. The flush extension must
      // detect the annotation and bypass the 300ms debounce.
      await act(async () => {
        view.dispatch({
          changes: { from: 0, to: view.state.doc.length, insert: "#BLOCKING" },
          annotations: pickedCompletion.of({
            label: "#BLOCKING",
            apply: "#BLOCKING",
          }),
        });
        // One microtask + macrotask is enough for the flush to run —
        // the 300ms debounce must NOT have elapsed yet.
        await new Promise((r) => setTimeout(r, 20));
      });

      expect(mockInvoke).toHaveBeenCalledWith(
        "dispatch_command",
        expect.objectContaining({
          cmd: "perspective.filter",
          args: { filter: "#BLOCKING", perspective_id: "p1" },
        }),
      );
    });

    it("flushes pending autosave on unmount", async () => {
      const { container, unmount } = render(
        <FilterEditor filter="" perspectiveId="p1" />,
      );
      const view = await getEditorView(container);

      mockInvoke.mockClear();

      // Raw-typing change (no pickedCompletion annotation): only the 300ms
      // debounce is running. Unmount before it fires — the unmount-flush
      // effect must run the pending save synchronously.
      await act(async () => {
        view.dispatch({
          changes: { from: 0, to: view.state.doc.length, insert: "#BLOCKING" },
        });
        // Short wait so the debounce is scheduled but NOT yet fired.
        await new Promise((r) => setTimeout(r, 20));
        unmount();
      });

      expect(mockInvoke).toHaveBeenCalledWith(
        "dispatch_command",
        expect.objectContaining({
          cmd: "perspective.filter",
          args: { filter: "#BLOCKING", perspective_id: "p1" },
        }),
      );
    });

    it("does not flush after clear", async () => {
      const { unmount } = render(
        <FilterEditor filter="#bug" perspectiveId="p1" />,
      );

      mockInvoke.mockClear();

      // Click the clear button — dispatches perspective.clearFilter and
      // cancels the debounce. The clear supersedes any stale pending save.
      fireEvent.click(screen.getByLabelText("Clear filter"));

      // Count the dispatches after clear — must be exactly the clearFilter
      // command, no extra perspective.filter from a flushed stale value.
      const filterDispatchCallsBeforeUnmount = mockInvoke.mock.calls.filter(
        (call) =>
          call[0] === "dispatch_command" &&
          (call[1] as { cmd: string })?.cmd === "perspective.filter",
      ).length;

      await act(async () => {
        unmount();
        await new Promise((r) => setTimeout(r, 20));
      });

      const filterDispatchCallsAfterUnmount = mockInvoke.mock.calls.filter(
        (call) =>
          call[0] === "dispatch_command" &&
          (call[1] as { cmd: string })?.cmd === "perspective.filter",
      ).length;

      expect(filterDispatchCallsAfterUnmount).toBe(
        filterDispatchCallsBeforeUnmount,
      );
    });

    it("autocomplete accept then remount preserves accepted tag", async () => {
      const { container, unmount } = render(
        <FilterEditor filter="" perspectiveId="p1" />,
      );
      const view = await getEditorView(container);

      // Simulate the bug scenario: user types `#blo` (raw, no annotation),
      // debounce fires and saves `#blo`, then user accepts `#BLOCKING`
      // from the dropdown, then immediately toggles perspective (unmount).
      await act(async () => {
        view.dispatch({
          changes: { from: 0, to: view.state.doc.length, insert: "#blo" },
        });
        await new Promise((r) => setTimeout(r, 400));
      });

      // Completion-accept transaction replaces `#blo` with `#BLOCKING`.
      await act(async () => {
        view.dispatch({
          changes: { from: 0, to: view.state.doc.length, insert: "#BLOCKING" },
          annotations: pickedCompletion.of({
            label: "#BLOCKING",
            apply: "#BLOCKING",
          }),
        });
        // Short wait — under the 300ms debounce threshold.
        await new Promise((r) => setTimeout(r, 50));
        unmount();
      });

      // The final perspective.filter call must have been for `#BLOCKING`,
      // not `#blo` — the flush-on-accept path must have dispatched it.
      const filterCalls = mockInvoke.mock.calls.filter(
        (call) =>
          call[0] === "dispatch_command" &&
          (call[1] as { cmd: string })?.cmd === "perspective.filter",
      );
      expect(filterCalls.length).toBeGreaterThanOrEqual(1);
      const lastCall = filterCalls[filterCalls.length - 1];
      expect(lastCall[1]).toMatchObject({
        cmd: "perspective.filter",
        args: { filter: "#BLOCKING", perspective_id: "p1" },
      });
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
