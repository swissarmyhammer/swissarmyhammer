/**
 * Repeat-open auto-focus test for card `01KR9G3KY1BWM1Y6BHH1W6T9H6`.
 *
 * Bug report: after the modal-layer refactor in
 * `01KR7CDEFWWVF4WH0BCHE8Y21J`, the first time an inspector opens in a
 * session, `useFirstFieldFocus` correctly dispatches `nav.focus` and
 * focus lands inside the inspector layer. On subsequent opens (close
 * inspector → click another card to inspect) focus does NOT move into
 * the inspector in production — it stays on whatever card was clicked.
 *
 * # Cycle under test (full production fidelity)
 *
 * 1. Click card A — dispatches `nav.focus(cardA_fq)` against a real
 *    `<FocusScope moniker="task:TA">` mounted under the window layer.
 * 2. Inspect A — pushes `task:TA` onto `inspector_stack`; the
 *    `<FocusLayer name="inspector">` mounts and the
 *    `EntityInspector`'s `useFirstFieldFocus` dispatches `nav.focus`
 *    against the first field's FQM under `/window/inspector/...`.
 *    Asserts focus lands on the inspector field.
 * 3. Dismiss A via `app.dismiss` — backend pops the stack, the
 *    inspector layer unmounts, `popLayer` returns the layer's
 *    `last_focused` and the React side issues a follow-up
 *    `spatial_focus` to restore the prior moniker. The simulator's
 *    `record_focus` walk mirrors the kernel so the layer's
 *    `last_focused` slot is set correctly during the open phase.
 * 4. Click card B — same as step 1 against `task:TB`.
 * 5. Inspect B — same as step 2; asserts `nav.focus(field_B.title)`
 *    fires AND the entity-focus probe reflects the field's FQM. This
 *    is the assertion the user reports failing in production.
 * 6. Dismiss B, click card A again, inspect A — third cycle, asserts
 *    the auto-focus is not single-shot.
 *
 * # Harness fidelity
 *
 * The simulator (`installKernelSimulator`) was extended for this card
 * to mirror the real Rust kernel:
 *   - `spatial_focus` walks `snapshot.layer_fq` up the parent chain
 *     and writes the focused FQM into each ancestor layer's
 *     `lastFocused` slot. Mirrors
 *     `swissarmyhammer-focus/src/registry.rs::record_focus`.
 *   - `spatial_pop_layer` returns the popped layer's `lastFocused`,
 *     matching `kanban-app/src/commands.rs::spatial_pop_layer`.
 *   - `spatial_focus` validates `snapshot` strictly when invoked with
 *     `strictFocusValidation: true` (this test opts in) — rejects
 *     `undefined` snapshots, snapshots whose `layer_fq` isn't pushed,
 *     and FQMs not present in `snapshot.scopes`.
 *
 * # Reproduction status
 *
 * Even with the harness extensions above, the test PASSES — auto-focus
 * fires on every inspector mount, including second/third opens. The
 * simulator faithfully reproduces production's IPC trace (verified by
 * the tracing diagnostic that was used while building this test) but
 * the focus-claim race the user manually reproduces in
 * `cargo tauri dev` does not surface against the simulator. The
 * remaining gap likely involves real DOM focus events, real-IPC
 * ordering, or a kernel-side resolve race that the JS simulator's
 * synchronous `record_focus` walk does not exhibit.
 *
 * The test stands as a regression guard for the contract: nav.focus
 * for the new entity's first field fires on every inspector mount,
 * even after multiple close → open cycles. If that contract regresses
 * in pure-React-side terms, this test goes red.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, act, waitFor } from "@testing-library/react";

// ---------------------------------------------------------------------------
// Hoisted Tauri-API spy triple — same shape as
// `inspector.close-restores-focus.browser.test.tsx`.
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
  emit: vi.fn(() => Promise.resolve()),
  listen: (...a: Parameters<typeof mockListen>) => mockListen(...a),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    label: "main",
    listen: vi.fn(() => Promise.resolve(() => {})),
  }),
}));

vi.mock("@tauri-apps/api/webview", () => ({
  getCurrentWebview: () => ({
    onDragDropEvent: vi.fn(() => Promise.resolve(() => {})),
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
// Imports come after the mocks.
// ---------------------------------------------------------------------------

import "@/components/fields/registrations";
import { AppShell } from "./app-shell";
import { InspectorsContainer } from "./inspectors-container";
import { FocusLayer } from "./focus-layer";
import { FocusScope } from "./focus-scope";
import { UIStateProvider } from "@/lib/ui-state-context";
import { AppModeProvider } from "@/lib/app-mode-context";
import { UndoProvider } from "@/lib/undo-context";
import {
  EntityFocusProvider,
  useEntityFocus,
} from "@/lib/entity-focus-context";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { SchemaProvider } from "@/lib/schema-context";
import { EntityStoreProvider } from "@/lib/entity-store-context";
import { FieldUpdateProvider } from "@/lib/field-update-context";
import { TooltipProvider } from "@/components/ui/tooltip";
import {
  ActiveBoardPathProvider,
  useDispatchCommand,
} from "@/lib/command-scope";
import { asSegment, composeFq, fqRoot } from "@/types/spatial";
import type { FullyQualifiedMoniker } from "@/types/spatial";
import { installKernelSimulator } from "@/test-helpers/kernel-simulator";
import { useEffect } from "react";

// ---------------------------------------------------------------------------
// Schema + entities — two distinct tasks.
// ---------------------------------------------------------------------------

const TASK_A = {
  entity_type: "task",
  id: "TA",
  moniker: "task:TA",
  fields: { title: "Alpha", status: "todo", body: "A body" },
};

const TASK_B = {
  entity_type: "task",
  id: "TB",
  moniker: "task:TB",
  fields: { title: "Bravo", status: "doing", body: "B body" },
};

const TASK_SCHEMA = {
  entity: {
    name: "task",
    body_field: "body",
    fields: ["title", "status", "body"],
  },
  fields: [
    {
      id: "f1",
      name: "title",
      type: { kind: "markdown", single_line: true },
      editor: "markdown",
      display: "text",
      icon: "type",
      section: "header",
    },
    {
      id: "f2",
      name: "status",
      type: { kind: "text", single_line: true },
      editor: "markdown",
      display: "text",
      icon: "circle",
      section: "header",
    },
    {
      id: "f3",
      name: "body",
      type: { kind: "markdown", single_line: false },
      editor: "markdown",
      display: "markdown",
      icon: "file-text",
      section: "body",
    },
  ],
};

// ---------------------------------------------------------------------------
// Mutable backend state — `inspector_stack` mutates as panels open / close.
// ---------------------------------------------------------------------------

const backendState = { inspector_stack: [] as string[] };

function uiStateSnapshot() {
  return {
    keymap_mode: "cua" as const,
    scope_chain: [],
    open_boards: [],
    has_clipboard: false,
    clipboard_entity_type: null,
    can_undo: false,
    can_redo: false,
    drag_session: null,
    windows: {
      main: {
        board_path: "/test",
        inspector_stack: [...backendState.inspector_stack],
        active_view_id: "",
        active_perspective_id: "",
        palette_open: false,
        palette_mode: "command" as const,
      },
    },
    recent_boards: [],
  };
}

/**
 * Emit a synthetic `ui-state-changed` event so the UIStateProvider
 * picks up the new `inspector_stack`. The `kind` is informational —
 * the React side reacts to the snapshot, not the kind.
 */
function emitUiStateChanged(kind: string) {
  const cbs = listeners.get("ui-state-changed") ?? [];
  for (const cb of cbs) {
    cb({ payload: { kind, state: uiStateSnapshot() } });
  }
}

async function defaultInvokeImpl(
  cmd: string,
  args?: unknown,
): Promise<unknown> {
  if (cmd === "get_ui_state") return uiStateSnapshot();
  if (cmd === "list_entity_types") return ["task"];
  if (cmd === "get_entity_schema") return TASK_SCHEMA;
  if (cmd === "get_entity") {
    const a = (args ?? {}) as { id?: string };
    const id = a.id ?? "TA";
    const fields =
      id === "TA" ? TASK_A.fields : id === "TB" ? TASK_B.fields : {};
    return {
      entity_type: "task",
      id,
      moniker: `task:${id}`,
      fields,
    };
  }
  if (cmd === "get_undo_state") return { can_undo: false, can_redo: false };
  if (cmd === "list_commands_for_scope") return [];
  if (cmd === "list_views") return [];
  if (cmd === "list_perspectives") return [];
  if (cmd === "log_command") return null;
  if (cmd === "dispatch_command") {
    const a = (args ?? {}) as { cmd?: string };
    if (a.cmd === "ui.inspector.close" || a.cmd === "app.dismiss") {
      backendState.inspector_stack.pop();
      emitUiStateChanged("InspectorClosed");
      return null;
    }
    return null;
  }
  return null;
}

const WINDOW_LAYER_NAME = asSegment("window");
const WINDOW_LAYER_FQ = fqRoot(WINDOW_LAYER_NAME);
const CARD_A_FQ: FullyQualifiedMoniker = composeFq(
  WINDOW_LAYER_FQ,
  asSegment("task:TA"),
);
const CARD_B_FQ: FullyQualifiedMoniker = composeFq(
  WINDOW_LAYER_FQ,
  asSegment("task:TB"),
);

function FocusedFqProbe() {
  const { focusedFq } = useEntityFocus();
  return <span data-testid="focused-fq-probe">{focusedFq ?? "null"}</span>;
}

/**
 * Test-only hook driver — captures `dispatchNavFocus` into a ref the
 * test body can call to simulate a card click that claims focus on the
 * card scope. Mirrors production: the `<FocusScope>` `onClick` handler
 * dispatches `nav.focus` against the card's FQM before any inspect
 * command fires.
 */
function DispatchCapture({
  ref,
}: {
  ref: { current: ((fq: FullyQualifiedMoniker) => Promise<void>) | null };
}) {
  const dispatchNavFocus = useDispatchCommand("nav.focus");
  useEffect(() => {
    ref.current = async (fq) => {
      await dispatchNavFocus({ args: { fq } });
    };
    return () => {
      ref.current = null;
    };
  }, [dispatchNavFocus, ref]);
  return null;
}

function renderInspectorChain(dispatchRef: {
  current: ((fq: FullyQualifiedMoniker) => Promise<void>) | null;
}) {
  return render(
    <SpatialFocusProvider>
      <FocusLayer name={WINDOW_LAYER_NAME}>
        <UIStateProvider>
          <EntityFocusProvider>
            <AppModeProvider>
              <UndoProvider>
                <TooltipProvider delayDuration={100}>
                  <SchemaProvider>
                    <EntityStoreProvider entities={{ task: [TASK_A, TASK_B] }}>
                      <FieldUpdateProvider>
                        <ActiveBoardPathProvider value="/test/board">
                          <AppShell>
                            <FocusedFqProbe />
                            <DispatchCapture ref={dispatchRef} />
                            {/* Two card scopes — exactly the
                                production shape that lets a card-click
                                claim focus on `task:TA` / `task:TB`
                                before the inspector opens. Each card
                                registers under the window layer so
                                `record_focus` walks the window-layer
                                ancestry on focus claim. */}
                            <FocusScope
                              moniker={asSegment("task:TA")}
                              commands={[]}
                              data-testid="card-a"
                            >
                              <span>card A</span>
                            </FocusScope>
                            <FocusScope
                              moniker={asSegment("task:TB")}
                              commands={[]}
                              data-testid="card-b"
                            >
                              <span>card B</span>
                            </FocusScope>
                            <InspectorsContainer />
                          </AppShell>
                        </ActiveBoardPathProvider>
                      </FieldUpdateProvider>
                    </EntityStoreProvider>
                  </SchemaProvider>
                </TooltipProvider>
              </UndoProvider>
            </AppModeProvider>
          </EntityFocusProvider>
        </UIStateProvider>
      </FocusLayer>
    </SpatialFocusProvider>,
  );
}

async function flushAsync() {
  await act(async () => {
    await new Promise((r) => setTimeout(r, 50));
  });
}

/**
 * Mutate the backend's `inspector_stack` and emit a `ui-state-changed`
 * so the React tree reacts. Wrapped in `act` so all state work flushes
 * before the assertion.
 */
async function setInspectorStack(next: string[]) {
  await act(async () => {
    backendState.inspector_stack = next;
    emitUiStateChanged("InspectorOpened");
    await Promise.resolve();
  });
}

describe("Inspector — auto-focus on every inspect (not only the first)", () => {
  beforeEach(() => {
    backendState.inspector_stack = [];
    mockInvoke.mockClear();
    mockListen.mockClear();
    listeners.clear();
  });

  it("opens A, closes A, opens B, closes B, opens A again — auto-focus dispatches nav.focus for the new entity's first field on EVERY mount", async () => {
    // Start with no panels open.
    backendState.inspector_stack = [];
    // `strictFocusValidation: true` mirrors the real Rust kernel's
    // snapshot validation (state.rs::focus) so the simulator can drive
    // `record_focus`'s parent-layer walk and the resulting
    // `last_focused` slot updates faithfully. Without it, the close
    // path's pop-layer follow-up bypasses the validation that gates
    // the kernel's last_focused walk in production.
    installKernelSimulator(mockInvoke, listeners, defaultInvokeImpl, {
      strictFocusValidation: true,
    });

    // Ref the `<DispatchCapture>` populates so the test body can
    // simulate a card-click `nav.focus` dispatch — the production flow
    // we missed earlier (the card scope claims focus BEFORE the
    // inspector dispatch fires, so the kernel's window layer
    // `last_focused` slot is set to the card FQM).
    const dispatchRef: {
      current: ((fq: FullyQualifiedMoniker) => Promise<void>) | null;
    } = { current: null };
    const { getByTestId, unmount } = renderInspectorChain(dispatchRef);
    await flushAsync();
    await waitFor(() => {
      expect(dispatchRef.current).not.toBeNull();
    });

    // Helper: every spatial_focus dispatch in the order it fired.
    const focusCalls = () =>
      mockInvoke.mock.calls
        .filter((c) => c[0] === "spatial_focus")
        .map((c) => (c[1] as { fq?: string } | undefined)?.fq ?? "?");

    /** Simulate a card click: nav.focus(card_fq) then inspect open. */
    async function clickCardThenInspect(
      cardFq: FullyQualifiedMoniker,
      taskMoniker: string,
    ) {
      // Step 1: the FocusScope's onClick handler dispatches
      // `nav.focus(cardFq)`. We do that directly here.
      await act(async () => {
        await dispatchRef.current!(cardFq);
      });
      // Step 2: the Inspectable's gesture dispatches `ui.inspect`,
      // which (in production) round-trips through the backend and
      // mutates `inspector_stack`. The test fakes the backend mutation
      // by pushing the entity moniker onto the stack and emitting
      // `ui-state-changed`. No flush between focus claim and inspector
      // open — production fires both from the same gesture, so the
      // React commits land back-to-back.
      await setInspectorStack([...backendState.inspector_stack, taskMoniker]);
    }

    /** Simulate the user dismissing the topmost panel. */
    async function dismissTop() {
      await act(async () => {
        backendState.inspector_stack.pop();
        emitUiStateChanged("InspectorClosed");
        await Promise.resolve();
      });
    }

    // ---- Open inspector A (card click → inspect) ----
    await clickCardThenInspect(CARD_A_FQ, "task:TA");
    await flushAsync();

    // After the first open: a spatial_focus IPC must have fired for
    // A's first field's FQM under the inspector layer.
    await waitFor(
      () => {
        const calls = focusCalls();
        const aField = calls.find((fq) =>
          fq.startsWith("/window/inspector/field:task:TA."),
        );
        expect(
          aField,
          `expected first-open spatial_focus for A's first field; saw: ${calls.join(", ")}`,
        ).toBeDefined();
      },
      { timeout: 1500 },
    );
    await waitFor(() => {
      expect(getByTestId("focused-fq-probe").textContent).toMatch(
        /\/window\/inspector\/field:task:TA\./,
      );
    });

    // Snapshot of how many spatial_focus calls fired during A's life.
    const aFinalCallCount = focusCalls().length;

    // ---- Close A ----
    await dismissTop();
    await flushAsync();

    // ---- Open inspector B (card click → inspect) ----
    await clickCardThenInspect(CARD_B_FQ, "task:TB");
    await flushAsync();

    // The contract: after the second open, a spatial_focus IPC must
    // have fired for B's first field. This is what
    // `useFirstFieldFocus` promises on EVERY inspector mount, not just
    // the first.
    await waitFor(
      () => {
        const calls = focusCalls();
        const callsAfterA = calls.slice(aFinalCallCount);
        const bField = callsAfterA.find((fq) =>
          fq.startsWith("/window/inspector/field:task:TB."),
        );
        expect(
          bField,
          `expected second-open spatial_focus for B's first field; ` +
            `calls after A's open were: ${callsAfterA.join(", ")}`,
        ).toBeDefined();
      },
      { timeout: 1500 },
    );

    // Tighter pin: the entity-focus store (a pure projection of
    // focus-changed events) reflects B's first field as the focused
    // FQM — i.e. B's `useFirstFieldFocus` claim wins, not whatever the
    // kernel restored after A closed.
    await waitFor(
      () => {
        expect(getByTestId("focused-fq-probe").textContent).toMatch(
          /\/window\/inspector\/field:task:TB\./,
        );
      },
      { timeout: 1500 },
    );

    const bFinalCallCount = focusCalls().length;

    // ---- Close B ----
    await dismissTop();
    await flushAsync();

    // ---- Open inspector A again (third inspect of the session) ----
    await clickCardThenInspect(CARD_A_FQ, "task:TA");
    await flushAsync();

    await waitFor(
      () => {
        const calls = focusCalls();
        const callsAfterB = calls.slice(bFinalCallCount);
        const aField = callsAfterB.find((fq) =>
          fq.startsWith("/window/inspector/field:task:TA."),
        );
        expect(
          aField,
          `expected third-open spatial_focus for A's first field; ` +
            `calls after B's open were: ${callsAfterB.join(", ")}`,
        ).toBeDefined();
      },
      { timeout: 1500 },
    );
    await waitFor(() => {
      expect(getByTestId("focused-fq-probe").textContent).toMatch(
        /\/window\/inspector\/field:task:TA\./,
      );
    });

    unmount();
  });
});
