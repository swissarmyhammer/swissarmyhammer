import React from "react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, act } from "@testing-library/react";
import { pickedCompletion } from "@codemirror/autocomplete";
import { userEvent } from "vitest/browser";

// Mock Tauri APIs before importing any modules that use them.
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
// Mutable so individual tests can register a `#` mentionable type and drive the
// real tag autocomplete pipeline.
let mockMentionableTypes: Array<{
  prefix: string;
  entityType: string;
  displayField: string;
}> = [];

vi.mock("@/lib/schema-context", () => ({
  useSchema: () => ({ mentionableTypes: mockMentionableTypes }),
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
    mockMentionableTypes = [];
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

    it("dispatches even when expression is invalid (buffer is source of truth)", async () => {
      // Mid-edit states are often invalid (`#bug &&`, trailing operators,
      // half-typed tags). The old behavior silently dropped these saves,
      // desyncing the UI buffer from the persisted filter. New contract:
      // always dispatch, surface the parse error via the local error state
      // for visual indication only.
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

      expect(mockInvoke).toHaveBeenCalledWith(
        "dispatch_command",
        expect.objectContaining({
          cmd: "perspective.filter",
          args: expect.objectContaining({ filter: "#bug &&" }),
        }),
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

    it("deleting all text dispatches perspective.clearFilter via debounce", async () => {
      // When the user clears the filter text, the debounced autosave fires
      // clearFilter after the 300ms debounce. Enter-on-empty does NOT flush
      // immediately because the onSubmit path in TextEditor skips empty
      // text — but the natural debounce still runs, so the clear still lands.
      const { container } = render(
        <FilterEditor filter="#existing" perspectiveId="p1" />,
      );
      const view = await getEditorView(container);

      mockInvoke.mockClear();

      // Clear the text — schedules apply("") via the debounced onChange.
      await act(async () => {
        view.dispatch({
          changes: { from: 0, to: view.state.doc.length, insert: "" },
        });
        // Wait past the 300ms debounce so the scheduled clear fires.
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

    // Regression tests for the bug where accepting a tag via keyboard Enter
    // fails to dispatch `perspective.filter` (and subsequently "disconnects"
    // the editor so follow-up edits also fail to save).
    //
    // These tests exercise the real completion pipeline — opening the
    // autocomplete via `startCompletion`, waiting for the async tag source to
    // resolve, then dispatching a real keyboard Enter event on the CM6
    // content. CM6's default apply-string path inserts the selected completion
    // and tags the transaction with `pickedCompletion`, which the flush
    // extension must observe.

    it("keyboard Enter on pending autocomplete does not dispatch a partial filter or disconnect the editor", async () => {
      // Reproduces the reported bug: user types `#blocki`, before the
      // async completion source resolves (status="pending"), hits Enter.
      // The submit handler must yield to autocomplete even during "pending",
      // not eat the Enter and dispatch `#blocki` as a stale partial.
      mockMentionableTypes = [
        { prefix: "#", entityType: "tag", displayField: "name" },
      ];
      // Never-resolving search promise — the completion stays in "pending"
      // state for the entire test. This mirrors the real race where Enter
      // arrives before the 150ms debounce + backend round-trip completes.
      let resolveSearch: ((results: unknown[]) => void) | null = null;
      const searchPromise = new Promise<unknown[]>((resolve) => {
        resolveSearch = resolve;
      });
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      mockInvoke.mockImplementation((command: string, _args?: any) => {
        if (command === "search_mentions") return searchPromise;
        return Promise.resolve(null);
      });

      const { container } = render(
        <FilterEditor filter="" perspectiveId="p1" />,
      );
      const view = await getEditorView(container);
      const { startCompletion, completionStatus } =
        await import("@codemirror/autocomplete");

      await act(async () => {
        view.dispatch({
          changes: { from: 0, to: view.state.doc.length, insert: "#blocki" },
          selection: { anchor: 7 },
        });
        // Wait long enough for the 300ms debounce to fire naturally — we
        // want to isolate the Enter-during-pending test from the natural
        // autosave path. Clearing mockInvoke below ensures the assertion is
        // only about what Enter causes.
        await new Promise((r) => setTimeout(r, 400));
      });
      view.focus();

      // Open the autocomplete. The async search is still pending because
      // `searchPromise` has not been resolved yet.
      await act(async () => {
        startCompletion(view);
        // Wait past the 150ms internal debounce inside CM6 autocomplete so
        // it has definitely entered the "pending" state.
        await new Promise((r) => setTimeout(r, 250));
      });

      // Precondition: completion is pending (waiting on the async source).
      expect(completionStatus(view.state)).toBe("pending");

      mockInvoke.mockClear();

      // Press Enter while pending. The invariant: Enter must not cause a new
      // dispatch — pending autocomplete should consume the Enter (or at least
      // prevent our submit handler from firing). Further edits must still
      // feed the autosave path.
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

      // No perspective.filter should fire with the partial `#blocki` in
      // response to the Enter — pending autocomplete must yield to the
      // completion pipeline, not the submit path.
      const partialCalls = mockInvoke.mock.calls.filter(
        (call) =>
          call[0] === "dispatch_command" &&
          (call[1] as { cmd: string })?.cmd === "perspective.filter" &&
          (call[1] as { args: { filter: string } })?.args?.filter === "#blocki",
      );
      expect(partialCalls).toHaveLength(0);

      // Now let the completion resolve — the tag gets inserted and dispatched.
      await act(async () => {
        resolveSearch?.([
          { id: "t1", display_name: "BLOCKING", color: "d73a4a" },
        ]);
        await new Promise((r) => setTimeout(r, 50));
      });

      // Subsequent edits must still feed the autosave — the editor is not
      // "disconnected". Append ` @will` and wait past the 300ms debounce.
      mockInvoke.mockClear();
      await act(async () => {
        view.dispatch({
          changes: {
            from: view.state.doc.length,
            to: view.state.doc.length,
            insert: " @will",
          },
        });
        await new Promise((r) => setTimeout(r, 400));
      });

      const filterCallsAfter = mockInvoke.mock.calls.filter(
        (call) =>
          call[0] === "dispatch_command" &&
          (call[1] as { cmd: string })?.cmd === "perspective.filter",
      );
      expect(filterCallsAfter.length).toBeGreaterThanOrEqual(1);
    });

    it("keyboard Enter on active autocomplete dispatches perspective.filter with the accepted tag", async () => {
      // Register `#` as a mentionable type so the tag completion source is live.
      mockMentionableTypes = [
        { prefix: "#", entityType: "tag", displayField: "name" },
      ];
      // Async source in `buildAsyncSearch` calls `search_mentions` — return a
      // tag so the autocomplete has something to select.
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      mockInvoke.mockImplementation((command: string, _args?: any) => {
        if (command === "search_mentions") {
          return Promise.resolve([
            { id: "t1", display_name: "BLOCKING", color: "d73a4a" },
          ]);
        }
        return Promise.resolve(null);
      });

      const { container } = render(
        <FilterEditor filter="" perspectiveId="p1" />,
      );
      const view = await getEditorView(container);
      const { startCompletion, currentCompletions } =
        await import("@codemirror/autocomplete");

      // Type `#blocki` into the editor so the completion source has a query
      // that narrows unambiguously to `#BLOCKING` (both virtual and real tag
      // sources will match it; `#BLOCKED` is excluded).
      await act(async () => {
        view.dispatch({
          changes: { from: 0, to: view.state.doc.length, insert: "#blocki" },
          selection: { anchor: 7 },
        });
        await new Promise((r) => setTimeout(r, 20));
      });
      view.focus();

      // Open the autocomplete programmatically (equivalent to the user
      // triggering it via typing / Ctrl-Space).
      await act(async () => {
        startCompletion(view);
        // Wait for the debounced async search (150ms) + generous slack.
        await new Promise((r) => setTimeout(r, 400));
      });

      // Precondition: autocomplete is active with at least one completion.
      expect(currentCompletions(view.state).length).toBeGreaterThan(0);

      mockInvoke.mockClear();

      // Dispatch a real keyboard Enter event on the CM6 content area.
      const cmContent = container.querySelector(".cm-content") as HTMLElement;
      await act(async () => {
        cmContent.dispatchEvent(
          new KeyboardEvent("keydown", {
            key: "Enter",
            bubbles: true,
            cancelable: true,
          }),
        );
        // Allow the microtask flush + any async dispatch to land. Must be
        // well under the 300ms debounce threshold.
        await new Promise((r) => setTimeout(r, 50));
      });

      // The accepted tag must have been dispatched via perspective.filter.
      const filterCalls = mockInvoke.mock.calls.filter(
        (call) =>
          call[0] === "dispatch_command" &&
          (call[1] as { cmd: string })?.cmd === "perspective.filter",
      );
      expect(filterCalls.length).toBeGreaterThanOrEqual(1);
      // The dispatched filter must be the completed `#BLOCKING`, not the
      // partial `#bl`.
      const lastCall = filterCalls[filterCalls.length - 1];
      expect(lastCall[1]).toMatchObject({
        cmd: "perspective.filter",
        args: { filter: "#BLOCKING", perspective_id: "p1" },
      });
      // No stale partial should have been dispatched during this Enter path.
      const partialCalls = filterCalls.filter(
        (call) =>
          (call[1] as { args: { filter: string } })?.args?.filter === "#blocki",
      );
      expect(partialCalls).toHaveLength(0);
    });

    it("editor continues to save after a keyboard-Enter completion accept", async () => {
      // Register `#` as a mentionable type so the tag completion source is live.
      mockMentionableTypes = [
        { prefix: "#", entityType: "tag", displayField: "name" },
      ];
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      mockInvoke.mockImplementation((command: string, _args?: any) => {
        if (command === "search_mentions") {
          return Promise.resolve([
            { id: "t1", display_name: "BLOCKING", color: "d73a4a" },
          ]);
        }
        return Promise.resolve(null);
      });

      const { container } = render(
        <FilterEditor filter="" perspectiveId="p1" />,
      );
      const view = await getEditorView(container);
      const { startCompletion } = await import("@codemirror/autocomplete");

      // Seed with `#blocki`, open autocomplete, accept `#BLOCKING` via Enter.
      await act(async () => {
        view.dispatch({
          changes: { from: 0, to: view.state.doc.length, insert: "#blocki" },
          selection: { anchor: 7 },
        });
        await new Promise((r) => setTimeout(r, 20));
      });
      view.focus();
      await act(async () => {
        startCompletion(view);
        await new Promise((r) => setTimeout(r, 400));
      });
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

      mockInvoke.mockClear();

      // Now simulate a subsequent edit — the user appends ` @will` to the
      // accepted tag. The editor must NOT have "disconnected"; the onChange
      // path must still feed the debounced autosave.
      await act(async () => {
        view.dispatch({
          changes: {
            from: view.state.doc.length,
            to: view.state.doc.length,
            insert: " @will",
          },
        });
        // Wait past the 300ms debounce so the autosave fires.
        await new Promise((r) => setTimeout(r, 400));
      });

      // perspective.filter must have been dispatched for the new value.
      const filterCalls = mockInvoke.mock.calls.filter(
        (call) =>
          call[0] === "dispatch_command" &&
          (call[1] as { cmd: string })?.cmd === "perspective.filter",
      );
      expect(filterCalls.length).toBeGreaterThanOrEqual(1);
      const lastCall = filterCalls[filterCalls.length - 1];
      expect(lastCall[1]).toMatchObject({
        cmd: "perspective.filter",
        args: { filter: "#BLOCKING @will", perspective_id: "p1" },
      });
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

    // =======================================================================
    // "Enter does not disconnect the editor" regressions — added 2026-04-16
    // after user reported that ANY Enter press breaks subsequent debounce
    // saves and that the original autocomplete-Enter bug was not actually
    // fixed. These drive the real CM6 keyboard path via userEvent and verify
    // that Enter NEVER destroys the pending debounced save.
    // =======================================================================

    it("real-keyboard typing (no Enter) dispatches filter after debounce", async () => {
      // Reproduces user feedback: "typing in the filter without pressing Enter
      // → no save". Uses real keyboard events via userEvent.type so the full
      // CM6 input pipeline runs (beforeinput → input → doc change), not just
      // a programmatic view.dispatch.
      const { container } = render(
        <FilterEditor filter="" perspectiveId="p1" />,
      );
      const cmContent = container.querySelector(".cm-content") as HTMLElement;
      expect(cmContent).toBeTruthy();
      cmContent.focus();
      mockInvoke.mockClear();

      await act(async () => {
        await userEvent.type(cmContent, "#bug");
        // Wait past the 300ms debounce.
        await new Promise((r) => setTimeout(r, 400));
      });

      const filterCalls = mockInvoke.mock.calls.filter(
        (call) =>
          call[0] === "dispatch_command" &&
          (call[1] as { cmd: string })?.cmd === "perspective.filter",
      );
      expect(filterCalls.length).toBeGreaterThanOrEqual(1);
      const lastCall = filterCalls[filterCalls.length - 1];
      expect(lastCall[1]).toMatchObject({
        cmd: "perspective.filter",
        args: { filter: "#bug", perspective_id: "p1" },
      });
    });

    it("Enter does not disconnect the editor — subsequent typing still dispatches", async () => {
      // Reproduces user feedback: "after any Enter press in the formula bar,
      // filtering is completely broken going forward". Invariant: after Enter,
      // the onChange → debounce → dispatch path must still work. Uses real
      // keyboard input via userEvent so CM6's full input pipeline runs — if
      // Enter destroys the debounce state or disrupts the input chain, this
      // test fails. Uses programmatic view.dispatch for the initial seed so
      // it does not depend on focus/autocomplete timing; the post-Enter path
      // uses real keystrokes to match the failure mode users observe.
      const { container } = render(
        <FilterEditor filter="" perspectiveId="p1" />,
      );
      const view = await getEditorView(container);
      const cmContent = container.querySelector(".cm-content") as HTMLElement;
      cmContent.focus();

      // Seed with `#bug` and press Enter. Under the broken design, this would
      // cancel the debounce AND dispatch `#bug` (net observable: one dispatch),
      // but would also leave the editor in a state where subsequent edits fail
      // to dispatch.
      await act(async () => {
        view.dispatch({
          changes: { from: 0, to: view.state.doc.length, insert: "#bug" },
          selection: { anchor: 4 },
        });
        await new Promise((r) => setTimeout(r, 20));
      });
      await act(async () => {
        await userEvent.keyboard("{Enter}");
        await new Promise((r) => setTimeout(r, 50));
      });

      // Now the actual test: type more via real keystrokes and verify the
      // debounce-dispatch path still works after Enter. If Enter "disconnects"
      // the editor (the user's symptom), these characters never dispatch.
      mockInvoke.mockClear();
      await act(async () => {
        await userEvent.type(cmContent, " @will");
        await new Promise((r) => setTimeout(r, 400));
      });

      const filterCallsAfter = mockInvoke.mock.calls.filter(
        (call) =>
          call[0] === "dispatch_command" &&
          (call[1] as { cmd: string })?.cmd === "perspective.filter",
      );
      expect(filterCallsAfter.length).toBeGreaterThanOrEqual(1);
      const lastAfter = filterCallsAfter[filterCallsAfter.length - 1];
      expect(lastAfter[1]).toMatchObject({
        cmd: "perspective.filter",
        args: { filter: "#bug @will", perspective_id: "p1" },
      });
    });

    it("Enter flushes a pending debounced save exactly once (no double-dispatch)", async () => {
      // Typing schedules a debounced save for the new text; pressing Enter
      // before the 300ms elapses must flush that exact save — not cancel it
      // and re-dispatch. Verifies the final-state invariant: the filter
      // dispatched matches the typed text, and only one dispatch carries
      // that final text.
      const { container } = render(
        <FilterEditor filter="" perspectiveId="p1" />,
      );
      const view = await getEditorView(container);
      const cmContent = container.querySelector(".cm-content") as HTMLElement;

      await act(async () => {
        view.dispatch({
          changes: { from: 0, to: view.state.doc.length, insert: "#bug" },
        });
        // Short wait so the debounce is scheduled but NOT yet fired.
        await new Promise((r) => setTimeout(r, 20));
      });
      mockInvoke.mockClear();

      await act(async () => {
        cmContent.dispatchEvent(
          new KeyboardEvent("keydown", {
            key: "Enter",
            bubbles: true,
            cancelable: true,
          }),
        );
        // Enough time for the flush microtask to run, well under 300ms.
        await new Promise((r) => setTimeout(r, 50));
      });

      // Exactly one perspective.filter dispatch for `#bug`. Flush fires the
      // scheduled callback synchronously; no separate direct apply call.
      const bugCalls = mockInvoke.mock.calls.filter(
        (call) =>
          call[0] === "dispatch_command" &&
          (call[1] as { cmd: string })?.cmd === "perspective.filter" &&
          (call[1] as { args: { filter: string } })?.args?.filter === "#bug",
      );
      expect(bugCalls).toHaveLength(1);

      // Wait past the would-be debounce window to confirm nothing else fires.
      await act(async () => {
        await new Promise((r) => setTimeout(r, 400));
      });
      const bugCallsAfter = mockInvoke.mock.calls.filter(
        (call) =>
          call[0] === "dispatch_command" &&
          (call[1] as { cmd: string })?.cmd === "perspective.filter" &&
          (call[1] as { args: { filter: string } })?.args?.filter === "#bug",
      );
      expect(bugCallsAfter).toHaveLength(1);
    });

    it("real-keyboard Enter on autocompleted tag dispatches #BLOCKING, not partial", async () => {
      // Reproduces the original user-reported bug with real keyboard input.
      // The existing test with the same name uses programmatic `view.dispatch`
      // for the Enter event — which mirrors the keymap path but may miss
      // subtle timing differences. This version uses `userEvent.keyboard` so
      // the CM6 input pipeline processes a genuine Enter.
      mockMentionableTypes = [
        { prefix: "#", entityType: "tag", displayField: "name" },
      ];
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      mockInvoke.mockImplementation((command: string, _args?: any) => {
        if (command === "search_mentions") {
          return Promise.resolve([
            { id: "t1", display_name: "BLOCKING", color: "d73a4a" },
          ]);
        }
        return Promise.resolve(null);
      });

      const { container } = render(
        <FilterEditor filter="" perspectiveId="p1" />,
      );
      const view = await getEditorView(container);
      const cmContent = container.querySelector(".cm-content") as HTMLElement;
      cmContent.focus();
      const { startCompletion } = await import("@codemirror/autocomplete");

      await act(async () => {
        view.dispatch({
          changes: { from: 0, to: view.state.doc.length, insert: "#blocki" },
          selection: { anchor: 7 },
        });
        await new Promise((r) => setTimeout(r, 20));
      });
      view.focus();
      await act(async () => {
        startCompletion(view);
        await new Promise((r) => setTimeout(r, 400));
      });

      mockInvoke.mockClear();

      // Real keyboard Enter via userEvent — exercises the actual input path.
      await act(async () => {
        await userEvent.keyboard("{Enter}");
        await new Promise((r) => setTimeout(r, 100));
      });

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
      // No stale partial dispatch.
      const partialCalls = filterCalls.filter(
        (call) =>
          (call[1] as { args: { filter: string } })?.args?.filter === "#blocki",
      );
      expect(partialCalls).toHaveLength(0);
    });

    it("parent re-render with new filter prop after Enter does not disconnect subsequent edits", async () => {
      // Reproduces the real-browser failure the user reported on 2026-04-16:
      // after Enter, the backend dispatch triggers a parent re-render with
      // the new `filter` prop. In production this happens every successful
      // dispatch. The unit tests with a null mockInvoke don't get this
      // re-render, which is why two previous fix passes appeared to work
      // in tests but failed in the browser.
      //
      // This test exercises the full loop: type → Enter → parent re-renders
      // with new filter prop → type more → verify dispatch still fires.
      function Wrapper({ initialFilter }: { initialFilter: string }) {
        const [filter, setFilter] = React.useState(initialFilter);
        // Mirror production: every perspective.filter dispatch updates
        // the prop we pass down, causing FilterEditor to re-render with a
        // new value prop. This is what backend events do in the real app.
        React.useEffect(() => {
          const origImpl = mockInvoke.getMockImplementation();
          mockInvoke.mockImplementation((cmd: string, args?: unknown) => {
            if (cmd === "dispatch_command") {
              const payload = args as {
                cmd?: string;
                args?: { filter?: string };
              };
              if (payload?.cmd === "perspective.filter") {
                queueMicrotask(() => setFilter(payload.args?.filter ?? ""));
              }
              if (payload?.cmd === "perspective.clearFilter") {
                queueMicrotask(() => setFilter(""));
              }
            }
            return origImpl ? origImpl(cmd, args) : Promise.resolve(null);
          });
          return () => {
            if (origImpl) mockInvoke.mockImplementation(origImpl);
          };
        }, []);
        return <FilterEditor filter={filter} perspectiveId="p1" />;
      }

      const { container } = render(<Wrapper initialFilter="" />);
      const view = await getEditorView(container);
      const cmContent = container.querySelector(".cm-content") as HTMLElement;
      cmContent.focus();

      // Type `#bug` and press Enter. This dispatches perspective.filter,
      // which (via the wrapper) updates the `filter` prop to `#bug`. The
      // parent re-render must not disconnect the editor.
      await act(async () => {
        view.dispatch({
          changes: { from: 0, to: view.state.doc.length, insert: "#bug" },
          selection: { anchor: 4 },
        });
        await new Promise((r) => setTimeout(r, 20));
      });
      await act(async () => {
        await userEvent.keyboard("{Enter}");
        // Wait for the dispatch + queued setFilter + re-render to settle.
        await new Promise((r) => setTimeout(r, 100));
      });

      // Now type more via real keyboard. If the Enter + re-render broke
      // the onChange pipeline, these keystrokes won't dispatch.
      mockInvoke.mockClear();
      await act(async () => {
        await userEvent.type(cmContent, " @will");
        await new Promise((r) => setTimeout(r, 400));
      });

      const filterCallsAfter = mockInvoke.mock.calls.filter(
        (call) =>
          call[0] === "dispatch_command" &&
          (call[1] as { cmd: string })?.cmd === "perspective.filter",
      );
      expect(filterCallsAfter.length).toBeGreaterThanOrEqual(1);
      const lastAfter = filterCallsAfter[filterCallsAfter.length - 1];
      expect(lastAfter[1]).toMatchObject({
        cmd: "perspective.filter",
        args: { filter: "#bug @will", perspective_id: "p1" },
      });
    });

    it("vim Escape from insert mode keeps typed filter — autosave still lands", async () => {
      // Per the architecture spec (scenario #7): vim insert-mode Escape exits
      // to normal mode and the typed filter still saves. Saving happens via
      // the natural 300ms debounce — the formula bar has no "save in place"
      // commit path because it is always-live. The test waits past the
      // debounce to confirm the typed text was persisted.
      mockKeymapMode = "vim";
      const { container } = render(
        <FilterEditor filter="" perspectiveId="p1" />,
      );
      const view = await getEditorView(container);

      const { getCM } = await import("@replit/codemirror-vim");
      const cm = getCM(view);
      expect(cm).toBeTruthy();
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      if (cm) (cm as any).state.vim.insertMode = true;

      await act(async () => {
        view.dispatch({
          changes: { from: 0, to: view.state.doc.length, insert: "#bug" },
        });
        await new Promise((r) => setTimeout(r, 20));
      });

      mockInvoke.mockClear();

      await act(async () => {
        view.dom.dispatchEvent(
          new KeyboardEvent("keydown", {
            key: "Escape",
            bubbles: true,
            cancelable: true,
          }),
        );
        // Wait past the 300ms autosave debounce so the filter lands.
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
  });
});
