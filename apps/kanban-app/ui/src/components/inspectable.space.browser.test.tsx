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
 *   6. Space at app open (no kernel focus) MUST `preventDefault` so the
 *      browser does not scroll the page, AND must NOT dispatch
 *      `ui.inspect`. Pinned by the root-scope `entity.inspect` command in
 *      `app-shell.tsx` — the per-Inspectable scope command does not fire
 *      because no Inspectable is in the empty scope chain.
 *   7. Space with kernel focus on a non-Inspectable scope (e.g. a
 *      perspective tab, filter editor) MUST `preventDefault` and must
 *      NOT dispatch `ui.inspect` — the root command picks up Space
 *      because the focused scope chain has no `<Inspectable>` to shadow,
 *      but its execute closure filters by the inspectable-entity
 *      prefixes (`task:`, `tag:`, `column:`, `board:`, `field:`,
 *      `attachment:`).
 *   8. Space inside an editable surface (`<input>`, `<textarea>`,
 *      `[contenteditable]`) MUST NOT `preventDefault` — the global
 *      keybinding handler's `isEditableTarget` gate short-circuits before
 *      any binding lookup so the editor's own input handler still
 *      inserts a literal space character.
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
import { commandToolCall } from "@/test/mock-command-list";
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
 * Build the default `invoke` implementation covering the IPCs the
 * provider stack fires on mount, parameterized by the keymap mode the
 * `get_ui_state` branch reports.
 *
 * The keymap mode threads through `<AppShell>`'s `useAppShellUIState`
 * hook into the global keybinding handler created by
 * `<KeybindingHandler>`. Tests that need vim-mode coverage of the
 * Space-key contract call this factory with `"vim"` so the same
 * scenarios run under each mode without duplicating fixture
 * machinery. The cua / emacs / vim binding-table parity for `Space`
 * lives in `lib/keybindings.ts`; per-`<Inspectable>` and root-scope
 * `CommandDef`s also carry all three modes — see the matching
 * `keys: { vim, cua, emacs }` blocks in `inspectable.tsx` and
 * `app-shell.tsx`.
 *
 * @param keymapMode - The keymap mode to advertise; defaults to `"cua"`.
 */
function makeDefaultInvokeImpl(
  keymapMode: "cua" | "vim" | "emacs" = "cua",
): (cmd: string, args?: unknown) => Promise<unknown> {
  return async function defaultInvokeImpl(
    cmd: string,
    args?: unknown,
  ): Promise<unknown> {
    if (cmd === "command_tool_call") return commandToolCall(args);
    if (cmd === "get_ui_state") {
      return {
        palette_open: false,
        palette_mode: "command",
        keymap_mode: keymapMode,
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
  };
}

/**
 * Backwards-compatible alias used by the existing cua-mode scenarios
 * below. Equivalent to `makeDefaultInvokeImpl("cua")`; the named
 * binding keeps every legacy `mockInvoke.mockImplementation(defaultInvokeImpl)`
 * call site working without churn.
 */
const defaultInvokeImpl = makeDefaultInvokeImpl("cua");

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

/**
 * Construct a cancelable bubbling Space `keydown` event and dispatch it at
 * `target`. Returns the event so the caller can assert
 * `event.defaultPrevented` after the dispatch — the production keybinding
 * handler calls `preventDefault()` only when it resolves a binding, so the
 * flag is the load-bearing signal that Space was claimed (no page scroll)
 * vs left alone (browser default fires).
 */
function dispatchSpace(target: EventTarget): KeyboardEvent {
  const event = new KeyboardEvent("keydown", {
    key: " ",
    code: "Space",
    bubbles: true,
    cancelable: true,
  });
  target.dispatchEvent(event);
  return event;
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

  // -------------------------------------------------------------------------
  // #6: Space at app open with no kernel focus — preventDefault, no inspect.
  // -------------------------------------------------------------------------
  //
  // The user opens the app, kernel focus is null (`<body>` carries DOM
  // focus). Pressing Space MUST NOT scroll the page; it MUST NOT
  // dispatch `ui.inspect` either (the recommended (a) variant in the
  // task plan: no-op + preventDefault is unambiguous).
  //
  // The fix is a root-scope `entity.inspect` command in `app-shell.tsx`
  // bound to Space — the binding lookup succeeds, the keybinding
  // handler calls `preventDefault()`, and the command's execute
  // closure no-ops because `focusedFq` is null.

  it("space_at_app_open_with_no_kernel_focus_preventDefaults_and_does_not_dispatch_inspect", async () => {
    const { unmount } = render(
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

    // No setFocus, no click — kernel focus is null, DOM focus on
    // `<body>`. The global keydown listener is attached on `document`
    // by `<KeybindingHandler>`, so dispatching at `document` mirrors
    // the production code path.
    let event!: KeyboardEvent;
    await act(async () => {
      event = dispatchSpace(document);
    });
    await flushSetup();

    expect(
      event.defaultPrevented,
      "Space at app open must preventDefault so the browser does not scroll",
    ).toBe(true);
    expect(
      inspectDispatches().length,
      "Space with no kernel focus must NOT dispatch ui.inspect",
    ).toBe(0);

    unmount();
  });

  // -------------------------------------------------------------------------
  // #7: Space with focus on a non-Inspectable scope — preventDefault, no inspect.
  // -------------------------------------------------------------------------
  //
  // A focused perspective tab / filter editor / other UI chrome is not
  // an inspectable entity. Pressing Space MUST NOT scroll the page and
  // MUST NOT dispatch `ui.inspect` — the root command's execute
  // closure filters by the inspectable-entity prefix set
  // (`task:`, `tag:`, `column:`, `board:`, `field:`, `attachment:`)
  // and no-ops on anything else. The keybinding handler still claims
  // the keystroke (preventDefault) because the binding resolved.

  it("space_with_focus_on_non_inspectable_scope_preventDefaults_and_does_not_dispatch_inspect", async () => {
    // Use a `perspective_tab:` moniker — chrome, not an entity. The
    // Inspectable wrapper guard explicitly excludes this prefix
    // (focus-architecture.guards.node.test.ts), so the FocusScope
    // below sits outside any Inspectable and the focused scope chain
    // contains no `entity.inspect` shadow.
    const { getByText, unmount } = render(
      withAppShell(
        <FocusScope moniker={asSegment("perspective_tab:active")}>
          <FocusButton moniker={asFq("perspective_tab:active")} />
        </FocusScope>,
      ),
    );
    await flushSetup();

    await act(async () => {
      fireEvent.click(getByText("Focus perspective_tab:active"));
    });
    await flushSetup();

    mockInvoke.mockClear();

    let event!: KeyboardEvent;
    await act(async () => {
      event = dispatchSpace(document);
    });
    await flushSetup();

    expect(
      event.defaultPrevented,
      "Space on a non-Inspectable focused scope must preventDefault (no scroll)",
    ).toBe(true);
    expect(
      inspectDispatches().length,
      "Space on a non-Inspectable focused scope must NOT dispatch ui.inspect",
    ).toBe(0);

    unmount();
  });

  // -------------------------------------------------------------------------
  // #8: Space inside an editable surface — does NOT preventDefault.
  // -------------------------------------------------------------------------
  //
  // Reinforces criterion 4 from the task plan: when DOM focus is on an
  // `<input>`, the global handler's `isEditableTarget` short-circuits
  // before binding resolution, so `preventDefault` is NOT called and
  // the editor inserts a literal space.

  it("space_inside_input_does_not_preventDefault_so_editor_inserts_a_space", async () => {
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

    let event!: KeyboardEvent;
    await act(async () => {
      event = dispatchSpace(input);
    });
    await flushSetup();

    expect(
      event.defaultPrevented,
      "Space inside an <input> must NOT preventDefault — the editor owns the gesture",
    ).toBe(false);
    expect(
      inspectDispatches().length,
      "Space inside an <input> must NOT dispatch ui.inspect",
    ).toBe(0);

    unmount();
  });

  // -------------------------------------------------------------------------
  // #9: Space with kernel focus on a card still dispatches with the
  //     wrapper moniker AND preventDefaults (positive scenario rolled
  //     into the new defaultPrevented assertion).
  // -------------------------------------------------------------------------

  it("space_with_kernel_focus_on_card_dispatches_inspect_and_preventDefaults", async () => {
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

    await act(async () => {
      fireEvent.click(getByText("Focus task:T1"));
    });
    await flushSetup();

    mockInvoke.mockClear();

    let event!: KeyboardEvent;
    await act(async () => {
      event = dispatchSpace(document);
    });
    await flushSetup();

    expect(
      event.defaultPrevented,
      "Space with kernel focus on an Inspectable must preventDefault",
    ).toBe(true);
    const dispatches = inspectDispatches();
    expect(dispatches.length).toBe(1);
    expect(dispatches[0].target).toBe("task:T1");

    unmount();
  });

  // -------------------------------------------------------------------------
  // Vim-mode parity — pins task `01KQJHFX0HADZH74P7KJQRFM4E` regression.
  // -------------------------------------------------------------------------
  //
  // The first iteration of the Space-binding fix scoped the new binding
  // to cua + emacs only, on a judgment call about a hypothetical vim
  // leader key. `SEQUENCE_TABLES.vim` has no `Space` prefix, so leaving
  // Space unbound there meant the binding-table lookup missed in vim
  // mode and the keydown handler did not call `preventDefault()` —
  // production users in vim mode still saw page-scroll on Space.
  //
  // The three scenarios below mirror #6, #7, and #9 above with the
  // `get_ui_state` mock advertising `keymap_mode: "vim"` instead of
  // `"cua"`. Each was a hard failure before vim was added to the three
  // `keys` maps (`BINDING_TABLES.vim`, the per-`<Inspectable>` scope
  // command in `inspectable.tsx`, and the root command in
  // `app-shell.tsx`).

  describe("Vim-mode parity — Space inspects in all three keymaps", () => {
    beforeEach(() => {
      mockInvoke.mockImplementation(makeDefaultInvokeImpl("vim"));
    });

    it("vim_space_at_app_open_with_no_kernel_focus_preventDefaults_and_does_not_dispatch_inspect", async () => {
      const { unmount } = render(
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

      let event!: KeyboardEvent;
      await act(async () => {
        event = dispatchSpace(document);
      });
      await flushSetup();

      expect(
        event.defaultPrevented,
        "Space at app open in vim mode must preventDefault so the browser does not scroll",
      ).toBe(true);
      expect(
        inspectDispatches().length,
        "Space with no kernel focus in vim mode must NOT dispatch ui.inspect",
      ).toBe(0);

      unmount();
    });

    it("vim_space_with_focus_on_non_inspectable_scope_preventDefaults_and_does_not_dispatch_inspect", async () => {
      const { getByText, unmount } = render(
        withAppShell(
          <FocusScope moniker={asSegment("perspective_tab:active")}>
            <FocusButton moniker={asFq("perspective_tab:active")} />
          </FocusScope>,
        ),
      );
      await flushSetup();

      await act(async () => {
        fireEvent.click(getByText("Focus perspective_tab:active"));
      });
      await flushSetup();

      mockInvoke.mockClear();

      let event!: KeyboardEvent;
      await act(async () => {
        event = dispatchSpace(document);
      });
      await flushSetup();

      expect(
        event.defaultPrevented,
        "Space on a non-Inspectable focused scope in vim mode must preventDefault (no scroll)",
      ).toBe(true);
      expect(
        inspectDispatches().length,
        "Space on a non-Inspectable focused scope in vim mode must NOT dispatch ui.inspect",
      ).toBe(0);

      unmount();
    });

    it("vim_space_with_kernel_focus_on_card_dispatches_inspect_and_preventDefaults", async () => {
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

      await act(async () => {
        fireEvent.click(getByText("Focus task:T1"));
      });
      await flushSetup();

      mockInvoke.mockClear();

      let event!: KeyboardEvent;
      await act(async () => {
        event = dispatchSpace(document);
      });
      await flushSetup();

      expect(
        event.defaultPrevented,
        "Space with kernel focus on an Inspectable in vim mode must preventDefault",
      ).toBe(true);
      const dispatches = inspectDispatches();
      expect(
        dispatches.length,
        "Space on a focused card in vim mode must dispatch ui.inspect exactly once",
      ).toBe(1);
      expect(dispatches[0].target).toBe("task:T1");

      unmount();
    });
  });
});
