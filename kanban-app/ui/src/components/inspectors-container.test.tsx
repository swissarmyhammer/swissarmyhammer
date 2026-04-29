import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, fireEvent, act } from "@testing-library/react";

// ---------------------------------------------------------------------------
// Tauri API mocks — must come before component imports.
//
// `mockInvoke` is hoisted so the SpatialFocusProvider's invoke calls
// (`spatial_push_layer`, `spatial_pop_layer`, `spatial_register_zone`, …)
// flow through it and tests can assert against them.
// ---------------------------------------------------------------------------

const mockInvoke = vi.hoisted(() =>
  vi.fn((..._args: unknown[]) => Promise.resolve()),
);

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
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
// Mock useDispatchCommand to capture dispatched commands.
// ---------------------------------------------------------------------------

const mockDispatchClose = vi.hoisted(() => vi.fn(() => Promise.resolve()));
const mockDispatchCloseAll = vi.hoisted(() => vi.fn(() => Promise.resolve()));

vi.mock("@/lib/command-scope", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/command-scope")>();
  return {
    ...actual,
    useDispatchCommand: (cmd: string) => {
      if (cmd === "ui.inspector.close") return mockDispatchClose;
      if (cmd === "ui.inspector.close_all") return mockDispatchCloseAll;
      return vi.fn(() => Promise.resolve());
    },
  };
});

// ---------------------------------------------------------------------------
// Mock useSchema — InspectorPanel uses this internally.
//
// `useRestoreFocus` is intentionally NOT mocked here: the production
// component no longer imports it (per card 01KNQXYC4RBQP1N2NQ33P8DPB9),
// and a `vi.mock` referencing a non-import is the symptom of stale test
// scaffolding. The "no useRestoreFocus" test asserts this directly.
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
import { FileDropProvider } from "@/lib/file-drop-context";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { FocusLayer } from "@/components/focus-layer";
import { asLayerName, type LayerKey } from "@/types/spatial";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Identity-stable layer name for the test window root, matches App.tsx. */
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
 * Render `InspectorsContainer` inside the spatial-focus + window-root
 * layer providers that the production tree mounts in `App.tsx`.
 *
 * `InspectorsContainer` calls `useCurrentLayerKey()` to thread the
 * window-root layer key into the inspector layer's `parentLayerKey`,
 * and the inspector `<FocusLayer>` it renders consumes
 * `useSpatialFocusActions()` for push/pop. Both throw outside the
 * production wrapping, so every render here mirrors `App.tsx`.
 */
function renderInspectors(
  extraWrap?: (node: React.ReactNode) => React.ReactNode,
) {
  const inner = extraWrap ? (
    extraWrap(<InspectorsContainer />)
  ) : (
    <InspectorsContainer />
  );
  return render(
    <SpatialFocusProvider>
      <FocusLayer name={WINDOW_LAYER_NAME}>{inner}</FocusLayer>
    </SpatialFocusProvider>,
  );
}

/** Flush microtasks queued by FocusLayer's push effect and other setup. */
async function flushSetup() {
  await act(async () => {
    await Promise.resolve();
  });
}

/** Pull every `spatial_push_layer` push as a `{ key, name, parent }` record. */
function pushedLayers() {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "spatial_push_layer")
    .map(
      (c) =>
        c[1] as {
          key: LayerKey;
          name: string;
          parent: LayerKey | null;
        },
    );
}

/** Pull every `spatial_pop_layer` pop as a `{ key }` record. */
function poppedLayers() {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "spatial_pop_layer")
    .map((c) => c[1] as { key: LayerKey });
}

/** Pull every `spatial_register_zone` registration. */
function registeredZones() {
  return mockInvoke.mock.calls
    .filter((c) => c[0] === "spatial_register_zone")
    .map(
      (c) =>
        c[1] as {
          key: string;
          moniker: string;
          rect: unknown;
          layerKey: LayerKey;
          parentZone: string | null;
        },
    );
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("InspectorsContainer", () => {
  beforeEach(() => {
    vi.clearAllMocks();
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

  it("renders nothing when inspector_stack is empty", async () => {
    mockUIState.mockReturnValue(uiStateWithStack([]));

    const { container } = renderInspectors();
    await flushSetup();

    // Backdrop should have pointer-events-none (invisible)
    const backdrop = container.querySelector(".fixed.inset-0");
    expect(backdrop?.className).toContain("pointer-events-none");
    // No slide panels
    expect(container.querySelectorAll('[class*="w-[420px]"]').length).toBe(0);
  });

  it("renders a panel for each inspector_stack entry", async () => {
    mockUIState.mockReturnValue(uiStateWithStack(["task:t1", "task:t2"]));

    const { container } = renderInspectors();
    await flushSetup();

    // Two slide panels should be rendered
    const panels = container.querySelectorAll('[class*="w-[420px]"]');
    expect(panels.length).toBe(2);
  });

  it("renders backdrop as visible when panels are open", async () => {
    mockUIState.mockReturnValue(uiStateWithStack(["task:t1"]));

    const { container } = renderInspectors();
    await flushSetup();

    const backdrop = container.querySelector(".fixed.inset-0");
    expect(backdrop?.className).toContain("opacity-100");
    expect(backdrop?.className).not.toContain("pointer-events-none");
  });

  it("dispatches ui.inspector.close_all when backdrop is clicked", async () => {
    mockUIState.mockReturnValue(uiStateWithStack(["task:t1"]));

    const { container } = renderInspectors();
    await flushSetup();

    const backdrop = container.querySelector(".fixed.inset-0");
    fireEvent.click(backdrop!);

    expect(mockDispatchCloseAll).toHaveBeenCalledTimes(1);
  });

  it("stacks panels with correct right offset", async () => {
    mockUIState.mockReturnValue(
      uiStateWithStack(["task:t1", "task:t2", "task:t3"]),
    );

    const { container } = renderInspectors();
    await flushSetup();

    const panels = container.querySelectorAll('[class*="w-[420px]"]');
    expect(panels.length).toBe(3);

    // First panel (t1) is deepest — right offset = (3-1-0)*420 = 840
    expect((panels[0] as HTMLElement).style.right).toBe("840px");
    // Second panel (t2) — right offset = (3-1-1)*420 = 420
    expect((panels[1] as HTMLElement).style.right).toBe("420px");
    // Third panel (t3) is topmost — right offset = 0
    expect((panels[2] as HTMLElement).style.right).toBe("0px");
  });

  it("renders nothing when window state does not exist", async () => {
    // Default mock has no windows entry for "main"
    const { container } = renderInspectors();
    await flushSetup();

    const panels = container.querySelectorAll('[class*="w-[420px]"]');
    expect(panels.length).toBe(0);
  });

  it("receives isDragging from FileDropProvider (drag highlight propagates)", async () => {
    // When InspectorsContainer is inside FileDropProvider (as it should be
    // in App.tsx), the attachment editor in inspector panels receives the
    // isDragging state for drag highlight rendering.
    //
    // This test wraps InspectorsContainer in FileDropProvider with
    // _testOverride and verifies the container renders without error,
    // proving the provider tree is compatible.
    mockUIState.mockReturnValue(uiStateWithStack(["task:t1"]));
    mockEntitiesByType.mockReturnValue({
      task: [
        {
          entity_type: "task",
          id: "t1",
          fields: { title: { String: "Test Task" } },
        },
      ],
    });

    // Wrapping in FileDropProvider with isDragging=true should not error
    const { container } = renderInspectors((node) => (
      <FileDropProvider _testOverride={{ isDragging: true }}>
        {node}
      </FileDropProvider>
    ));
    await flushSetup();

    // Panel should render (one slide panel)
    const panels = container.querySelectorAll('[class*="w-[420px]"]');
    expect(panels.length).toBe(1);

    // If any data-file-drop-zone elements exist (attachment editors),
    // they should have the drag highlight class from the isDragging override.
    const dropZones = container.querySelectorAll("[data-file-drop-zone]");
    for (const zone of dropZones) {
      expect(zone.className).toContain("ring-2");
    }
  });

  it("parses entityType and entityId from moniker strings", async () => {
    mockUIState.mockReturnValue(uiStateWithStack(["board:b1"]));
    mockEntitiesByType.mockReturnValue({
      board: [
        {
          entity_type: "board",
          id: "b1",
          fields: { name: { String: "Test" } },
        },
      ],
    });

    const { container } = renderInspectors();
    await flushSetup();

    // Panel should render (one slide panel)
    const panels = container.querySelectorAll('[class*="w-[420px]"]');
    expect(panels.length).toBe(1);
  });

  // ---------------------------------------------------------------------
  // Spatial-nav: inspector layer push/pop semantics.
  //
  // The container mounts a single `<FocusLayer name="inspector">` when the
  // panel stack is non-empty. Per card `01KQCTJY1QZ710A05SE975GHNR`, the
  // per-panel `<FocusZone moniker="panel:*">` was deleted — the inspector
  // body now renders directly inside `<SlidePanel>` and field zones
  // register at the layer root. These tests pin the layer-level wiring
  // (push when stack non-empty, pop when stack empties); the field-zone
  // registration shape is pinned in
  // `inspector-focus-bridge.layer-barrier.browser.test.tsx`.
  // ---------------------------------------------------------------------

  it("does not push an inspector layer when no panels are open", async () => {
    mockUIState.mockReturnValue(uiStateWithStack([]));

    renderInspectors();
    await flushSetup();

    const inspectorLayers = pushedLayers().filter(
      (l) => l.name === "inspector",
    );
    expect(inspectorLayers).toHaveLength(0);
  });

  it("pushes exactly one inspector layer when the first panel opens", async () => {
    mockUIState.mockReturnValue(uiStateWithStack(["task:t1"]));

    renderInspectors();
    await flushSetup();

    const inspectorLayers = pushedLayers().filter(
      (l) => l.name === "inspector",
    );
    expect(inspectorLayers).toHaveLength(1);

    // The inspector layer's parent is the window-root layer.
    const windowLayer = pushedLayers().find((l) => l.name === "window")!;
    expect(inspectorLayers[0].parent).toBe(windowLayer.key);
  });

  it("opening a second panel does not push another inspector layer", async () => {
    // First mount opens the first panel — pushes window + inspector layers.
    mockUIState.mockReturnValue(uiStateWithStack(["task:t1"]));
    const { rerender } = renderInspectors();
    await flushSetup();

    const inspectorLayersAfterOne = pushedLayers().filter(
      (l) => l.name === "inspector",
    );
    expect(inspectorLayersAfterOne).toHaveLength(1);

    // Open the second panel — re-render with two entries in the stack.
    mockUIState.mockReturnValue(uiStateWithStack(["task:t1", "task:t2"]));
    rerender(
      <SpatialFocusProvider>
        <FocusLayer name={WINDOW_LAYER_NAME}>
          <InspectorsContainer />
        </FocusLayer>
      </SpatialFocusProvider>,
    );
    await flushSetup();

    // Still exactly one inspector layer — no extra layer push. The
    // simplified container does not register a per-panel zone, so the
    // only registrations belong to descendant field zones (covered by
    // the field-zone snapshot test).
    const inspectorLayersAfterTwo = pushedLayers().filter(
      (l) => l.name === "inspector",
    );
    expect(inspectorLayersAfterTwo).toHaveLength(1);

    // No `panel:*` zone is registered — the panel zone was deleted in
    // card `01KQCTJY1QZ710A05SE975GHNR`.
    const panelZones = registeredZones().filter((z) =>
      z.moniker.startsWith("panel:"),
    );
    expect(panelZones).toEqual([]);
  });

  it("closing one of two panels keeps the inspector layer alive", async () => {
    // Open two panels.
    mockUIState.mockReturnValue(uiStateWithStack(["task:t1", "task:t2"]));
    const { rerender } = renderInspectors();
    await flushSetup();

    const inspectorLayer = pushedLayers().find((l) => l.name === "inspector")!;

    // Reset call log so we only see what happens during the close.
    mockInvoke.mockClear();

    // Close the topmost panel (t2) — re-render with only t1 in the stack.
    mockUIState.mockReturnValue(uiStateWithStack(["task:t1"]));
    rerender(
      <SpatialFocusProvider>
        <FocusLayer name={WINDOW_LAYER_NAME}>
          <InspectorsContainer />
        </FocusLayer>
      </SpatialFocusProvider>,
    );
    await flushSetup();

    // The inspector layer should still be alive — no pop_layer for it yet.
    const popsForInspector = poppedLayers().filter(
      (p) => p.key === inspectorLayer.key,
    );
    expect(popsForInspector).toHaveLength(0);
  });

  it("closing the only panel unmounts the inspector layer (pop_layer fires once)", async () => {
    mockUIState.mockReturnValue(uiStateWithStack(["task:t1"]));
    const { rerender } = renderInspectors();
    await flushSetup();

    const inspectorLayer = pushedLayers().find((l) => l.name === "inspector")!;

    // Reset call log so we only see what happens during the close.
    mockInvoke.mockClear();

    // Close the panel — rerender with an empty stack.
    mockUIState.mockReturnValue(uiStateWithStack([]));
    rerender(
      <SpatialFocusProvider>
        <FocusLayer name={WINDOW_LAYER_NAME}>
          <InspectorsContainer />
        </FocusLayer>
      </SpatialFocusProvider>,
    );
    await flushSetup();

    // Exactly one pop targeting the inspector layer's key.
    const popsForInspector = poppedLayers().filter(
      (p) => p.key === inspectorLayer.key,
    );
    expect(popsForInspector).toHaveLength(1);
  });

  // Note: the "production source no longer imports useRestoreFocus"
  // assertion lives in `inspectors-container.guards.node.test.ts`
  // (a Node-only source-level guard), since reading the .tsx file from
  // disk is awkward inside a jsdom suite.
});
