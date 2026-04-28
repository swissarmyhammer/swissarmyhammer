/**
 * End-to-end-shape tests for the **Escape closes the inspector** chain.
 *
 * Mounts AppShell + UIStateProvider + InspectorsContainer in the same
 * relative ordering App.tsx uses, populates `inspector_stack` from a
 * mocked backend, and drives the chain at the document level (the
 * keyboard event the user actually delivers). Each test asserts on the
 * **observable end state** — what `inspector_stack` becomes, what the
 * panel DOM looks like, what `dispatch_command` was invoked with —
 * rather than poking at private call paths.
 *
 * This complements the unit-level `app-shell.test.tsx` and
 * `inspectors-container.test.tsx` files: those test individual seams
 * (drillOut returns null → dispatch app.dismiss, panel zone registers
 * with parent_zone null, etc.). This file pins the **chain** —
 * Escape → `nav.drillOut` → `app.dismiss` → `inspector_close` →
 * UIState reactive update → panel unmount.
 *
 * # The mocked backend
 *
 * `invoke` is mocked to return realistic responses for the commands the
 * chain triggers:
 *
 *   - `get_ui_state` returns the test's current UIState snapshot.
 *   - `spatial_register_zone`, `spatial_register_layer`, `spatial_focus`
 *     return null (the kernel is not under test here; the kernel-level
 *     contract for `drill_out` is pinned in the Rust integration test
 *     `swissarmyhammer-focus/tests/inspector_dismiss.rs`).
 *   - `spatial_drill_out` returns null when called against the panel-
 *     zone key — simulating the kernel's "this scope is at the layer
 *     root, fall through to dismiss" answer.
 *   - `dispatch_command(app.dismiss)` synthesises the same effect
 *     `DismissCmd::execute` produces: pops the topmost panel from the
 *     test's UIState and emits a `ui-state-changed` event so the React
 *     `<UIStateProvider>` updates state and `<InspectorsContainer>`
 *     re-renders without the popped panel. The Rust side of this
 *     contract is pinned in
 *     `swissarmyhammer-kanban/tests/dismiss_inspector_integration.rs`.
 *
 * The chain gate this file pins is the **React-side wiring**: that
 * pressing Escape under various focus / mode conditions actually drives
 * the dispatch + state update sequence end-to-end.
 *
 * Card: `01KQ9TVZYXN65JHA479D1CS91T`.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, fireEvent, act, waitFor } from "@testing-library/react";

// ---------------------------------------------------------------------------
// Tauri API mocks — must be installed before importing components.
// ---------------------------------------------------------------------------

/**
 * Per-test mutable UIState snapshot. The mocked backend reads from this
 * (via `get_ui_state`) and writes to it (via `dispatch_command(app.dismiss)`),
 * mirroring how the real Rust backend mutates `Arc<UIState>` and then
 * fans out `ui-state-changed` events.
 */
interface MutableUIState {
  inspector_stack: string[];
  palette_open: boolean;
}

const backendState: MutableUIState = {
  inspector_stack: [],
  palette_open: false,
};

/** Capture handler refs so tests can fire synthetic Tauri events. */
const listenCallbacks: Record<string, (event: unknown) => void> = {};

/**
 * Build the `UIStateSnapshot` shape the React `<UIStateProvider>` expects
 * from the mocked `get_ui_state` invoke and the synthetic
 * `ui-state-changed` event payloads.
 */
function uiStateSnapshot() {
  return {
    keymap_mode: "cua",
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
        palette_open: backendState.palette_open,
        palette_mode: "command" as const,
      },
    },
    recent_boards: [],
  };
}

/**
 * Emit a synthetic `ui-state-changed` event into the captured listener
 * (the same shape the Tauri backend emits after a `UIStateChange`).
 */
function emitUiStateChanged(kind: string) {
  const cb = listenCallbacks["ui-state-changed"];
  if (!cb) return;
  cb({ payload: { kind, state: uiStateSnapshot() } });
}

/**
 * Track every `spatial_register_zone` call so a test can find the panel
 * zone's `SpatialKey` (the kernel mints a fresh ULID per mount; tests
 * cannot hardcode it).
 */
const registeredZones: Array<{
  key: string;
  moniker: string;
  parentZone: string | null;
}> = [];

/**
 * Drill-out responses keyed by the SpatialKey passed to `spatial_drill_out`.
 * Tests set entries here so the kernel mock returns the right answer for
 * the panel-zone key (null) versus a leaf inside the panel (the panel
 * zone's moniker).
 */
const drillOutResponses = new Map<string, string | null>();

/** Latest `next_key` set by a `spatial_focus` invoke, replayed via `focus-changed`. */
let lastFocusedKey: string | null = null;

const mockInvoke = vi.fn(
  async (cmd: string, args?: Record<string, unknown>) => {
    if (cmd === "get_ui_state") return uiStateSnapshot();

    if (cmd === "spatial_register_zone") {
      registeredZones.push({
        key: args?.key as string,
        moniker: args?.moniker as string,
        parentZone: (args?.parentZone as string | null) ?? null,
      });
      return null;
    }

    if (cmd === "spatial_register_layer" || cmd === "spatial_push_layer") {
      return null;
    }

    if (cmd === "spatial_unregister_scope" || cmd === "spatial_pop_layer") {
      return null;
    }

    if (cmd === "spatial_update_rect") return null;

    if (cmd === "spatial_focus") {
      lastFocusedKey = (args?.key as string) ?? null;
      // Replay through the focus-changed bus so the React provider
      // records the focused key and `focusedKey()` returns it.
      const cb = listenCallbacks["focus-changed"];
      if (cb) {
        cb({
          payload: {
            window_label: "main",
            prev_key: null,
            next_key: lastFocusedKey,
            next_moniker: null,
          },
        });
      }
      return null;
    }

    if (cmd === "spatial_drill_out") {
      const key = (args?.key as string) ?? "";
      const focusedMoniker = (args?.focusedMoniker as string) ?? "";
      // Under the no-silent-dropout contract the kernel always returns
      // a moniker. Test entries with `null` mean "no zone-level drill
      // happened (layer root or torn state)" — echo the focused
      // moniker so the React closure detects equality and dispatches
      // app.dismiss. Test entries with a string mean "drill walked to
      // a parent zone" — return that string verbatim. Unknown keys
      // also echo the focused moniker (the kernel does this for torn
      // state, accompanied by tracing::error!).
      if (drillOutResponses.has(key)) {
        const v = drillOutResponses.get(key);
        return v === null ? focusedMoniker : v;
      }
      return focusedMoniker;
    }

    if (cmd === "spatial_drill_in") {
      // Symmetric to drill_out: echo the focused moniker so the React
      // closure's setFocus(result) is an idempotent no-op (the
      // legacy `null → no-op` semantic carried into the new contract).
      const focusedMoniker = (args?.focusedMoniker as string) ?? "";
      return focusedMoniker;
    }

    if (cmd === "spatial_navigate") return null;

    if (cmd === "log_command") return null;

    // CommandPalette reads `list_commands_for_scope` to populate its
    // command list. The palette mounts when `palette_open=true`; tests
    // for the palette branch above don't care about its body, but
    // `useMemo` over a null result throws — return an empty list.
    if (cmd === "list_commands_for_scope") return [];
    if (cmd === "list_views") return [];
    if (cmd === "list_perspectives") return [];

    // InspectorPanel tries to fetch the panel's entity when it's not in
    // the local store. The mocked InspectorFocusBridge renders nothing,
    // so the actual entity body never matters; return a minimal stub
    // bag so `entityFromBag` doesn't throw on a null result.
    if (cmd === "get_entity") {
      const eType = (args?.entityType as string) ?? "task";
      const id = (args?.id as string) ?? "stub";
      return {
        entity_type: eType,
        id,
        moniker: `${eType}:${id}`,
        fields: {},
      };
    }

    if (cmd === "dispatch_command") {
      const cmdId = args?.cmd as string;
      if (cmdId === "app.dismiss") {
        // Mirror DismissCmd::execute: layer 1 closes palette, layer 2
        // pops the topmost inspector panel. Pinned in the Rust
        // integration test `dismiss_inspector_integration.rs`.
        if (backendState.palette_open) {
          backendState.palette_open = false;
          emitUiStateChanged("palette_open");
          return null;
        }
        if (backendState.inspector_stack.length > 0) {
          backendState.inspector_stack.pop();
          emitUiStateChanged("inspector_stack");
          return null;
        }
        return null;
      }
      if (cmdId === "ui.inspect") {
        const target = args?.target as string;
        if (target) {
          backendState.inspector_stack.push(target);
          emitUiStateChanged("inspector_stack");
        }
        return null;
      }
      if (cmdId === "ui.inspector.close") {
        if (backendState.inspector_stack.length > 0) {
          backendState.inspector_stack.pop();
          emitUiStateChanged("inspector_stack");
        }
        return null;
      }
      if (cmdId === "ui.setFocus") {
        // Drill closures call setFocus → dispatch_command(ui.setFocus, …).
        // The chain doesn't need a side effect here; the focused key is
        // tracked separately via `spatial_focus` above.
        return null;
      }
      if (cmdId === "app.command" || cmdId === "app.palette") {
        backendState.palette_open = true;
        emitUiStateChanged("palette_open");
        return null;
      }
      return null;
    }

    return null;
  },
);

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) =>
    mockInvoke(args[0] as string, args[1] as Record<string, unknown>),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn((eventName: string, cb: (event: unknown) => void) => {
    listenCallbacks[eventName] = cb;
    return Promise.resolve(() => {
      delete listenCallbacks[eventName];
    });
  }),
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
// Mock the heavy descendants — the chain only needs InspectorsContainer's
// shape (panel zone registration + UIState reactive read), not the full
// inspector body.
// ---------------------------------------------------------------------------

vi.mock("@/lib/schema-context", () => ({
  useSchema: () => ({
    getSchema: () => null,
    schemas: {},
    loading: false,
  }),
  useSchemaOptional: () => undefined,
  SchemaProvider: ({ children }: { children: React.ReactNode }) => children,
}));

vi.mock("@/components/rust-engine-container", () => ({
  useEntitiesByType: () => ({}),
  useRefreshEntities: () => () => Promise.resolve(),
  useSetEntitiesByType: () => () => {},
  useEngineSetActiveBoardPath: () => () => {},
  RustEngineContainer: ({ children }: { children: React.ReactNode }) =>
    children,
}));

vi.mock("@/components/inspector-focus-bridge", () => ({
  InspectorFocusBridge: () => null,
}));

// ---------------------------------------------------------------------------
// Component imports — after mocks.
// ---------------------------------------------------------------------------

import { AppShell } from "./app-shell";
import { InspectorsContainer } from "./inspectors-container";
import { FocusLayer } from "./focus-layer";
import { UIStateProvider, useUIState } from "@/lib/ui-state-context";
import { AppModeProvider } from "@/lib/app-mode-context";
import { UndoProvider } from "@/lib/undo-context";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { asLayerName } from "@/types/spatial";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const WINDOW_LAYER_NAME = asLayerName("window");

/**
 * A small probe component that pulls `inspector_stack` out of UIState
 * and renders it as a data attribute. Tests assert against the rendered
 * value rather than the React context directly so the assertion is the
 * same shape the user observes (the panel container reads UIState the
 * same way).
 */
function StackProbe() {
  const ui = useUIState();
  const stack = ui.windows?.main?.inspector_stack ?? [];
  return (
    <div data-testid="stack-probe" data-stack={JSON.stringify(stack)}>
      stack:{stack.length}
    </div>
  );
}

/**
 * Render the production-shaped chain that the chain-under-test needs:
 *
 *   <SpatialFocusProvider>
 *     <FocusLayer name="window">
 *       <UIStateProvider>           ← drives inspector_stack
 *         <EntityFocusProvider>
 *           <AppModeProvider>
 *             <UndoProvider>
 *               <AppShell>          ← owns the keydown handler
 *                 <StackProbe />    ← observes UIState
 *                 <InspectorsContainer />  ← renders panels
 *               </AppShell>
 *             </UndoProvider>
 *           </AppModeProvider>
 *         </EntityFocusProvider>
 *       </UIStateProvider>
 *     </FocusLayer>
 *   </SpatialFocusProvider>
 *
 * Mirrors the relevant subset of `App.tsx`'s container hierarchy. The
 * extra providers (DragSession, FileDrop, Schema, etc.) are not needed
 * for the dismiss chain so they are omitted.
 */
function renderChain() {
  return render(
    <SpatialFocusProvider>
      <FocusLayer name={WINDOW_LAYER_NAME}>
        <UIStateProvider>
          <EntityFocusProvider>
            <AppModeProvider>
              <UndoProvider>
                <AppShell>
                  <StackProbe />
                  <InspectorsContainer />
                </AppShell>
              </UndoProvider>
            </AppModeProvider>
          </EntityFocusProvider>
        </UIStateProvider>
      </FocusLayer>
    </SpatialFocusProvider>,
  );
}

/**
 * Read the current `inspector_stack` array from the rendered probe.
 *
 * Reading via the DOM avoids retaining a stale React context reference
 * across re-renders — the probe re-renders on every UIState change so
 * the attribute always reflects the latest state.
 */
function readStackFromDom(container: HTMLElement): string[] {
  const probe = container.querySelector("[data-testid='stack-probe']");
  const raw = probe?.getAttribute("data-stack") ?? "[]";
  return JSON.parse(raw);
}

/** Find the panel zone's `spatial_register_zone` arg by moniker. */
function findPanelZone(moniker: string) {
  return registeredZones.find((z) => z.moniker === moniker);
}

/** Flush microtasks queued by the FocusLayer / FocusZone register effects. */
async function flushSetup() {
  await act(async () => {
    await Promise.resolve();
    await Promise.resolve();
  });
}

/**
 * Dispatch a synthetic `focus-changed` payload through the captured
 * listener so the SpatialFocusProvider records the supplied key as the
 * latest focused [`SpatialKey`]. The chain reads `focusedKey()` from
 * the provider when Escape is pressed.
 */
function emitFocusChanged(key: string | null, moniker: string | null = key) {
  const cb = listenCallbacks["focus-changed"];
  if (!cb) throw new Error("focus-changed listener not captured");
  cb({
    payload: {
      window_label: "main",
      prev_key: null,
      next_key: key,
      // Default the moniker to the key so tests that don't care about
      // the moniker still satisfy the no-silent-dropout contract — the
      // app-shell drill closures need a focused moniker to thread into
      // the kernel call.
      next_moniker: moniker,
    },
  });
}

/** Press Escape on `document` and let the chain settle. */
async function pressEscape() {
  await act(async () => {
    fireEvent.keyDown(document, { key: "Escape", code: "Escape" });
    // Two microtask flushes: one for the dispatch promise, one for the
    // resulting `ui-state-changed` event the React provider applies.
    await Promise.resolve();
    await Promise.resolve();
  });
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("Inspector dismiss chain — Escape closes the inspector", () => {
  beforeEach(() => {
    backendState.inspector_stack = [];
    backendState.palette_open = false;
    registeredZones.length = 0;
    drillOutResponses.clear();
    lastFocusedKey = null;
    for (const k of Object.keys(listenCallbacks)) delete listenCallbacks[k];
    mockInvoke.mockClear();
  });

  // -------------------------------------------------------------------------
  // Inspector-close path — focus at the panel zone
  // -------------------------------------------------------------------------

  it("escape with panel zone focused closes the inspector (CUA)", async () => {
    backendState.inspector_stack = ["task:t1"];

    const { container } = renderChain();
    await flushSetup();
    await waitFor(() => {
      expect(readStackFromDom(container)).toEqual(["task:t1"]);
    });

    // The panel zone must have registered with `parent_zone = null` —
    // that's the kernel's "layer root" shape. Drill-out from this key
    // returns null (matches the Rust kernel test
    // `drill_out_panel_zone_returns_none`).
    const panelZone = findPanelZone("panel:task:t1");
    expect(panelZone, "panel zone must register").toBeDefined();
    expect(panelZone!.parentZone).toBeNull();
    drillOutResponses.set(panelZone!.key, null);

    // Simulate the user clicking on the panel zone (focus claim) — the
    // SpatialFocusProvider records this as the focused key.
    emitFocusChanged(panelZone!.key);

    await pressEscape();

    // The chain fired: `nav.drillOut` returned null (panel is at layer
    // root) → `app.dismiss` was dispatched → DismissCmd popped the
    // panel → `ui-state-changed` updated the React state.
    expect(readStackFromDom(container)).toEqual([]);
  });

  it("escape with panel zone focused closes the inspector (vim normal mode)", async () => {
    // Same chain as the CUA test above. In vim's normal mode (no
    // editor active, no insert mode), Escape is also bound to
    // `nav.drillOut` — see `lib/keybindings.ts`. The chain is keymap-
    // independent in this branch; the test pins it explicitly so a
    // future regression that diverges the vim binding (e.g.
    // re-mapping Escape to a vim-specific command) is caught here.
    backendState.inspector_stack = ["task:t1"];
    // Override get_ui_state to return keymap_mode="vim" before render.
    mockInvoke.mockImplementationOnce(async (cmd: string) => {
      if (cmd === "get_ui_state") {
        return { ...uiStateSnapshot(), keymap_mode: "vim" };
      }
      return null;
    });

    const { container } = renderChain();
    await flushSetup();
    await waitFor(() => {
      expect(readStackFromDom(container)).toEqual(["task:t1"]);
    });

    const panelZone = findPanelZone("panel:task:t1");
    expect(panelZone).toBeDefined();
    drillOutResponses.set(panelZone!.key, null);
    emitFocusChanged(panelZone!.key);

    await pressEscape();

    expect(readStackFromDom(container)).toEqual([]);
  });

  // -------------------------------------------------------------------------
  // Multi-panel — top-only pop
  // -------------------------------------------------------------------------

  it("escape with two panels open closes only the topmost", async () => {
    backendState.inspector_stack = ["task:tA", "task:tB"];

    const { container } = renderChain();
    await flushSetup();
    await waitFor(() => {
      expect(readStackFromDom(container)).toEqual(["task:tA", "task:tB"]);
    });

    // Topmost is panel B — its zone is the layer-root scope drill-out
    // returns null from. Focus there and Escape pops just B.
    const panelB = findPanelZone("panel:task:tB");
    expect(panelB).toBeDefined();
    drillOutResponses.set(panelB!.key, null);
    emitFocusChanged(panelB!.key);

    await pressEscape();

    expect(readStackFromDom(container)).toEqual(["task:tA"]);

    // Second Escape — the new topmost panel A is now at the layer
    // root. Its zone re-registered when its slot moved up.
    const panelAAfter = findPanelZone("panel:task:tA");
    expect(panelAAfter).toBeDefined();
    drillOutResponses.set(panelAAfter!.key, null);
    emitFocusChanged(panelAAfter!.key);

    await pressEscape();

    expect(readStackFromDom(container)).toEqual([]);
  });

  // -------------------------------------------------------------------------
  // Drill-out walks the zone chain before dismissing
  // -------------------------------------------------------------------------

  it("escape with a leaf focused inside the panel walks to the panel zone first, then dismisses on the next press", async () => {
    backendState.inspector_stack = ["task:t1"];
    const { container } = renderChain();
    await flushSetup();

    const panelZone = findPanelZone("panel:task:t1");
    expect(panelZone).toBeDefined();

    // Simulate a leaf inside the panel: drill-out from `leaf:k` returns
    // the panel zone's moniker (the kernel walks one hop up the zone
    // chain). Pinned in `inspector_dismiss.rs::drill_out_field_inside_panel_returns_panel_moniker`.
    drillOutResponses.set("leaf:k", "panel:task:t1");
    drillOutResponses.set(panelZone!.key, null);

    // Focus the synthetic leaf and press Escape. The chain should
    // setFocus to the panel zone moniker — inspector_stack is unchanged.
    emitFocusChanged("leaf:k");
    await pressEscape();
    expect(readStackFromDom(container)).toEqual(["task:t1"]);

    // Re-focus on the panel zone (in production the entity-focus bridge
    // would emit this in response to the setFocus call). Press Escape:
    // now drill-out returns null, and `app.dismiss` fires.
    emitFocusChanged(panelZone!.key);
    await pressEscape();
    expect(readStackFromDom(container)).toEqual([]);
  });

  // -------------------------------------------------------------------------
  // Palette shadows inspector
  // -------------------------------------------------------------------------

  it("escape with palette and inspector both open closes the palette first; second escape closes the inspector", async () => {
    backendState.inspector_stack = ["task:t1"];
    backendState.palette_open = true;

    const { container } = renderChain();
    await flushSetup();
    await waitFor(() => {
      expect(readStackFromDom(container)).toEqual(["task:t1"]);
    });

    // No spatial focus → drillOut closure short-circuits to dismiss.
    // `DismissCmd` then closes the palette first (layer 1).
    emitFocusChanged(null);
    await pressEscape();
    expect(backendState.palette_open).toBe(false);
    expect(readStackFromDom(container)).toEqual(["task:t1"]);

    // Second Escape closes the inspector.
    emitFocusChanged(null);
    await pressEscape();
    expect(readStackFromDom(container)).toEqual([]);
  });

  // -------------------------------------------------------------------------
  // No spatial focus + nothing open — no-op regression guard
  // -------------------------------------------------------------------------

  it("escape with nothing focused, no inspector, no palette is a no-op", async () => {
    const { container } = renderChain();
    await flushSetup();
    expect(readStackFromDom(container)).toEqual([]);

    emitFocusChanged(null);
    await pressEscape();

    // Nothing to dismiss — backend returns Value::Null and emits no
    // state change, so the panel stack stays empty and palette stays
    // closed. Acts as a regression guard: a stray side effect would
    // surface as a state change here.
    expect(readStackFromDom(container)).toEqual([]);
    expect(backendState.palette_open).toBe(false);
  });

  // -------------------------------------------------------------------------
  // Editor / inline-input shadowing — Escape never reaches the document
  // handler when an editable surface is active.
  // -------------------------------------------------------------------------

  /**
   * The document-level keydown handler skips events whose target is an
   * editable surface (`<input>`, `<textarea>`, `<select>`,
   * `[contenteditable]`, `.cm-editor`). That filter is what protects CM
   * editors and inline rename inputs from having their Escape stolen
   * by `nav.drillOut`. The four tests below exercise the filter from
   * each editor flavour by firing Escape with the editable element as
   * the event target — the chain must NOT dispatch `app.dismiss`.
   */
  /**
   * Whether `app.dismiss` was dispatched after the most recent
   * `mockInvoke.mockClear()`. Returns true if the chain reached the
   * dismiss step; false if the document handler bailed out (e.g. an
   * editable target).
   */
  function escapeReachedDismiss(): boolean {
    return mockInvoke.mock.calls.some(
      (c: unknown[]) =>
        c[0] === "dispatch_command" &&
        (c[1] as Record<string, unknown>)?.cmd === "app.dismiss",
    );
  }

  it("escape inside an <input> (inline rename) does not reach the document handler — inspector stays open", async () => {
    backendState.inspector_stack = ["task:t1"];
    const { container } = renderChain();
    await flushSetup();

    // Mount an <input> as a sibling so the keydown's target is an
    // editable surface. The keyhandler's `isEditableTarget` check
    // bails on `<input>` targets — the dispatch must not fire.
    const input = document.createElement("input");
    input.value = "rename";
    document.body.appendChild(input);
    input.focus();

    mockInvoke.mockClear();
    await act(async () => {
      fireEvent.keyDown(input, { key: "Escape", code: "Escape" });
      await Promise.resolve();
    });

    expect(escapeReachedDismiss()).toBe(false);
    expect(readStackFromDom(container)).toEqual(["task:t1"]);

    document.body.removeChild(input);
  });

  it("escape inside a CM editor (.cm-editor) does not reach the document handler — inspector stays open", async () => {
    backendState.inspector_stack = ["task:t1"];
    const { container } = renderChain();
    await flushSetup();

    // The keyhandler's `isEditableTarget` check looks for a
    // `.cm-editor` ancestor. Build a host with the class so Escape on
    // the descendant is treated as editor-owned.
    const host = document.createElement("div");
    host.className = "cm-editor";
    const inner = document.createElement("div");
    host.appendChild(inner);
    document.body.appendChild(host);

    mockInvoke.mockClear();
    await act(async () => {
      fireEvent.keyDown(inner, { key: "Escape", code: "Escape" });
      await Promise.resolve();
    });

    expect(escapeReachedDismiss()).toBe(false);
    expect(readStackFromDom(container)).toEqual(["task:t1"]);

    document.body.removeChild(host);
  });

  it("escape inside a [contenteditable] subtree does not reach the document handler — inspector stays open", async () => {
    backendState.inspector_stack = ["task:t1"];
    const { container } = renderChain();
    await flushSetup();

    // Same shape as the CM-editor case, but driven by the
    // `[contenteditable]` ancestor branch of `isEditableTarget`.
    const host = document.createElement("div");
    host.setAttribute("contenteditable", "true");
    const inner = document.createElement("span");
    host.appendChild(inner);
    document.body.appendChild(host);

    mockInvoke.mockClear();
    await act(async () => {
      fireEvent.keyDown(inner, { key: "Escape", code: "Escape" });
      await Promise.resolve();
    });

    expect(escapeReachedDismiss()).toBe(false);
    expect(readStackFromDom(container)).toEqual(["task:t1"]);

    document.body.removeChild(host);
  });

  // -------------------------------------------------------------------------
  // Drill-out from a non-panel scope walks the zone chain
  // -------------------------------------------------------------------------

  it("escape from a non-panel scope walks zones via drill-out without dismissing", async () => {
    backendState.inspector_stack = ["task:t1"];
    const { container } = renderChain();
    await flushSetup();

    // Imagine the user has spatial focus on a card outside the inspector
    // (drill-out from `task:T1A` returns its parent `column:TODO`, not
    // `null`). The drillOut closure dispatches setFocus, NOT app.dismiss.
    // Inspector stays open — this is the expected per-Escape behavior
    // when focus is not at a layer-root scope.
    drillOutResponses.set("k:card", "column:TODO");
    emitFocusChanged("k:card");

    await pressEscape();

    // Inspector untouched — drillOut returned a non-null moniker, so
    // the closure took the setFocus branch and never fired dismiss.
    expect(readStackFromDom(container)).toEqual(["task:t1"]);
  });

  // -------------------------------------------------------------------------
  // ui.inspect → claim panel-zone focus on mount (card 01KQ9Z9VN6EXM9JWJRNM5T7T19)
  //
  // The `<ClaimPanelFocusOnMount>` helper inside `<InspectorPanel>`'s
  // `<FocusZone>` calls `spatial_focus(panelKey)` once on first mount.
  // Without it, drill-out from Escape walks the source element's zone
  // chain (e.g. `task:T1A` → `column:TODO` → `ui:board` → null →
  // dismiss) before dismiss fires — three Escapes for a card-driven
  // open, two for a navbar-driven open. With it, the kernel's focused
  // key advances to the panel zone immediately and the very first
  // Escape's drill-out lands at the layer root and dismiss fires.
  // -------------------------------------------------------------------------

  /**
   * Dispatch `ui.inspect` against a target moniker by calling through the
   * mocked `invoke` adapter (the same path React's `useDispatchCommand`
   * exercises). Returns once the post-dispatch microtasks have drained,
   * including the `ui-state-changed` re-render that mounts the panel and
   * the `<ClaimPanelFocusOnMount>` helper's deferred `spatial_focus`
   * IPC call.
   *
   * Three microtask flushes:
   *   1. The `await invoke("dispatch_command", …)` continuation — when
   *      the mock returns and the React provider applies
   *      `inspector_stack`.
   *   2. The `<ClaimPanelFocusOnMount>` helper's `useEffect` — fires the
   *      `queueMicrotask(() => focus(panelKey))` deferral.
   *   3. The deferred microtask itself — invokes `spatial_focus` on the
   *      kernel, which the mock replays as a `focus-changed` event.
   */
  async function dispatchInspect(target: string) {
    await act(async () => {
      await mockInvoke("dispatch_command", {
        cmd: "ui.inspect",
        target,
        args: {},
      });
      await Promise.resolve();
      await Promise.resolve();
      await Promise.resolve();
    });
  }

  it("inspect_dispatch_moves_focus_to_panel_zone — ui.inspect on a fresh inspector advances spatial focus to the new panel zone", async () => {
    // Source element focused: a navbar Inspect leaf in the parent
    // window layer (drill-out returns `ui:board` — not null — so
    // without the panel-zone claim, Escape would walk the chain
    // before dismiss).
    const navbarInspectKey = "k:navbar-inspect";
    drillOutResponses.set(navbarInspectKey, "ui:board");

    const { container } = renderChain();
    await flushSetup();
    emitFocusChanged(navbarInspectKey);

    // Sanity: inspector starts empty and the navbar leaf owns focus.
    expect(readStackFromDom(container)).toEqual([]);
    expect(lastFocusedKey).toBeNull();

    // Open the inspector for `task:t1` — the navbar Inspect button's
    // production effect.
    await dispatchInspect("task:t1");

    // The new panel zone has registered, and the
    // `<ClaimPanelFocusOnMount>` helper has fired
    // `spatial_focus(panelKey)`. The mock recorded this as the latest
    // focused key.
    const panelZone = findPanelZone("panel:task:t1");
    expect(panelZone, "panel zone must register").toBeDefined();
    expect(lastFocusedKey).toBe(panelZone!.key);

    // The visible affordance: `data-focused="true"` on the panel zone's
    // outer div, driven by `useFocusClaim` reacting to the replayed
    // `focus-changed` event.
    const panelDiv = container.querySelector(
      "[data-moniker='panel:task:t1']",
    ) as HTMLElement | null;
    expect(panelDiv).not.toBeNull();
    await waitFor(() =>
      expect(panelDiv!.getAttribute("data-focused")).toBe("true"),
    );
  });

  it("escape_after_inspect_closes_in_one_press_from_navbar_open — first Escape after a navbar-driven inspect dismisses the inspector", async () => {
    // Navbar Inspect leaf has focus; drill-out from it returns
    // `ui:board` (the navbar's enclosing zone) — i.e. the source is NOT
    // at a layer root. In the pre-fix world this meant pressing Escape
    // would setFocus to `ui:board`, then a second Escape would drill to
    // null, then a third would dismiss. The new claim moves focus to
    // the panel zone immediately, so the first Escape dismisses.
    const navbarInspectKey = "k:navbar-inspect";
    drillOutResponses.set(navbarInspectKey, "ui:board");
    drillOutResponses.set("ui:board", null);

    const { container } = renderChain();
    await flushSetup();
    emitFocusChanged(navbarInspectKey);

    await dispatchInspect("task:t1");

    const panelZone = findPanelZone("panel:task:t1");
    expect(panelZone).toBeDefined();
    // The kernel's contract: a panel zone is at the layer root, so
    // drill-out from its key returns null and the chain dismisses.
    drillOutResponses.set(panelZone!.key, null);

    // Sanity: focus has moved to the panel zone.
    expect(lastFocusedKey).toBe(panelZone!.key);

    // ONE Escape closes the inspector — that's the contract this test
    // pins.
    await pressEscape();
    expect(readStackFromDom(container)).toEqual([]);
  });

  it("escape_after_inspect_closes_in_one_press_from_card_open — first Escape after a card-driven inspect dismisses the inspector", async () => {
    // Card has focus; drill-out walks two hops (`task:T1A` →
    // `column:TODO` → `ui:board` → null) before dismiss. In the
    // pre-fix world this meant the user had to press Escape three
    // times. With the panel-zone focus claim on mount, the kernel's
    // focused key skips that chain and lands on the panel zone, so
    // ONE Escape dismisses.
    const cardKey = "k:task:T1A";
    drillOutResponses.set(cardKey, "column:TODO");
    drillOutResponses.set("column:TODO", "ui:board");
    drillOutResponses.set("ui:board", null);

    const { container } = renderChain();
    await flushSetup();
    emitFocusChanged(cardKey);

    // Simulate the dblclick → ui.inspect production path. The card
    // moniker `task:T1A` is what `<Inspectable>` would dispatch as the
    // target.
    await dispatchInspect("task:T1A");

    const panelZone = findPanelZone("panel:task:T1A");
    expect(panelZone).toBeDefined();
    drillOutResponses.set(panelZone!.key, null);

    // The claim has moved focus to the panel zone — the source card
    // is no longer the focused key.
    expect(lastFocusedKey).toBe(panelZone!.key);

    // ONE Escape, not three.
    await pressEscape();
    expect(readStackFromDom(container)).toEqual([]);
  });

  it("inspector_close_restores_previous_focus — closing the inspector replays the parent layer's last_focused", async () => {
    // Open the inspector with the card focused; the panel-zone claim
    // moves focus into the inspector layer. Closing the inspector
    // unmounts the inspector layer; the kernel pops the layer and
    // emits the parent layer's `last_focused` (the card's key) as the
    // next focus. The `<SpatialFocusProvider>` records that, the
    // entity-focus bridge mirrors it back into the moniker store, and
    // the chain comes back to where the user started.
    //
    // The kernel-side contract for the layer-pop emit is pinned in the
    // Rust crate's tests; this test pins the React bridge — namely
    // that the React side does not interfere with the restore (e.g.
    // by re-claiming focus on unmount) and that the synthetic restore
    // event flows through the captured `focus-changed` listener so
    // downstream consumers see the card key as the latest focus.
    const cardKey = "k:task:T1A";
    drillOutResponses.set(cardKey, "column:TODO");

    const { container } = renderChain();
    await flushSetup();
    emitFocusChanged(cardKey);

    await dispatchInspect("task:T1A");

    const panelZone = findPanelZone("panel:task:T1A");
    expect(panelZone).toBeDefined();
    drillOutResponses.set(panelZone!.key, null);
    // After the claim, the panel zone is the latest key the kernel
    // moved focus to (`spatial_focus` records into `lastFocusedKey`).
    expect(lastFocusedKey).toBe(panelZone!.key);

    await pressEscape();
    expect(readStackFromDom(container)).toEqual([]);

    // After dismiss the panel is gone from the DOM (the panel zone
    // unregistered as part of the React unmount).
    expect(
      container.querySelector("[data-moniker='panel:task:T1A']"),
    ).toBeNull();

    // The Rust kernel emits a `focus-changed` for the parent layer's
    // `last_focused` after the layer pop. Replay that event through
    // the captured listener — production wiring sends it via
    // `emit_focus_changed` from `spatial_pop_layer` in `commands.rs`.
    // The React side must accept the event without overwriting it: no
    // stray `spatial_focus` IPC fires from the unmount path, and the
    // post-dismiss `focus-changed` carrying the card key is the most
    // recent focus event observed by the provider.
    let postDismissFocusKey: string | null | undefined;
    const cb = listenCallbacks["focus-changed"];
    expect(cb).toBeDefined();
    const wrapped = (event: unknown) => {
      const payload = (event as { payload: { next_key: string | null } })
        .payload;
      postDismissFocusKey = payload.next_key;
      // Forward to the original handler so the provider state stays
      // consistent.
      cb!(event);
    };
    listenCallbacks["focus-changed"] = wrapped;

    emitFocusChanged(cardKey);

    expect(postDismissFocusKey).toBe(cardKey);

    // And no extra `spatial_focus` IPC fired from the React unmount
    // path — the kernel is the source of truth for the restore.
    const focusCallsAfterDismiss = mockInvoke.mock.calls.filter(
      (c) => c[0] === "spatial_focus",
    );
    // The pre-dismiss claim from the panel-mount helper accounts for
    // the only `spatial_focus` IPC we expect in this test.
    expect(focusCallsAfterDismiss).toHaveLength(1);
    expect(focusCallsAfterDismiss[0]![1]).toMatchObject({
      key: panelZone!.key,
    });
  });
});
