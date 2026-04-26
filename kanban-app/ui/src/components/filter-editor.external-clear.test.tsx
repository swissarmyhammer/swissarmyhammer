/**
 * Regression tests for the "external clearFilter" bug:
 *
 *   When `perspective.clearFilter` (or any external `perspective.filter` set)
 *   is dispatched from outside the formula-bar editor — context menu, command
 *   palette, keybinding, another window — the backend clears the filter and
 *   the parent re-renders with `filter=""`. The CM6 buffer must follow.
 *
 * Prior to the fix, `TextEditorProps.value` was captured only at mount and
 * `FilterEditorBody` never imperatively reset the CM6 buffer in response to a
 * prop change (only the inline × button did). Result: the stale filter text
 * stayed on screen indefinitely even though the backend state was cleared.
 *
 * The fix adds a prop-to-buffer reconciliation effect in `FilterEditorBody`
 * guarded by `lastDispatchedRef` (to ignore echoes of our own dispatches) and
 * by a buffer-equality check (to avoid clobbering keystrokes in flight).
 *
 * These tests drive the three failure modes:
 *
 *   1. External clearFilter (filter: "#bug" → "") resets the buffer.
 *   2. External filter set (filter: "#bug" → "@alice") updates the buffer.
 *   3. The reconciliation does NOT clobber the user's own debounced save
 *      when the backend echo arrives with the same filter the user typed.
 */

import React from "react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, act } from "@testing-library/react";
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

/** Locate the CM6 EditorView mounted by the FilterEditor under test. */
async function getView(container: HTMLElement) {
  const cm = container.querySelector(".cm-editor") as HTMLElement;
  expect(cm).toBeTruthy();
  const { EditorView } = await import("@codemirror/view");
  const view = EditorView.findFromDOM(cm);
  expect(view).toBeTruthy();
  return view!;
}

/**
 * Controlled parent that holds the `filter` prop in React state so the test
 * can simulate an external mutation by calling `setFilter(...)` directly.
 *
 * Exposes a ref-shaped API via a test hook so the test can drive the external
 * update without going through a backend round-trip.
 */
function ControlledParent({
  initialFilter,
  controllerRef,
}: {
  initialFilter: string;
  controllerRef: React.MutableRefObject<
    ((next: string | undefined) => void) | null
  >;
}) {
  const [filter, setFilter] = React.useState<string | undefined>(initialFilter);
  controllerRef.current = setFilter;
  return <FilterEditor filter={filter ?? ""} perspectiveId="p1" />;
}

describe("FilterEditor — external filter-prop reconciliation", () => {
  beforeEach(() => {
    mockKeymapMode = "cua";
    mockInvoke.mockReset();
    mockInvoke.mockImplementation(() => Promise.resolve(null));
  });

  it("external clearFilter: prop transitions from '#bug' to '' → CM6 buffer resets to empty", async () => {
    // Simulates the context-menu / command-palette path. The editor mounts
    // with `filter="#bug"`, the buffer shows `#bug`, then the parent pushes
    // `filter=""` (as would happen after an external `perspective.clearFilter`
    // refetch). The buffer must reflect the new prop.
    const controllerRef: React.MutableRefObject<
      ((next: string | undefined) => void) | null
    > = { current: null };
    const { container } = render(
      <ControlledParent initialFilter="#bug" controllerRef={controllerRef} />,
    );
    const view = await getView(container);
    expect(view.state.doc.toString()).toBe("#bug");

    // External clearFilter → parent refetches → filter prop becomes empty.
    await act(async () => {
      controllerRef.current?.("");
      // Flush reconciliation effect + the setValue-triggered onChange.
      await new Promise((r) => setTimeout(r, 20));
    });

    expect(view.state.doc.toString()).toBe("");
  });

  it("external filter set: prop transitions from '#bug' to '@alice' → CM6 buffer updates", async () => {
    // Simulates undo of clearFilter, or a filter set dispatched from another
    // window. The prop changes to a new non-empty value; the buffer follows.
    const controllerRef: React.MutableRefObject<
      ((next: string | undefined) => void) | null
    > = { current: null };
    const { container } = render(
      <ControlledParent initialFilter="#bug" controllerRef={controllerRef} />,
    );
    const view = await getView(container);
    expect(view.state.doc.toString()).toBe("#bug");

    await act(async () => {
      controllerRef.current?.("@alice");
      await new Promise((r) => setTimeout(r, 20));
    });

    expect(view.state.doc.toString()).toBe("@alice");
  });

  it("external undefined: prop transitions from '#bug' to undefined → CM6 buffer resets to empty", async () => {
    // `perspective.filter` is optional; clearFilter drops the field entirely,
    // so the refetched perspective has `filter: undefined`. `FilterFormulaBar`
    // coerces undefined → "" before handing it to `FilterEditor`, but cover
    // the coerced-empty path end-to-end through the controlled parent anyway.
    const controllerRef: React.MutableRefObject<
      ((next: string | undefined) => void) | null
    > = { current: null };
    const { container } = render(
      <ControlledParent initialFilter="#bug" controllerRef={controllerRef} />,
    );
    const view = await getView(container);
    expect(view.state.doc.toString()).toBe("#bug");

    await act(async () => {
      controllerRef.current?.(undefined);
      await new Promise((r) => setTimeout(r, 20));
    });

    expect(view.state.doc.toString()).toBe("");
  });

  it("reconciliation does not clobber user typing: echoed backend refresh with same value is a no-op", async () => {
    // Scenario:
    //   1. User types "abc" → 300 ms debounce → applyFilter("abc") dispatches
    //      perspective.filter; the ref is stamped to "abc".
    //   2. The parent echoes the dispatched value back as the `filter` prop —
    //      the normal backend refresh round-trip.
    //   3. The reconciliation effect sees filter === lastDispatchedRef and
    //      MUST NOT reset the buffer. Any reset here would cause a visible
    //      flicker and could clobber a character typed between the dispatch
    //      and the echo.
    //
    // The invariant: after the echo, the buffer still reads "abc" and no
    // subsequent `setValue("abc")` fires (we can't observe that directly, but
    // if it did fire, the onChange would schedule a second apply — so we
    // assert `perspective.filter` was only dispatched once for "abc").
    function RoundTripParent() {
      const [filter, setFilter] = React.useState("");
      React.useEffect(() => {
        const prev = mockInvoke.getMockImplementation();
        mockInvoke.mockImplementation((cmd: string, args?: unknown) => {
          if (cmd === "dispatch_command") {
            const payload = args as {
              cmd?: string;
              args?: { filter?: string };
            };
            if (payload?.cmd === "perspective.filter") {
              // Mirror the real backend: echo the saved filter back via
              // setState in a microtask (entity-field-changed event).
              queueMicrotask(() => setFilter(payload.args?.filter ?? ""));
            }
            if (payload?.cmd === "perspective.clearFilter") {
              queueMicrotask(() => setFilter(""));
            }
          }
          return prev ? prev(cmd, args) : Promise.resolve(null);
        });
        return () => {
          if (prev) mockInvoke.mockImplementation(prev);
        };
      }, []);
      return <FilterEditor filter={filter} perspectiveId="p1" />;
    }

    const { container } = render(<RoundTripParent />);
    const view = await getView(container);
    view.contentDOM.focus();

    mockInvoke.mockClear();

    // Type "abc" via real keystrokes and wait past the 300ms debounce.
    await act(async () => {
      await userEvent.type(view.contentDOM, "abc");
      // Debounce + microtask echo + reconciliation effect + potential
      // retrigger — 500ms gives everything time to settle.
      await new Promise((r) => setTimeout(r, 500));
    });

    // Buffer must hold "abc" — no flicker, no reset.
    expect(view.state.doc.toString()).toBe("abc");

    // perspective.filter must have been dispatched for "abc", and it must
    // NOT have been re-dispatched by the reconciliation echo. The round-
    // trip parent echoes `filter="abc"`; the effect must recognise this as
    // its own echo (lastDispatchedRef === "abc") and no-op.
    const abcCalls = mockInvoke.mock.calls.filter(
      (call) =>
        call[0] === "dispatch_command" &&
        (call[1] as { cmd: string })?.cmd === "perspective.filter" &&
        (call[1] as { args: { filter: string } })?.args?.filter === "abc",
    );
    expect(abcCalls).toHaveLength(1);
  });

  it("inline × clear still works after reconciliation is in place", async () => {
    // Regression guard for the existing × clear button path
    // (handleClearAndReset). With the new reconciliation effect in place, the
    // backend's echo of filter="" must be recognised as our own dispatch (the
    // ref is stamped to "" inside handleClear) and NOT cause the effect to
    // fire a redundant setValue/dispatch.
    const controllerRef: React.MutableRefObject<
      ((next: string | undefined) => void) | null
    > = { current: null };
    const { container } = render(
      <ControlledParent initialFilter="#bug" controllerRef={controllerRef} />,
    );
    const view = await getView(container);
    expect(view.state.doc.toString()).toBe("#bug");

    // Click the × button.
    const clearBtn = container.querySelector<HTMLButtonElement>(
      '[aria-label="Clear filter"]',
    );
    expect(clearBtn).toBeTruthy();

    mockInvoke.mockClear();

    await act(async () => {
      clearBtn!.click();
      // Wait past the 300ms autosave debounce so the setValue-driven
      // handleChange has fully settled. After this point, lastDispatchedRef
      // is stamped to "" and the buffer is "".
      await new Promise((r) => setTimeout(r, 500));
    });

    expect(view.state.doc.toString()).toBe("");

    // EXACTLY one clearFilter dispatch fired — the immediate handleClear
    // path. The debounce-driven echo from handleClearAndReset's
    // `setValue("")` must NOT round-trip as a second dispatch: handleClear
    // stamps `lastDispatchedRef.current = ""` BEFORE setValue runs, so
    // handleChange's ref-match guard suppresses the redundant schedule.
    //
    // This tightening (>=1 to ===1) locks in the fix for the undo-stack
    // duplication bug — any regression that re-introduces the double
    // dispatch will flip this assertion.
    const clearCalls = mockInvoke.mock.calls.filter(
      (call) =>
        call[0] === "dispatch_command" &&
        (call[1] as { cmd: string })?.cmd === "perspective.clearFilter",
    );
    expect(clearCalls.length).toBe(1);

    // Now the backend pushes filter="" back through the controlled parent,
    // simulating the entity-field-changed refetch. The reconciliation effect
    // must see lastDispatchedRef === "" (stamped by handleClear + applyFilter)
    // AND buffer === "" and no-op — no additional dispatch.
    const beforeEcho = mockInvoke.mock.calls.length;
    await act(async () => {
      controllerRef.current?.("");
      await new Promise((r) => setTimeout(r, 400));
    });
    expect(mockInvoke.mock.calls.length).toBe(beforeEcho);
  });
});
