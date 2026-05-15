/**
 * Scenario matrix for FilterEditor — 10 acceptance scenarios × 3 keymap
 * modes = 30 test cases.
 *
 * These enforce the formula-bar contract pinned by the user on 2026-04-17:
 *
 *   "The filter bar is an always-open, always-live formula input. It has NO
 *   popover, NO dialog, NO draft state, NO commit-and-close semantics, NO
 *   cancel-discards-draft semantics. Typing always saves (debounced). Enter
 *   is a shortcut for flush now. Nothing else about the editor should change
 *   on Enter."
 *
 * The scenarios that previously regressed in the real browser (notably
 * scenario #4: type → Enter → type-more → verify save) are the primary
 * regression target. These tests drive the CM6 input pipeline through real
 * user events / view.dispatch and assert on Tauri `dispatch_command` calls.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { useState } from "react";
import { render, fireEvent, act, screen } from "@testing-library/react";
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

let mockKeymapMode = "cua";
vi.mock("@/lib/ui-state-context", () => ({
  useUIState: () => ({ keymap_mode: mockKeymapMode }),
}));

vi.mock("@/lib/schema-context", () => ({
  useSchema: () => ({ mentionableTypes: [] }),
}));

vi.mock("@/lib/entity-store-context", () => ({
  useEntityStore: () => ({ getEntities: () => [] }),
}));

vi.mock("@/components/window-container", () => ({
  useBoardData: () => ({ virtualTagMeta: [] }),
}));

import { FilterEditor } from "./filter-editor";

const MODES = ["cua", "emacs", "vim"] as const;
type Mode = (typeof MODES)[number];

/** Get the CM6 EditorView from the rendered filter editor. */
async function getEditorView(container: HTMLElement) {
  const cmEditor = container.querySelector(".cm-editor") as HTMLElement;
  expect(cmEditor).toBeTruthy();
  const { EditorView } = await import("@codemirror/view");
  const view = EditorView.findFromDOM(cmEditor);
  expect(view).toBeTruthy();
  return view!;
}

/** Type text into the CM6 editor via a real user-event. */
async function type(view: import("@codemirror/view").EditorView, text: string) {
  view.contentDOM.focus();
  await userEvent.type(view.contentDOM, text);
}

/** Dispatch a real keyboard event on the CM6 contentDOM (vim-safe path). */
async function key(
  view: import("@codemirror/view").EditorView,
  keyName: string,
) {
  await act(async () => {
    view.contentDOM.dispatchEvent(
      new KeyboardEvent("keydown", {
        key: keyName,
        bubbles: true,
        cancelable: true,
      }),
    );
  });
}

/** Return all perspective.filter dispatch calls recorded by the mock. */
function filterCalls(): Array<
  [string, { cmd: string; args: { filter: string } }]
> {
  return mockInvoke.mock.calls.filter(
    (call) =>
      call[0] === "dispatch_command" &&
      (call[1] as { cmd: string })?.cmd === "perspective.filter",
  ) as Array<[string, { cmd: string; args: { filter: string } }]>;
}

/** Return all perspective.clearFilter dispatch calls recorded by the mock. */
function clearFilterCalls() {
  return mockInvoke.mock.calls.filter(
    (call) =>
      call[0] === "dispatch_command" &&
      (call[1] as { cmd: string })?.cmd === "perspective.clearFilter",
  );
}

/**
 * Put vim into insert mode by directly flipping the internal vim state —
 * matches the proven pattern used in cm-submit-cancel.test.ts.
 */
async function enterVimInsertMode(view: import("@codemirror/view").EditorView) {
  const { getCM } = await import("@replit/codemirror-vim");
  const cm = getCM(view);
  expect(cm).toBeTruthy();
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  if (cm) (cm as any).state.vim.insertMode = true;
}

// ---------------------------------------------------------------------------
// Test matrix — run all 10 scenarios in each keymap mode.
// ---------------------------------------------------------------------------

describe.each(MODES)("FilterEditor scenarios (mode=%s)", (mode: Mode) => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockKeymapMode = mode;
  });

  /** Render the editor and — in vim mode — immediately enter insert mode so
   * typing takes effect. CUA/emacs start in insert-equivalent by default. */
  async function renderEditor(initialFilter = "") {
    const result = render(
      <FilterEditor filter={initialFilter} perspectiveId="p1" />,
    );
    const view = await getEditorView(result.container);
    if (mode === "vim") await enterVimInsertMode(view);
    return { ...result, view };
  }

  // -------------------------------------------------------------------------
  // Scenario 1 — Type-and-wait: debounced save at 300ms.
  // -------------------------------------------------------------------------

  it("1. type-and-wait → debounced save fires", async () => {
    const { view } = await renderEditor("");
    await act(async () => {
      await type(view, "#bug");
      await new Promise((r) => setTimeout(r, 400));
    });
    const calls = filterCalls();
    expect(calls.length).toBeGreaterThanOrEqual(1);
    expect(calls[calls.length - 1][1]).toMatchObject({
      cmd: "perspective.filter",
      args: { filter: "#bug", perspective_id: "p1" },
    });
  });

  // -------------------------------------------------------------------------
  // Scenario 2 — Type-and-keep-typing: final value saved.
  // -------------------------------------------------------------------------

  it("2. type-and-keep-typing → final value saved", async () => {
    const { view } = await renderEditor("");
    await act(async () => {
      await type(view, "#bug");
      await new Promise((r) => setTimeout(r, 200));
      await type(view, " @alice");
      await new Promise((r) => setTimeout(r, 400));
    });
    const calls = filterCalls();
    expect(calls.length).toBeGreaterThanOrEqual(1);
    expect(calls[calls.length - 1][1]).toMatchObject({
      cmd: "perspective.filter",
      args: { filter: "#bug @alice" },
    });
  });

  // -------------------------------------------------------------------------
  // Scenario 3 — Type-and-Enter (no autocomplete): synchronous flush, editor
  //              stays focused and mounted.
  // -------------------------------------------------------------------------

  it("3. type-and-Enter (no autocomplete) → immediate flush, editor stays", async () => {
    const { container, view } = await renderEditor("");

    // Seed via view.dispatch to guarantee the change listener fires and
    // schedules the debounce. userEvent.type can be flaky here because
    // CM6's input pipeline doesn't always produce docChanged transactions
    // for synthetic key events in all keymap modes.
    await act(async () => {
      view.dispatch({
        changes: { from: 0, to: view.state.doc.length, insert: "#bug" },
      });
      await new Promise((r) => setTimeout(r, 20));
    });
    mockInvoke.mockClear();

    // Press Enter via a native KeyboardEvent on the contentDOM — matches the
    // pattern used in the vim keymap capture listener and the cua keymap
    // run() path.
    await act(async () => {
      view.contentDOM.dispatchEvent(
        new KeyboardEvent("keydown", {
          key: "Enter",
          bubbles: true,
          cancelable: true,
        }),
      );
      await new Promise((r) => setTimeout(r, 50));
    });

    // Dispatched synchronously (well under the 300ms debounce threshold).
    const calls = filterCalls();
    expect(calls.length).toBeGreaterThanOrEqual(1);
    expect(calls[calls.length - 1][1]).toMatchObject({
      args: { filter: "#bug" },
    });
    // Editor remains mounted.
    expect(container.querySelectorAll(".cm-editor").length).toBe(1);
  });

  // -------------------------------------------------------------------------
  // Scenario 4 — THE FAILING SCENARIO from the task description.
  // After Enter, subsequent typing must still feed the autosave pipeline.
  // -------------------------------------------------------------------------

  it("4. type-Enter-type-more → subsequent autosave fires with final text", async () => {
    const { view } = await renderEditor("");

    // Type, then Enter.
    await act(async () => {
      await type(view, "#bug");
      await new Promise((r) => setTimeout(r, 20));
    });
    await key(view, "Enter");
    await new Promise((r) => setTimeout(r, 50));

    // After Enter, clear mock and type more.
    mockInvoke.mockClear();
    await act(async () => {
      await type(view, " @alice");
      await new Promise((r) => setTimeout(r, 400));
    });

    // The new typing must reach the autosave — editor is NOT disconnected.
    const calls = filterCalls();
    expect(calls.length).toBeGreaterThanOrEqual(1);
    expect(calls[calls.length - 1][1]).toMatchObject({
      args: { filter: "#bug @alice" },
    });
  });

  // Scenarios 5 and 6 (autocomplete accept via Enter / Tab / click) are
  // covered by the dedicated filter-editor.test.tsx which mocks the mention
  // source. Not duplicated here.

  // -------------------------------------------------------------------------
  // Scenario 7 — Escape: cua/emacs stays focused with filter saved; vim
  //              insert exits to normal with filter saved; vim normal stays.
  // -------------------------------------------------------------------------

  it("7. Escape → filter still saves, editor stays mounted", async () => {
    const { container, view } = await renderEditor("");
    await act(async () => {
      await type(view, "#bug");
      await new Promise((r) => setTimeout(r, 20));
    });
    await key(view, "Escape");
    await new Promise((r) => setTimeout(r, 400));

    // No clearFilter should have been dispatched.
    expect(clearFilterCalls()).toHaveLength(0);

    // Editor stays mounted.
    expect(container.querySelectorAll(".cm-editor").length).toBe(1);

    // Filter saved (via either flush-on-Enter-not-triggered + natural
    // debounce, or via Enter flush — depending on mode).
    const calls = filterCalls();
    expect(calls.length).toBeGreaterThanOrEqual(1);
    expect(calls[calls.length - 1][1]).toMatchObject({
      args: { filter: "#bug" },
    });
  });

  // -------------------------------------------------------------------------
  // Scenario 8 — Clear button dispatches clearFilter, pending debounces are
  //              dropped, editor stays mounted.
  // -------------------------------------------------------------------------

  it("8. clear × button → clearFilter dispatched, editor stays", async () => {
    const { container } = await renderEditor("#bug");
    mockInvoke.mockClear();
    fireEvent.click(screen.getByLabelText("Clear filter"));
    await new Promise((r) => setTimeout(r, 50));
    expect(clearFilterCalls().length).toBeGreaterThanOrEqual(1);
    expect(container.querySelectorAll(".cm-editor").length).toBe(1);
  });

  // -------------------------------------------------------------------------
  // Scenario 9 — Blur (click outside) → debounce still fires.
  // -------------------------------------------------------------------------

  it("9. blur before debounce → debounce still fires", async () => {
    const { view } = await renderEditor("");
    await act(async () => {
      await type(view, "#bug");
      await new Promise((r) => setTimeout(r, 20));
      view.contentDOM.blur();
      await new Promise((r) => setTimeout(r, 400));
    });
    const calls = filterCalls();
    expect(calls.length).toBeGreaterThanOrEqual(1);
    expect(calls[calls.length - 1][1]).toMatchObject({
      args: { filter: "#bug" },
    });
  });

  // -------------------------------------------------------------------------
  // Scenario 10 — Perspective switch before debounce fires: unmount-flush
  //               saves the pending value.
  // -------------------------------------------------------------------------

  it("10. unmount before debounce → pending save flushes", async () => {
    const { view, unmount } = await renderEditor("");
    await act(async () => {
      await type(view, "#bug");
      await new Promise((r) => setTimeout(r, 20));
    });
    mockInvoke.mockClear();
    await act(async () => {
      unmount();
      await new Promise((r) => setTimeout(r, 20));
    });
    const calls = filterCalls();
    expect(calls.length).toBeGreaterThanOrEqual(1);
    expect(calls[calls.length - 1][1]).toMatchObject({
      args: { filter: "#bug" },
    });
  });

  // -------------------------------------------------------------------------
  // Scenario 11 — The real-world regression: async dispatch + parent prop
  //               update + continued typing. Matches what happens in the
  //               running Tauri app: dispatch is async, backend fires an
  //               event, parent re-renders with a new filter prop, user is
  //               already typing. The editor must stay live.
  // -------------------------------------------------------------------------

  it("11. async dispatch + parent prop update does not disconnect the editor", async () => {
    // Build a parent wrapper that mimics the production flow: async dispatch
    // resolves after a delay, then the parent's filter prop updates to match
    // the dispatched value (as if a backend entity-field-changed event fired
    // and refreshed the perspective).
    function Parent() {
      const [filter, setFilter] = useState("");
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      mockInvoke.mockImplementation(async (...args: any[]) => {
        if (
          args[0] === "dispatch_command" &&
          args[1]?.cmd === "perspective.filter"
        ) {
          // Simulate the async IPC round-trip.
          await new Promise((r) => setTimeout(r, 30));
          // Simulate the backend event → parent prop update.
          setFilter(args[1].args.filter);
        }
        return null;
      });
      return <FilterEditor filter={filter} perspectiveId="p1" />;
    }

    const result = render(<Parent />);
    const view = await getEditorView(result.container);
    if (mode === "vim") await enterVimInsertMode(view);

    // First edit: type, wait for debounce + round-trip + prop update.
    await act(async () => {
      await type(view, "#bug");
      await new Promise((r) => setTimeout(r, 500));
    });
    const afterFirst = filterCalls();
    expect(afterFirst.length).toBeGreaterThanOrEqual(1);
    expect(afterFirst[afterFirst.length - 1][1]).toMatchObject({
      args: { filter: "#bug" },
    });

    // Second edit — THIS is what used to break. Typing continues; the editor
    // must still emit changes; the debounce must still fire; and the final
    // value must be dispatched with the appended text.
    await act(async () => {
      await type(view, " @alice");
      await new Promise((r) => setTimeout(r, 500));
    });
    const afterSecond = filterCalls();
    expect(afterSecond.length).toBeGreaterThan(afterFirst.length);
    const last = afterSecond[afterSecond.length - 1][1];
    expect(last.args.filter).toBe("#bug @alice");
  });
});
