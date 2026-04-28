/**
 * Browser-mode test pinning the "Enter on a focused inspector panel zone
 * drills into the panel's first child" contract.
 *
 * Source of truth for card `01KQ9X3A9NMRYK50GWP4S4ZMJ4`. With the
 * `board.inspect: vim Enter` binding gone (see card
 * `01KQ9XJ4XGKVW24EZSQCA6K3E2`), Enter on a focused panel zone resolves
 * via the global `nav.drillIn` command — its execute closure reads the
 * focused `SpatialKey`, awaits `spatial_drill_in`, and on a non-null
 * moniker dispatches `setFocus(moniker)`. This test pins the React-side
 * wiring: when the panel zone is focused and the kernel resolves
 * drill-in to the first inspector field zone's moniker, the bridge
 * fans the resolved moniker out via `dispatch_command(ui.setFocus, …)`.
 *
 * The kernel-side resolution (panel zone → first field zone) is pinned
 * in the Rust crate's drill-in tests; this file pins the React side
 * that consumes whatever moniker the kernel returns.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, fireEvent, act, waitFor } from "@testing-library/react";

// ---------------------------------------------------------------------------
// Tauri API mocks — must be installed before importing components.
// ---------------------------------------------------------------------------

interface MutableUIState {
  inspector_stack: string[];
  palette_open: boolean;
}

const backendState: MutableUIState = {
  inspector_stack: [],
  palette_open: false,
};

const listenCallbacks: Record<string, (event: unknown) => void> = {};

function uiStateSnapshot() {
  return {
    keymap_mode: "vim",
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
 * Drill-in responses keyed by SpatialKey. Tests set entries here so the
 * mock kernel returns the right child moniker for the panel-zone key.
 */
const drillInResponses = new Map<string, string | null>();

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
      const cb = listenCallbacks["focus-changed"];
      if (cb) {
        cb({
          payload: {
            window_label: "main",
            prev_key: null,
            next_key: (args?.key as string) ?? null,
            next_moniker: null,
          },
        });
      }
      return null;
    }

    if (cmd === "spatial_drill_in") {
      const key = (args?.key as string) ?? "";
      return drillInResponses.has(key) ? drillInResponses.get(key)! : null;
    }

    if (cmd === "spatial_drill_out") return null;
    if (cmd === "spatial_navigate") return null;
    if (cmd === "log_command") return null;

    if (cmd === "list_commands_for_scope") return [];
    if (cmd === "list_views") return [];
    if (cmd === "list_perspectives") return [];

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
      // We just observe the dispatch records — no side effects needed.
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

// Schema / engine mocks — the chain only needs InspectorsContainer's
// shape (panel zone registration + UIState reactive read), not the full
// inspector body.
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
  RustEngineContainer: ({ children }: { children: React.ReactNode }) => children,
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
import { UIStateProvider } from "@/lib/ui-state-context";
import { AppModeProvider } from "@/lib/app-mode-context";
import { UndoProvider } from "@/lib/undo-context";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { asLayerName } from "@/types/spatial";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const WINDOW_LAYER_NAME = asLayerName("window");

function renderChain() {
  return render(
    <SpatialFocusProvider>
      <FocusLayer name={WINDOW_LAYER_NAME}>
        <UIStateProvider>
          <EntityFocusProvider>
            <AppModeProvider>
              <UndoProvider>
                <AppShell>
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

function findPanelZone(moniker: string) {
  return registeredZones.find((z) => z.moniker === moniker);
}

async function flushSetup() {
  await act(async () => {
    await Promise.resolve();
    await Promise.resolve();
  });
}

async function emitFocusChanged(
  key: string | null,
  moniker: string | null = null,
) {
  const cb = listenCallbacks["focus-changed"];
  if (!cb) throw new Error("focus-changed listener not captured");
  await act(async () => {
    cb({
      payload: {
        window_label: "main",
        prev_key: null,
        next_key: key,
        next_moniker: moniker,
      },
    });
    await Promise.resolve();
  });
}

async function pressEnter() {
  await act(async () => {
    fireEvent.keyDown(document, { key: "Enter", code: "Enter" });
    await Promise.resolve();
    await Promise.resolve();
  });
}

/** Filter dispatch_command calls down to those for `ui.setFocus`. */
function setFocusDispatches(): Array<Record<string, unknown>> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "dispatch_command")
    .map((c) => c[1] as Record<string, unknown>)
    .filter((p) => p.cmd === "ui.setFocus");
}

/** Filter `spatial_drill_in` calls. */
function spatialDrillInCalls(): Array<{ key: string }> {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "spatial_drill_in")
    .map((c) => c[1] as { key: string });
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("InspectorsContainer — Enter on focused panel zone drills in", () => {
  beforeEach(() => {
    backendState.inspector_stack = [];
    backendState.palette_open = false;
    registeredZones.length = 0;
    drillInResponses.clear();
    for (const k of Object.keys(listenCallbacks)) delete listenCallbacks[k];
    mockInvoke.mockClear();
  });

  it("enter_on_focused_panel_zone_drills_into_first_field", async () => {
    backendState.inspector_stack = ["task:t1"];

    renderChain();
    await flushSetup();
    // Allow the panel mount + zone register effects to settle. The
    // `<ClaimPanelFocusOnMount>` helper queues a focus call via
    // `queueMicrotask`; let it drain before the test drives focus.
    await act(async () => {
      await new Promise((r) => setTimeout(r, 50));
    });

    const panelZone = findPanelZone("panel:task:t1");
    await waitFor(() => {
      expect(
        findPanelZone("panel:task:t1"),
        "panel zone must register on mount",
      ).toBeDefined();
    });
    expect(panelZone).toBeDefined();

    // Stub the kernel's drill-in for the panel zone key — return the
    // moniker of the first inspector field zone the panel is meant
    // to drill into (`field:task:t1.title` is the structural first
    // child the kernel would resolve to in production).
    drillInResponses.set(panelZone!.key, "field:task:t1.title");

    // Drive a focus-changed event for the panel zone so:
    //   1. `SpatialFocusProvider`'s `focusedKeyRef` records the panel
    //      key (the `nav.drillIn` execute closure reads this).
    //   2. The entity-focus bridge mirrors the panel moniker into the
    //      entity-focus store so `extractScopeBindings` walks the
    //      panel scope chain when resolving Enter.
    await emitFocusChanged(panelZone!.key, "panel:task:t1");
    await flushSetup();

    // Reset the spy so we measure only the keystroke's IPC trace.
    mockInvoke.mockClear();

    await pressEnter();
    await flushSetup();

    // The drill closure dispatched `spatial_drill_in` for the focused
    // panel key.
    const drillCalls = spatialDrillInCalls();
    expect(
      drillCalls.length,
      "vim Enter on the focused panel zone must dispatch spatial_drill_in once",
    ).toBe(1);
    expect(drillCalls[0].key).toBe(panelZone!.key);

    // The closure's success branch fanned out via setFocus → the
    // entity-focus bridge dispatched `ui.setFocus` whose
    // `args.scope_chain` opens with the resolved moniker.
    const setFocusCalls = setFocusDispatches();
    expect(
      setFocusCalls.length,
      "the drill closure must dispatch ui.setFocus for the resolved moniker",
    ).toBeGreaterThanOrEqual(1);
    const targetCall = setFocusCalls.find((c) => {
      const args = c.args as { scope_chain?: string[] } | undefined;
      return args?.scope_chain?.[0] === "field:task:t1.title";
    });
    expect(
      targetCall,
      "ui.setFocus dispatch must carry the panel's first field moniker at the head of args.scope_chain",
    ).toBeTruthy();
  });
});
