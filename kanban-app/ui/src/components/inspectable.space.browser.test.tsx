/**
 * Browser-mode tests for the Space-key inspect dispatch contract owned by
 * the `<Inspectable>` wrapper.
 *
 * Companion to `inspectable.spatial.test.tsx`, which pins the dblclick
 * dispatch path. After moving inspect ownership off the BoardView's
 * `board.inspect` and onto Inspectable itself (card 01KQ9XJ4XGKVW24EZSQCA6K3E2),
 * Space on a focused inspectable dispatches `ui.inspect` from the
 * Inspectable's scope-level command — independent of which layer the
 * entity lives in (board, inspector, palette result list, etc).
 *
 * The tests below pin:
 *
 *   1. Space on a focused descendant inside a single `<Inspectable>` fires
 *      `ui.inspect` against the wrapper's moniker.
 *   2. Nested `<Inspectable>`s — the closest enclosing one wins (its scope
 *      shadows the outer one because it is closer in the scope chain).
 *   3. Space on an `<input>` inside an `<Inspectable>` does NOT dispatch
 *      `ui.inspect` (the editable surface owns Space; it inserts a literal
 *      space character). Asserted via the global keybinding handler's
 *      `isEditableTarget` gate.
 *   4. Same exclusion for `[contenteditable]`.
 *   5. Regression guard — dblclick on an `<Inspectable>` still dispatches
 *      `ui.inspect` after the Space owner moves into the wrapper.
 *
 * Mock pattern matches `inspectable.spatial.test.tsx` so the two files
 * stay in sync as the Inspectable contract evolves.
 */

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, fireEvent, act } from "@testing-library/react";

// ---------------------------------------------------------------------------
// Tauri API mocks — must come before component imports.
//
// The default `invoke` stub returns a populated `get_ui_state` payload
// so `<AppShell>`'s `useAppShellUIState` hook can read
// `uiState.windows?.[label]` without a null-deref. Tests must keep that
// branch in any custom impl they install — override the rest, defer to
// this default for `get_ui_state`.
// ---------------------------------------------------------------------------

type ListenCallback = (event: { payload: unknown }) => void;

const { mockInvoke, mockListen, listeners } = vi.hoisted(() => {
  const listeners = new Map<string, ListenCallback[]>();
  const mockInvoke = vi.fn(
    async (_cmd: string, _args?: unknown): Promise<unknown> => undefined,
  );
  const mockListen = vi.fn(
    (eventName: string, cb: ListenCallback): Promise<() => void> => {
      const cbs = listeners.get(eventName) ?? [];
      cbs.push(cb);
      listeners.set(eventName, cbs);
      return Promise.resolve(() => {
        const arr = listeners.get(eventName);
        if (arr) {
          const idx = arr.indexOf(cb);
          if (idx >= 0) arr.splice(idx, 1);
        }
      });
    },
  );
  return { mockInvoke, mockListen, listeners };
});

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...a: unknown[]) => mockInvoke(...(a as [string, unknown?])),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: (...a: Parameters<typeof mockListen>) => mockListen(...a),
  emit: vi.fn(() => Promise.resolve()),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    label: "main",
    listen: vi.fn(() => Promise.resolve(() => {})),
  }),
}));

vi.mock("@tauri-apps/plugin-log", () => ({
  error: vi.fn(),
  warn: vi.fn(),
  info: vi.fn(),
  debug: vi.fn(),
  trace: vi.fn(),
  attachConsole: vi.fn(() => Promise.resolve()),
}));

// ---------------------------------------------------------------------------
// Imports — after mocks
// ---------------------------------------------------------------------------

import { Inspectable } from "./inspectable";
import { FocusScope } from "./focus-scope";
import { AppShell } from "./app-shell";
import { FocusLayer } from "./focus-layer";
import { UIStateProvider } from "@/lib/ui-state-context";
import { AppModeProvider } from "@/lib/app-mode-context";
import { UndoProvider } from "@/lib/undo-context";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import {
  EntityFocusProvider,
  useEntityFocus,
} from "@/lib/entity-focus-context";
import { asFq, asSegment, type FullyQualifiedMoniker } from "@/types/spatial";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Wait for register effects scheduled in `useEffect` to flush. */
async function flushSetup() {
  await act(async () => {
    await Promise.resolve();
  });
  await act(async () => {
    await Promise.resolve();
  });
}

/**
 * Tracks the moniker → FullyQualifiedMoniker mapping that the kernel would normally
 * maintain. Card `01KQD0WK54G0FRD7SZVZASA9ST` made the entity-focus
 * store a pure projection of kernel events; tests that mock `invoke`
 * without a kernel simulator need this minimal stub so click-driven
 * `setFocus` still updates the React store via the spatial-focus
 * bridge.
 */
const monikerToKey = new Map<string, string>();
const currentFocusKey: { key: string | null } = { key: null };

/**
 * Default `invoke` implementation covering the IPCs the provider stack
 * fires on mount. The `get_ui_state` branch keeps `<AppShell>` from
 * tripping on a null-deref of `uiState.windows`.
 */
async function defaultInvokeImpl(
  cmd: string,
  args?: unknown,
): Promise<unknown> {
  if (cmd === "get_ui_state") {
    return {
      palette_open: false,
      palette_mode: "command",
      keymap_mode: "cua",
      scope_chain: [],
      open_boards: [],
      windows: {},
      recent_boards: [],
    };
  }
  if (cmd === "spatial_register_scope" || cmd === "spatial_register_scope") {
    const a = (args ?? {}) as { fq?: string; segment?: string };
    if (a.fq && a.segment) monikerToKey.set(a.segment, a.fq);
    return undefined;
  }
  if (cmd === "spatial_unregister_scope") {
    const a = (args ?? {}) as { fq?: string };
    if (a.fq) {
      for (const [m, k] of monikerToKey.entries()) {
        if (k === a.fq) {
          monikerToKey.delete(m);
          break;
        }
      }
    }
    return undefined;
  }
  if (cmd === "spatial_drill_in" || cmd === "spatial_drill_out") {
    const a = (args ?? {}) as { focusedFq?: string };
    return a.focusedFq ?? null;
  }
  if (cmd === "spatial_focus") {
    // Synthesize the kernel's focus-changed emit so the entity-focus
    // bridge writes the React store. Mirrors the real kernel behavior:
    // resolve moniker → key, advance focus_by_window, emit
    // focus-changed with both fields. See card
    // `01KQD0WK54G0FRD7SZVZASA9ST` for the projection invariant.
    //
    // Queued via `queueMicrotask` to match the kernel simulator's
    // timing contract — production events arrive asynchronously, so
    // emitting synchronously would hide regressions that depend on
    // the async write semantics.
    const a = (args ?? {}) as { fq?: string };
    const fq = a.fq ?? null;
    let moniker: string | null = null;
    for (const [s, k] of monikerToKey.entries()) {
      if (k === fq) {
        moniker = s;
        break;
      }
    }

    if (fq) {
      const prev = currentFocusKey.key;
      currentFocusKey.key = fq;
      queueMicrotask(() => {
        const handlers = listeners.get("focus-changed") ?? [];
        for (const h of handlers) {
          h({
            payload: {
              window_label: "main",
              prev_fq: prev,
              next_fq: fq,
              next_segment: moniker,
            },
          });
        }
      });
    }
    return undefined;
  }
  if (cmd === "spatial_clear_focus") {
    // Explicit-clear counterpart — kernel emits a
    // `Some(prev) → None` `focus-changed` event so the React-side
    // bridge flips the store back to `null`. Idempotent when the
    // window had no prior focus.
    const prev = currentFocusKey.key;
    if (prev === null) return undefined;
    currentFocusKey.key = null;
    queueMicrotask(() => {
      const handlers = listeners.get("focus-changed") ?? [];
      for (const h of handlers) {
        h({
          payload: {
            window_label: "main",
            prev_fq: prev,
            next_fq: null,
            next_segment: null,
          },
        });
      }
    });
    return undefined;
  }
  if (cmd === "list_entity_types") return [];
  if (cmd === "get_entity_schema") return null;
  if (cmd === "list_commands_for_scope") return [];
  if (cmd === "dispatch_command") return undefined;
  return undefined;
}

/**
 * Render `ui` inside the production-shaped provider stack with
 * `<AppShell>` so the global keydown listener is mounted. The Space key
 * binding from the Inspectable's scope-level command is only consulted
 * by the global handler created inside `<KeybindingHandler>`, so the
 * shell is required for these tests.
 *
 * Mirrors the `renderShell` helper in `app-shell.test.tsx`.
 */
function withAppShell(ui: React.ReactElement): React.ReactElement {
  return (
    <SpatialFocusProvider>
      <FocusLayer name={asSegment("window")}>
        <EntityFocusProvider>
          <UIStateProvider>
            <AppModeProvider>
              <UndoProvider>
                <AppShell>{ui}</AppShell>
              </UndoProvider>
            </AppModeProvider>
          </UIStateProvider>
        </EntityFocusProvider>
      </FocusLayer>
    </SpatialFocusProvider>
  );
}

/** Collect every `dispatch_command` call's args, in order. */
function dispatchCommandCalls(): Array<Record<string, unknown>> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "dispatch_command")
    .map((c) => c[1] as Record<string, unknown>);
}

/** Filter `dispatch_command` calls down to those for `ui.inspect`. */
function inspectDispatches(): Array<Record<string, unknown>> {
  return dispatchCommandCalls().filter((c) => c.cmd === "ui.inspect");
}

/**
 * Test helper: a focusable card that wires its inner button to a setFocus
 * call so Space can be tested with a moniker actually selected in the
 * entity-focus store.
 */
function FocusButton({ moniker }: { moniker: FullyQualifiedMoniker }) {
  const { setFocus } = useEntityFocus();
  return (
    <button type="button" onClick={() => setFocus(moniker)}>
      Focus {moniker}
    </button>
  );
}

// ---------------------------------------------------------------------------
// Tests — Space dispatch contract
// ---------------------------------------------------------------------------

describe("Inspectable — Space-key inspect dispatch contract", () => {
  beforeEach(() => {
    mockInvoke.mockClear();
    mockListen.mockClear();
    listeners.clear();
    monikerToKey.clear();
    currentFocusKey.key = null;
    mockInvoke.mockImplementation(defaultInvokeImpl);
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  // -------------------------------------------------------------------------
  // #1: Space on a focused inspectable dispatches inspect with wrapper moniker
  // -------------------------------------------------------------------------

  it("space_on_focused_inspectable_dispatches_inspect_with_wrapper_moniker", async () => {
    const { getByText, unmount } = render(
      withAppShell(
        <Inspectable moniker={asSegment("task:T1")}>
          <FocusScope moniker={asSegment("task:T1")}>
            <FocusButton moniker={asFq("task:T1")} />
          </FocusScope>
        </Inspectable>,
      ),
    );
    await flushSetup();

    // Click the inner button to claim focus on the wrapping FocusScope.
    await act(async () => {
      fireEvent.click(getByText("Focus task:T1"));
    });
    await flushSetup();

    mockInvoke.mockClear();

    // Fire Space at the document level — same path the global keymap
    // handler in `<KeybindingHandler>` listens on. The Inspectable's
    // scope-level command (`entity.inspect`) is keyed to Space and is in
    // the focused scope chain, so it dispatches `ui.inspect` against
    // the wrapper's moniker.
    await act(async () => {
      fireEvent.keyDown(document, { key: " ", code: "Space" });
    });
    await flushSetup();

    const dispatches = inspectDispatches();
    expect(
      dispatches.length,
      "Space on a focused inspectable must dispatch ui.inspect exactly once",
    ).toBe(1);
    // `runBackendDispatch` carries `target` at the top level of the
    // IPC payload, not inside `args` — same shape `inspectable.spatial.test.tsx`
    // pins for the dblclick path.
    expect(dispatches[0].target).toBe("task:T1");

    unmount();
  });

  // -------------------------------------------------------------------------
  // #2: Nested inspectables — closest one wins
  // -------------------------------------------------------------------------

  it("space_on_focused_descendant_dispatches_inspect_with_nearest_inspectable_moniker", async () => {
    const { getByText, unmount } = render(
      withAppShell(
        <Inspectable moniker={asSegment("task:T1")}>
          <Inspectable moniker={asSegment("field:task:T1.title")}>
            <FocusScope moniker={asSegment("field:task:T1.title")}>
              <FocusButton moniker={asFq("field:task:T1.title")} />
            </FocusScope>
          </Inspectable>
        </Inspectable>,
      ),
    );
    await flushSetup();

    await act(async () => {
      fireEvent.click(getByText("Focus field:task:T1.title"));
    });
    await flushSetup();

    mockInvoke.mockClear();
    await act(async () => {
      fireEvent.keyDown(document, { key: " ", code: "Space" });
    });
    await flushSetup();

    const dispatches = inspectDispatches();
    expect(
      dispatches.length,
      "Space on a focused inner inspectable must dispatch exactly once",
    ).toBe(1);
    // The closest enclosing `<Inspectable>` wins — its scope-level
    // `entity.inspect` shadows the outer one in the scope chain.
    expect(dispatches[0].target).toBe("field:task:T1.title");

    unmount();
  });

  // -------------------------------------------------------------------------
  // #3: Space on an <input> inside an Inspectable is ignored by the
  //     global handler so the editor can insert a literal space.
  // -------------------------------------------------------------------------

  it("space_inside_input_does_not_dispatch_inspect", async () => {
    const { getByTestId, unmount } = render(
      withAppShell(
        <Inspectable moniker={asSegment("task:T1")}>
          <FocusScope moniker={asSegment("task:T1")}>
            <input data-testid="text-input" type="text" />
          </FocusScope>
        </Inspectable>,
      ),
    );
    await flushSetup();

    const input = getByTestId("text-input") as HTMLInputElement;
    input.focus();
    await flushSetup();

    mockInvoke.mockClear();

    // Fire Space directly at the input. The global keybinding handler's
    // `isEditableTarget` check returns true for `<input>` and short-
    // circuits before any binding resolution, so `entity.inspect`
    // never runs.
    await act(async () => {
      fireEvent.keyDown(input, { key: " ", code: "Space" });
    });
    await flushSetup();

    expect(
      inspectDispatches().length,
      "Space inside an <input> must NOT dispatch ui.inspect",
    ).toBe(0);

    unmount();
  });

  // -------------------------------------------------------------------------
  // #4: Same exclusion for [contenteditable]
  // -------------------------------------------------------------------------

  it("space_inside_contenteditable_does_not_dispatch_inspect", async () => {
    const { getByTestId, unmount } = render(
      withAppShell(
        <Inspectable moniker={asSegment("task:T1")}>
          <FocusScope moniker={asSegment("task:T1")}>
            <div
              data-testid="ce-host"
              contentEditable
              suppressContentEditableWarning
            >
              <span data-testid="ce-inner">x</span>
            </div>
          </FocusScope>
        </Inspectable>,
      ),
    );
    await flushSetup();

    // Focus the contenteditable host so `e.target.closest("[contenteditable]")`
    // resolves on the keydown.
    const host = getByTestId("ce-host") as HTMLElement;
    host.focus();
    await flushSetup();

    mockInvoke.mockClear();
    await act(async () => {
      fireEvent.keyDown(host, { key: " ", code: "Space" });
    });
    await flushSetup();

    expect(
      inspectDispatches().length,
      "Space inside a [contenteditable] host must NOT dispatch ui.inspect",
    ).toBe(0);

    unmount();
  });

  // -------------------------------------------------------------------------
  // #5: Regression — dblclick still works after Space owner moves to wrapper.
  // -------------------------------------------------------------------------

  it("dblclick_on_inspectable_still_dispatches_inspect", async () => {
    const { getByTestId, unmount } = render(
      withAppShell(
        <Inspectable moniker={asSegment("task:T1")}>
          <FocusScope moniker={asSegment("task:T1")}>
            <div data-testid="card-body">card</div>
          </FocusScope>
        </Inspectable>,
      ),
    );
    await flushSetup();

    mockInvoke.mockClear();
    await act(async () => {
      fireEvent.doubleClick(getByTestId("card-body"));
    });
    await flushSetup();

    const dispatches = inspectDispatches();
    expect(dispatches.length).toBe(1);
    expect(dispatches[0].target).toBe("task:T1");

    unmount();
  });
});
