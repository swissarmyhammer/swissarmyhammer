/**
 * Regression test for the user-reported "delete scenario":
 *
 *   1. Type a tag (`#BLOCKED`) — autosave dispatches `#BLOCKED`.
 *   2. Append another tag (` #sis`) — autosave dispatches `#BLOCKED #sis`.
 *   3. Delete the appended tag — autosave dispatches `#BLOCKED`.
 *
 * User report (2026-04-17): "I did #BLOCKED, then #sis, then deleted #sis and
 * it did not get back to #BLOCKED." The saved filter must always match what
 * the user sees in the buffer after each debounce.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { useState } from "react";
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

import { FilterEditor } from "./filter-editor";

async function getView(container: HTMLElement) {
  const cm = container.querySelector(".cm-editor") as HTMLElement;
  expect(cm).toBeTruthy();
  const { EditorView } = await import("@codemirror/view");
  const view = EditorView.findFromDOM(cm);
  expect(view).toBeTruthy();
  return view!;
}

function filterCalls() {
  return mockInvoke.mock.calls.filter(
    (call) =>
      call[0] === "dispatch_command" &&
      (call[1] as { cmd: string })?.cmd === "perspective.filter",
  );
}

function lastFilter(): string | null {
  const calls = filterCalls();
  if (calls.length === 0) return null;
  return (calls[calls.length - 1][1] as { args: { filter: string } }).args
    .filter;
}

/** Parent that wires async dispatch back to the `filter` prop — same
 *  round-trip shape as the real app. */
function RoundTripParent() {
  const [filter, setFilter] = useState("");
  mockInvoke.mockImplementation(
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    async (...args: any[]) => {
      if (
        args[0] === "dispatch_command" &&
        args[1]?.cmd === "perspective.filter"
      ) {
        await new Promise((r) => setTimeout(r, 20));
        setFilter(args[1].args.filter);
      }
      return null;
    },
  );
  return <FilterEditor filter={filter} perspectiveId="p1" />;
}

describe("FilterEditor type → type → delete scenario", () => {
  beforeEach(() => {
    mockKeymapMode = "cua";
    mockInvoke.mockReset();
  });

  it("tag → append tag → delete appended tag: saved filter matches buffer at each step", async () => {
    const result = render(<RoundTripParent />);
    const view = await getView(result.container);
    view.contentDOM.focus();

    // STEP 1 — Type #BLOCKED.
    await act(async () => {
      await userEvent.type(view.contentDOM, "#BLOCKED");
      await new Promise((r) => setTimeout(r, 500));
    });
    expect(view.state.doc.toString()).toBe("#BLOCKED");
    expect(lastFilter()).toBe("#BLOCKED");

    // STEP 2 — Append " #sis".
    await act(async () => {
      await userEvent.type(view.contentDOM, " #sis");
      await new Promise((r) => setTimeout(r, 500));
    });
    expect(view.state.doc.toString()).toBe("#BLOCKED #sis");
    expect(lastFilter()).toBe("#BLOCKED #sis");

    // STEP 3 — Delete " #sis" (5 backspaces).
    await act(async () => {
      for (let i = 0; i < 5; i++) {
        await userEvent.type(view.contentDOM, "{Backspace}");
      }
      await new Promise((r) => setTimeout(r, 500));
    });
    expect(view.state.doc.toString()).toBe("#BLOCKED");

    // CRITICAL INVARIANT — the saved filter must match the buffer. The user
    // report was: "it did not get back to #BLOCKED". This assertion gates
    // against that.
    expect(lastFilter()).toBe("#BLOCKED");
  });

  it("tag → append tag → delete to empty: saved filter clears", async () => {
    const result = render(<RoundTripParent />);
    const view = await getView(result.container);
    view.contentDOM.focus();

    // Seed with #BLOCKED.
    await act(async () => {
      await userEvent.type(view.contentDOM, "#BLOCKED");
      await new Promise((r) => setTimeout(r, 500));
    });
    expect(lastFilter()).toBe("#BLOCKED");

    // Delete all 8 characters.
    await act(async () => {
      for (let i = 0; i < 8; i++) {
        await userEvent.type(view.contentDOM, "{Backspace}");
      }
      await new Promise((r) => setTimeout(r, 500));
    });

    expect(view.state.doc.toString()).toBe("");
    // Deleting to empty must dispatch clearFilter (or equivalent empty save).
    const clearCalls = mockInvoke.mock.calls.filter(
      (call) =>
        call[0] === "dispatch_command" &&
        (call[1] as { cmd: string })?.cmd === "perspective.clearFilter",
    );
    expect(clearCalls.length).toBeGreaterThanOrEqual(1);
  });

  it("type #BL, wait for autosave, pick #BLOCKED from autocomplete → filter saves as #BLOCKED", async () => {
    // User-reported scenario (2026-04-17): "I typed #BL — which triggered
    // the autocomplete, then I waited long enough to autosave and picked
    // #BLOCKED from the autocomplete — the filter did not save."
    const result = render(<RoundTripParent />);
    const view = await getView(result.container);
    view.contentDOM.focus();

    // STEP 1 — type "#BL" and wait for autosave.
    await act(async () => {
      await userEvent.type(view.contentDOM, "#BL");
      await new Promise((r) => setTimeout(r, 500));
    });
    expect(view.state.doc.toString()).toBe("#BL");
    expect(lastFilter()).toBe("#BL");
    const afterPartial = filterCalls().length;

    // STEP 2 — simulate accepting "#BLOCKED" from autocomplete. CM6's
    // built-in autocomplete dispatches a transaction carrying the
    // `pickedCompletion` annotation. This is what the filter-editor's
    // `buildCompletionFlushExtension` listens for to flush the debounce.
    const { pickedCompletion } = await import("@codemirror/autocomplete");
    await act(async () => {
      view.dispatch({
        changes: { from: 0, to: view.state.doc.length, insert: "#BLOCKED" },
        annotations: pickedCompletion.of({
          label: "#BLOCKED",
          apply: "#BLOCKED",
        }),
      });
      // Give the microtask flush + async dispatch round-trip + next
      // debounce a chance to land.
      await new Promise((r) => setTimeout(r, 500));
    });
    expect(view.state.doc.toString()).toBe("#BLOCKED");

    // CRITICAL — the accepted tag must be dispatched.
    expect(filterCalls().length).toBeGreaterThan(afterPartial);
    expect(lastFilter()).toBe("#BLOCKED");
  });

  it("invalid-but-in-progress filter still saves (buffer is always the source of truth)", async () => {
    // User-reported insight (2026-04-17): "it is very easy to be in an invalid
    // state while editing. We must not reject based on validation. I SHOULD
    // be able to save a filter #BL even though that is invalid — but then
    // the perspective should have a filter_error we can use to highlight
    // the error in the editor."
    //
    // Contract:
    //   - Every buffer change flows to the backend. No silent rejection.
    //   - Invalid expressions are dispatched AS-IS.
    //   - The editor reads the parse error locally for visual indication
    //     (red tint via `text-destructive`).
    const result = render(<RoundTripParent />);
    const view = await getView(result.container);
    view.contentDOM.focus();

    // Type a valid filter first — baseline.
    await act(async () => {
      await userEvent.type(view.contentDOM, "#BLOCKED");
      await new Promise((r) => setTimeout(r, 500));
    });
    expect(lastFilter()).toBe("#BLOCKED");
    const afterValid = filterCalls().length;

    // Now type an intermediate invalid state: `#BLOCKED &&` — the parser
    // rejects the trailing operator. The OLD code set an error and did
    // NOT dispatch. The NEW behavior: dispatch regardless, set the error
    // for visual indication only.
    await act(async () => {
      await userEvent.type(view.contentDOM, " &&");
      await new Promise((r) => setTimeout(r, 500));
    });
    expect(view.state.doc.toString()).toBe("#BLOCKED &&");

    // CRITICAL — the buffer and the saved filter must match.
    expect(filterCalls().length).toBeGreaterThan(afterValid);
    expect(lastFilter()).toBe("#BLOCKED &&");

    // AND — the editor should be visually flagged via `text-destructive`
    // so the user can see the filter is malformed.
    const editorEl = result.container.querySelector(
      '[data-testid="filter-editor"]',
    );
    expect(editorEl?.className).toContain("text-destructive");
  });

  it("every dispatched filter command carries perspective:{id} in its scopeChain", async () => {
    // Real-app log evidence (2026-04-17 06:42:03): when `handleFlush` fires
    // from an autocomplete accept after a parent re-render has dropped focus,
    // the dispatch's scopeChain was `["board", "store", "mode", "window",
    // "engine"]` — missing `perspective:{id}`. The backend returned
    // "command not available in current context" and the save was lost.
    //
    // Contract: the FilterEditor must be wrapped in its own CommandScopeProvider
    // with moniker `perspective:{id}` so every dispatch carries that scope in
    // its tree scope, regardless of focus state.
    const result = render(<RoundTripParent />);
    const view = await getView(result.container);
    view.contentDOM.focus();

    await act(async () => {
      await userEvent.type(view.contentDOM, "#BLOCKED");
      await new Promise((r) => setTimeout(r, 500));
    });

    const filterInvokes = mockInvoke.mock.calls.filter(
      (call) =>
        call[0] === "dispatch_command" &&
        (call[1] as { cmd: string })?.cmd === "perspective.filter",
    );
    expect(filterInvokes.length).toBeGreaterThanOrEqual(1);

    for (const invokeCall of filterInvokes) {
      const payload = invokeCall[1] as {
        scopeChain?: string[];
      };
      expect(payload.scopeChain).toBeDefined();
      expect(payload.scopeChain).toContain("perspective:p1");
    }
  });

  it("clicking the × button visually empties the editor buffer", async () => {
    // User-reported (2026-04-17): "'clear' does not visually clear the
    // filter though it does actually save." The × dispatches clearFilter
    // successfully, but the CM6 buffer still displays the old text until
    // the user switches perspectives.
    const result = render(<RoundTripParent />);
    const view = await getView(result.container);
    view.contentDOM.focus();

    await act(async () => {
      await userEvent.type(view.contentDOM, "#BLOCKED");
      await new Promise((r) => setTimeout(r, 500));
    });
    expect(view.state.doc.toString()).toBe("#BLOCKED");

    // Click the × clear button.
    const clearBtn = result.container.querySelector(
      '[aria-label="Clear filter"]',
    );
    expect(clearBtn).toBeTruthy();
    await act(async () => {
      (clearBtn as HTMLElement).click();
      await new Promise((r) => setTimeout(r, 100));
    });

    // CRITICAL — the buffer must be empty. The user must see the filter
    // cleared immediately, not after a perspective switch.
    expect(view.state.doc.toString()).toBe("");
  });

  it("progressively typing and deleting dispatches for every final state", async () => {
    // This stresses the same invariant at finer granularity: the final
    // dispatched value must always match the final buffer content after each
    // debounce window.
    const result = render(<RoundTripParent />);
    const view = await getView(result.container);
    view.contentDOM.focus();

    const steps = [
      { action: "type", text: "#a" },
      { action: "type", text: " #b" },
      { action: "type", text: " #c" },
      { action: "backspace", count: 3 }, // delete " #c"
      { action: "backspace", count: 3 }, // delete " #b"
    ];

    const expectedBuffers = ["#a", "#a #b", "#a #b #c", "#a #b", "#a"];

    for (let i = 0; i < steps.length; i++) {
      const step = steps[i];
      await act(async () => {
        if (step.action === "type") {
          await userEvent.type(view.contentDOM, step.text!);
        } else {
          for (let j = 0; j < (step.count ?? 0); j++) {
            await userEvent.type(view.contentDOM, "{Backspace}");
          }
        }
        await new Promise((r) => setTimeout(r, 500));
      });
      expect(view.state.doc.toString()).toBe(expectedBuffers[i]);
      expect(lastFilter()).toBe(expectedBuffers[i]);
    }
  });
});
