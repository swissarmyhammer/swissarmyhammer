/**
 * Spatial-nav integration tests for `<InspectorsContainer>`.
 *
 * The container layer is shaped as:
 *
 *   `<FocusLayer name="window">`               (App.tsx mounts this)
 *     ↳ `<InspectorsContainer>`                (this component)
 *         ↳ `<FocusLayer name="inspector">`    (mounted while panels are open)
 *             ↳ `<InspectorPanel>` × n         (one per inspector_stack entry)
 *                 ↳ `<SlidePanel>` (`position: fixed`)
 *                     ↳ `<FocusZone moniker="panel:<type>:<id>">`
 *                         ↳ inspector body
 *
 * These tests pin the *focus rendering* and *cross-panel fallback* contracts
 * on top of the structural wiring `inspectors-container.test.tsx` already
 * covers: namely that
 *
 *   1. The panel zone shows a visible `<FocusIndicator>` when the Rust side
 *      reports its `SpatialKey` as the focused key. This is the affordance
 *      the user gets when drill-out lands focus on the panel after Escape
 *      from a field row inside it.
 *
 *   2. With two panels open, focus events that flip the focused key from
 *      panel B's zone to panel A's zone (the `last_focused` fallback the
 *      Rust kernel emits when panel B's zone is unregistered) update the
 *      visible indicator on the right panel — no extra layer push, no
 *      stale indicator on the closed panel.
 *
 * Mocks the Tauri IPC boundary so we can capture the `spatial_register_zone`
 * args (to know which `SpatialKey` each panel zone owns) and dispatch
 * synthetic `focus-changed` events to drive the React-side claim subscription.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, act, waitFor } from "@testing-library/react";

// ---------------------------------------------------------------------------
// Tauri API mocks — must come before component imports.
//
// `mockInvoke` is hoisted so the SpatialFocusProvider's invoke calls
// flow through it. `listenHandlers` captures the `focus-changed` listener
// the provider installs so the tests can dispatch synthetic payloads.
// ---------------------------------------------------------------------------

const mockInvoke = vi.hoisted(() =>
  vi.fn((..._args: unknown[]) => Promise.resolve()),
);

const listenHandlers = vi.hoisted(
  () => ({}) as Record<string, (event: { payload: unknown }) => void>,
);

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(
    (event: string, handler: (e: { payload: unknown }) => void) => {
      listenHandlers[event] = handler;
      return Promise.resolve(() => {
        delete listenHandlers[event];
      });
    },
  ),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    label: "main",
    listen: vi.fn(() => Promise.resolve(() => {})),
  }),
}));

// ---------------------------------------------------------------------------
// Mock useUIState to control inspector_stack from tests.
// ---------------------------------------------------------------------------

const mockUIState = vi.hoisted(() =>
  vi.fn(() => ({
    keymap_mode: "cua",
    scope_chain: [],
    open_boards: [],
    has_clipboard: false,
    clipboard_entity_type: null,
    windows: {},
    recent_boards: [],
  })),
);

vi.mock("@/lib/ui-state-context", () => ({
  useUIState: () => mockUIState(),
}));

// ---------------------------------------------------------------------------
// Mock the close-command dispatchers — these tests don't exercise close,
// but the container still calls `useDispatchCommand("ui.inspector.close")`
// during render so the hook must resolve.
// ---------------------------------------------------------------------------

vi.mock("@/lib/command-scope", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/command-scope")>();
  return {
    ...actual,
    useDispatchCommand: () => vi.fn(() => Promise.resolve()),
  };
});

// ---------------------------------------------------------------------------
// Mock useSchema — InspectorPanel uses this internally.
// ---------------------------------------------------------------------------

vi.mock("@/lib/schema-context", () => ({
  useSchema: () => ({
    getSchema: () => null,
    schemas: {},
    loading: false,
  }),
  useSchemaOptional: () => undefined,
}));

vi.mock("@/lib/entity-focus-context", () => {
  const actions = {
    setFocus: vi.fn(),
    registerScope: vi.fn(),
    unregisterScope: vi.fn(),
    getScope: vi.fn(),
    broadcastNavCommand: vi.fn(),
  };
  return {
    useEntityFocus: () => ({
      focusedMoniker: null,
      setFocusedMoniker: vi.fn(),
    }),
    useFocusActions: () => actions,
    useOptionalFocusActions: () => actions,
    useEntityScopeRegistration: () => {},
    useFocusedMoniker: () => null,
    useFocusedMonikerRef: () => ({ current: null }),
    useIsFocused: () => false,
    useIsDirectFocus: () => false,
    useOptionalIsDirectFocus: () => false,
  };
});

// ---------------------------------------------------------------------------
// Mock RustEngineContainer hook — provides entity store.
// ---------------------------------------------------------------------------

const mockEntitiesByType = vi.hoisted(() =>
  vi.fn<() => Record<string, unknown[]>>(() => ({})),
);

vi.mock("@/components/rust-engine-container", () => ({
  useEntitiesByType: () => mockEntitiesByType(),
}));

// ---------------------------------------------------------------------------
// Import component under test after mocks.
// ---------------------------------------------------------------------------

import { InspectorsContainer } from "./inspectors-container";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { FocusLayer } from "@/components/focus-layer";
import {
  asLayerName,
  type FocusChangedPayload,
  type LayerKey,
  type SpatialKey,
} from "@/types/spatial";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const WINDOW_LAYER_NAME = asLayerName("window");

/** Build a UIState snapshot with a given inspector_stack for the "main" window. */
function uiStateWithStack(stack: string[]) {
  return {
    keymap_mode: "cua",
    scope_chain: [],
    open_boards: [],
    has_clipboard: false,
    clipboard_entity_type: null,
    windows: {
      main: {
        board_path: "/test",
        inspector_stack: stack,
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
 * Render `InspectorsContainer` inside the production-shaped provider stack
 * (`<SpatialFocusProvider>` + `<FocusLayer name="window">`). Mirrors the
 * production wrapping in `App.tsx`.
 */
function renderInspectors() {
  return render(
    <SpatialFocusProvider>
      <FocusLayer name={WINDOW_LAYER_NAME}>
        <InspectorsContainer />
      </FocusLayer>
    </SpatialFocusProvider>,
  );
}

/** Flush microtasks queued by the FocusLayer / FocusZone register effects. */
async function flushSetup() {
  await act(async () => {
    await Promise.resolve();
  });
}

/** Build a `focus-changed` payload with sensible defaults. */
function makePayload(
  overrides: Partial<FocusChangedPayload> = {},
): FocusChangedPayload {
  return {
    window_label: "main" as FocusChangedPayload["window_label"],
    prev_key: null,
    next_key: null,
    next_moniker: null,
    ...overrides,
  };
}

/**
 * Pull every `spatial_register_zone` registration as a typed record. The
 * panel zones go through this path so their `SpatialKey` shows up in the
 * mock's call log and can be threaded into a synthetic `focus-changed`
 * payload.
 */
function registeredZones() {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "spatial_register_zone")
    .map(
      (c) =>
        c[1] as {
          key: SpatialKey;
          moniker: string;
          rect: unknown;
          layerKey: LayerKey;
          parentZone: string | null;
        },
    );
}

/** Pull every `spatial_unregister_scope` call. */
function unregisteredScopes() {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "spatial_unregister_scope")
    .map((c) => c[1] as { key: SpatialKey });
}

/** Find the panel zone registration for `panel:<entityType>:<entityId>`. */
function panelZoneFor(entityType: string, entityId: string) {
  const moniker = `panel:${entityType}:${entityId}`;
  const reg = registeredZones().find((z) => z.moniker === moniker);
  if (!reg) {
    const seen = registeredZones()
      .map((z) => z.moniker)
      .join(", ");
    throw new Error(
      `expected panel zone for "${moniker}"; saw [${seen || "<none>"}]`,
    );
  }
  return reg;
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("InspectorsContainer (spatial-nav focus rendering)", () => {
  beforeEach(() => {
    mockInvoke.mockClear();
    for (const k of Object.keys(listenHandlers)) delete listenHandlers[k];
    mockUIState.mockReturnValue({
      keymap_mode: "cua",
      scope_chain: [],
      open_boards: [],
      has_clipboard: false,
      clipboard_entity_type: null,
      windows: {},
      recent_boards: [],
    });
    mockEntitiesByType.mockReturnValue({});
  });

  it("renders <FocusIndicator> inside the panel zone when the panel's key becomes the focused key", async () => {
    // One panel open. Drill-out from a field row inside that panel
    // returns the panel zone's moniker; the React command sets entity
    // focus and the kernel flips spatial focus to the panel's
    // `SpatialKey`. The panel zone's `useFocusClaim` subscription
    // flips `data-focused` to true and renders `<FocusIndicator>` —
    // that's the visible affordance the user sees when drill-out
    // lands them on the panel.
    mockUIState.mockReturnValue(uiStateWithStack(["task:t1"]));

    const { container } = renderInspectors();
    await flushSetup();

    const panelZone = panelZoneFor("task", "t1");
    const panelDiv = container.querySelector(
      "[data-moniker='panel:task:t1']",
    ) as HTMLElement | null;
    expect(panelDiv).not.toBeNull();
    // Before any focus event, the indicator must NOT be rendered.
    expect(
      panelDiv!.querySelector("[data-testid='focus-indicator']"),
    ).toBeNull();
    expect(panelDiv!.getAttribute("data-focused")).toBeNull();

    // Drive the synthetic focus event for the panel zone's key.
    act(() => {
      listenHandlers["focus-changed"]?.({
        payload: makePayload({ next_key: panelZone.key }),
      });
    });

    await waitFor(() =>
      expect(
        panelDiv!.querySelector("[data-testid='focus-indicator']"),
      ).not.toBeNull(),
    );
    expect(panelDiv!.getAttribute("data-focused")).toBe("true");
  });

  it("the visible indicator follows focus from panel B to panel A on cross-panel fallback", async () => {
    // Two panels open. Focus is on panel B (the topmost). Closing
    // panel B unregisters its zone in the Rust registry; the kernel
    // then routes focus back to panel A via cross-zone leaf fallback
    // (rule 2 of beam search) and emits a `focus-changed` event with
    // `next_key = panel A's key`. From the React side that means:
    //
    //   1. Panel B's `spatial_unregister_scope` fires when its zone
    //      unmounts.
    //   2. Panel A's zone is still registered.
    //   3. The synthetic `focus-changed(prev=B, next=A)` event flips
    //      the visible indicator from panel B (now gone from the DOM)
    //      to panel A.
    //
    // We assert all three; the kernel-side decision to emit
    // `next_key = A.key` lives in the Rust crate's drill / fallback
    // tests.
    mockUIState.mockReturnValue(uiStateWithStack(["task:tA", "task:tB"]));
    const { container, rerender } = renderInspectors();
    await flushSetup();

    const panelA = panelZoneFor("task", "tA");
    const panelB = panelZoneFor("task", "tB");
    expect(panelA.key).not.toBe(panelB.key);

    // Pretend the user has focused panel B.
    act(() => {
      listenHandlers["focus-changed"]?.({
        payload: makePayload({ next_key: panelB.key }),
      });
    });
    const panelBDiv = container.querySelector(
      "[data-moniker='panel:task:tB']",
    ) as HTMLElement | null;
    expect(panelBDiv).not.toBeNull();
    await waitFor(() =>
      expect(
        panelBDiv!.querySelector("[data-testid='focus-indicator']"),
      ).not.toBeNull(),
    );

    // Close panel B — re-render with only panel A's entry in the
    // inspector_stack. The React side should call
    // `spatial_unregister_scope(panel B's key)` as the FocusZone
    // wrapper unmounts.
    mockUIState.mockReturnValue(uiStateWithStack(["task:tA"]));
    rerender(
      <SpatialFocusProvider>
        <FocusLayer name={WINDOW_LAYER_NAME}>
          <InspectorsContainer />
        </FocusLayer>
      </SpatialFocusProvider>,
    );
    await flushSetup();

    const unregisteredKeys = unregisteredScopes().map((u) => u.key);
    expect(unregisteredKeys).toContain(panelB.key);
    // Panel A's zone is still registered (we never see its key in the
    // unregister log) — that's what lets the kernel route fallback
    // focus to it.
    expect(unregisteredKeys).not.toContain(panelA.key);

    // Now simulate the kernel's `last_focused` fallback: emit
    // `focus-changed(prev=B, next=A)`. Panel A's claim subscription
    // must fire and the visible indicator must follow.
    act(() => {
      listenHandlers["focus-changed"]?.({
        payload: makePayload({ prev_key: panelB.key, next_key: panelA.key }),
      });
    });

    const panelADiv = container.querySelector(
      "[data-moniker='panel:task:tA']",
    ) as HTMLElement | null;
    expect(panelADiv).not.toBeNull();
    await waitFor(() =>
      expect(
        panelADiv!.querySelector("[data-testid='focus-indicator']"),
      ).not.toBeNull(),
    );
    expect(panelADiv!.getAttribute("data-focused")).toBe("true");

    // Panel B is gone from the DOM entirely.
    expect(
      container.querySelector("[data-moniker='panel:task:tB']"),
    ).toBeNull();
  });

  it("clicking the panel body (not just the inspector body) focuses the panel zone", async () => {
    // The panel zone's `<FocusZone>` lives inside the SlidePanel and
    // is sized with `min-h-full` so a click anywhere on the panel
    // content area routes through the FocusZone's click handler — not
    // just on the inspector body itself. The handler invokes
    // `spatial_focus(panel-zone-key)` so the kernel knows to update
    // focus to the panel.
    mockUIState.mockReturnValue(uiStateWithStack(["task:t1"]));
    const { container } = renderInspectors();
    await flushSetup();

    const panelZone = panelZoneFor("task", "t1");
    const panelDiv = container.querySelector(
      "[data-moniker='panel:task:t1']",
    ) as HTMLElement | null;
    expect(panelDiv).not.toBeNull();

    // Reset call log so we only see the click's effect.
    mockInvoke.mockClear();
    act(() => {
      panelDiv!.click();
    });

    const focusCalls = mockInvoke.mock.calls.filter(
      (c) => c[0] === "spatial_focus",
    );
    expect(focusCalls.length).toBeGreaterThan(0);
    expect(focusCalls[0]?.[1]).toMatchObject({ key: panelZone.key });
  });
});
