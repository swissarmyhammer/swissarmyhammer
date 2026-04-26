/**
 * Tests for the `<FocusLayer>` primitive.
 *
 * The layer is responsible for:
 *
 *  - Minting a fresh `LayerKey` per instance (stable across re-renders).
 *  - Calling `spatial_push_layer` with `(key, name, parent)` on mount.
 *  - Calling `spatial_pop_layer` on unmount.
 *  - Resolving `parent` via the (prop > ancestor context > null) chain.
 *  - Publishing its key via `FocusLayerContext` so descendants can read it.
 *  - Throwing from `useCurrentLayerKey` when called outside any layer.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { useEffect } from "react";
import { render, act, renderHook } from "@testing-library/react";

const mockInvoke = vi.fn((..._args: unknown[]) => Promise.resolve());
const listenHandlers: Record<string, (event: { payload: unknown }) => void> =
  {};

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn((event: string, handler: (e: { payload: unknown }) => void) => {
    listenHandlers[event] = handler;
    return Promise.resolve(() => {
      delete listenHandlers[event];
    });
  }),
}));

import {
  FocusLayer,
  FocusLayerContext,
  useCurrentLayerKey,
} from "./focus-layer";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { asLayerKey, asLayerName, type LayerKey } from "@/types/spatial";

/** Microtask flush so the provider's `listen()` setup completes. */
async function flushSetup() {
  await act(async () => {
    await Promise.resolve();
  });
}

beforeEach(() => {
  mockInvoke.mockClear();
  for (const k of Object.keys(listenHandlers)) delete listenHandlers[k];
});

/** Pull the (key, name, parent) record from the most recent push call. */
function lastPushArgs() {
  const calls = mockInvoke.mock.calls.filter(
    (c) => c[0] === "spatial_push_layer",
  );
  if (calls.length === 0) throw new Error("expected spatial_push_layer call");
  return calls[calls.length - 1][1] as {
    key: LayerKey;
    name: string;
    parent: LayerKey | null;
  };
}

describe("<FocusLayer>", () => {
  it("pushes a layer with parent=null when mounted at the root", async () => {
    const { unmount } = render(
      <SpatialFocusProvider>
        <FocusLayer name={asLayerName("window")}>{null}</FocusLayer>
      </SpatialFocusProvider>,
    );
    await flushSetup();

    const args = lastPushArgs();
    expect(args.name).toBe("window");
    expect(args.parent).toBeNull();
    expect(typeof args.key).toBe("string");
    expect(args.key.length).toBeGreaterThan(0);

    unmount();
  });

  it("invokes spatial_pop_layer on unmount", async () => {
    const { unmount } = render(
      <SpatialFocusProvider>
        <FocusLayer name={asLayerName("inspector")}>{null}</FocusLayer>
      </SpatialFocusProvider>,
    );
    await flushSetup();

    const pushed = lastPushArgs();
    mockInvoke.mockClear();

    unmount();

    const popCalls = mockInvoke.mock.calls.filter(
      (c) => c[0] === "spatial_pop_layer",
    );
    expect(popCalls).toHaveLength(1);
    expect(popCalls[0][1]).toEqual({ key: pushed.key });
  });

  it("nested layers: child resolves parent from context", async () => {
    let outerKey: LayerKey | null = null;
    function CaptureOuter() {
      const k = useCurrentLayerKey();
      outerKey = k;
      return null;
    }

    const { unmount } = render(
      <SpatialFocusProvider>
        <FocusLayer name={asLayerName("window")}>
          <CaptureOuter />
          <FocusLayer name={asLayerName("inspector")}>{null}</FocusLayer>
        </FocusLayer>
      </SpatialFocusProvider>,
    );
    await flushSetup();

    // Two pushes: window root (parent=null), and inspector (parent=outer.key).
    // React commits child effects before parent effects, so we look up
    // pushes by name rather than relying on call order.
    const pushes = mockInvoke.mock.calls
      .filter((c) => c[0] === "spatial_push_layer")
      .map(
        (c) => c[1] as { key: LayerKey; name: string; parent: LayerKey | null },
      );
    expect(pushes).toHaveLength(2);
    expect(outerKey).not.toBeNull();
    const windowPush = pushes.find((p) => p.name === "window")!;
    const inspectorPush = pushes.find((p) => p.name === "inspector")!;
    expect(windowPush.parent).toBeNull();
    expect(windowPush.key).toBe(outerKey);
    expect(inspectorPush.parent).toBe(outerKey);

    unmount();
  });

  it("explicit parentLayerKey prop overrides the ancestor context", async () => {
    const explicitParent = asLayerKey("explicit-parent-id");

    const { unmount } = render(
      <SpatialFocusProvider>
        <FocusLayer name={asLayerName("window")}>
          <FocusLayer
            name={asLayerName("dialog")}
            parentLayerKey={explicitParent}
          >
            {null}
          </FocusLayer>
        </FocusLayer>
      </SpatialFocusProvider>,
    );
    await flushSetup();

    const pushes = mockInvoke.mock.calls
      .filter((c) => c[0] === "spatial_push_layer")
      .map((c) => c[1] as { name: string; parent: LayerKey | null });
    // The inner ("dialog") layer should ignore the ancestor context.
    const dialog = pushes.find((p) => p.name === "dialog")!;
    expect(dialog.parent).toBe(explicitParent);

    unmount();
  });

  it("explicit parentLayerKey={null} forces a root mount", async () => {
    const { unmount } = render(
      <SpatialFocusProvider>
        <FocusLayer name={asLayerName("window")}>
          <FocusLayer name={asLayerName("dialog")} parentLayerKey={null}>
            {null}
          </FocusLayer>
        </FocusLayer>
      </SpatialFocusProvider>,
    );
    await flushSetup();

    const pushes = mockInvoke.mock.calls
      .filter((c) => c[0] === "spatial_push_layer")
      .map((c) => c[1] as { name: string; parent: LayerKey | null });
    const dialog = pushes.find((p) => p.name === "dialog")!;
    expect(dialog.parent).toBeNull();

    unmount();
  });

  it("publishes the minted key via FocusLayerContext", async () => {
    let observed: LayerKey | null = null;
    function CaptureContext() {
      const k = useCurrentLayerKey();
      observed = k;
      return null;
    }

    const { unmount } = render(
      <SpatialFocusProvider>
        <FocusLayer name={asLayerName("window")}>
          <CaptureContext />
        </FocusLayer>
      </SpatialFocusProvider>,
    );
    await flushSetup();

    const args = lastPushArgs();
    expect(observed).toBe(args.key);

    unmount();
  });

  it("layer key is stable across re-renders", async () => {
    function Bumper({ tick }: { tick: number }) {
      // Force a re-render of the FocusLayer parent without unmounting it.
      useEffect(() => {}, [tick]);
      return <span>tick={tick}</span>;
    }

    const { rerender, unmount } = render(
      <SpatialFocusProvider>
        <FocusLayer name={asLayerName("window")}>
          <Bumper tick={1} />
        </FocusLayer>
      </SpatialFocusProvider>,
    );
    await flushSetup();
    const firstPush = lastPushArgs();

    rerender(
      <SpatialFocusProvider>
        <FocusLayer name={asLayerName("window")}>
          <Bumper tick={2} />
        </FocusLayer>
      </SpatialFocusProvider>,
    );
    await flushSetup();

    // Only one push call should have happened — the same layer instance
    // continues to live across the re-render.
    const pushes = mockInvoke.mock.calls.filter(
      (c) => c[0] === "spatial_push_layer",
    );
    expect(pushes).toHaveLength(1);
    expect(pushes[0][1]).toEqual(firstPush);

    unmount();
  });
});

// ---------------------------------------------------------------------------
// Dialog / palette overlay scenarios
//
// Generalizes the inspector-per-panel layer model to other modal overlays
// (dialogs and the command palette). Exercises the two real-world topologies
// we care about:
//
//   1. Dialog opened from a window-rooted leaf → parent is the window root.
//   2. Dialog opened from inside an inspector panel → parent is the
//      inspector layer, NOT the window root, even though the dialog
//      portals to `document.body` (so the React ancestor chain on the
//      mounted dialog points at the root, not the inspector).
//
// The second case is the whole reason `<FocusLayer>` exposes the
// `parentLayerKey` prop: openers explicitly pass their own layer key
// (read at the call site via `useCurrentLayerKey`) so the layer parent
// reflects the *logical* opener regardless of how the dialog is mounted
// in the DOM.
// ---------------------------------------------------------------------------

describe("<FocusLayer> overlay scenarios", () => {
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

  it("dialog opened from a window-rooted leaf has the window as its parent", async () => {
    // The dialog's opener — which lives directly under the window layer —
    // captures the window's `LayerKey` from `useCurrentLayerKey()` and
    // forwards it to the dialog's `<FocusLayer>` via `parentLayerKey`.
    let openerLayerKey: LayerKey | null = null;
    function Opener() {
      openerLayerKey = useCurrentLayerKey();
      return null;
    }

    const { unmount } = render(
      <SpatialFocusProvider>
        <FocusLayer name={asLayerName("window")}>
          <Opener />
          {/* The dialog renders elsewhere in the React tree (mimicking a
              portal) — but it still receives the window's layer key as
              its explicit parent. */}
        </FocusLayer>
      </SpatialFocusProvider>,
    );
    await flushSetup();

    // Sanity: the opener captured the window's layer key.
    expect(openerLayerKey).not.toBeNull();
    const windowPush = pushedLayers().find((p) => p.name === "window")!;
    expect(windowPush.key).toBe(openerLayerKey);

    // Now mount the dialog as a sibling of the window layer (simulates
    // the portal — the dialog's React parent is NOT the window layer).
    const dialog = render(
      <SpatialFocusProvider>
        <FocusLayer
          name={asLayerName("dialog")}
          parentLayerKey={openerLayerKey}
        >
          {null}
        </FocusLayer>
      </SpatialFocusProvider>,
    );
    await flushSetup();

    const dialogPush = pushedLayers().find((p) => p.name === "dialog")!;
    expect(dialogPush.parent).toBe(openerLayerKey);

    dialog.unmount();
    unmount();
  });

  it("dialog opened from inside an inspector panel has the inspector as its parent", async () => {
    // Two-deep nesting: window → inspector. The opener lives inside the
    // inspector and reads the inspector's layer key. Even when the
    // dialog renders under a non-related parent (the portal target), the
    // explicit `parentLayerKey` keeps it logically rooted at the
    // inspector — exactly what we need for arrow-key capture and
    // `last_focused` restoration on dismiss.
    let inspectorLayerKey: LayerKey | null = null;
    function InspectorOpener() {
      inspectorLayerKey = useCurrentLayerKey();
      return null;
    }

    const { unmount } = render(
      <SpatialFocusProvider>
        <FocusLayer name={asLayerName("window")}>
          <FocusLayer name={asLayerName("inspector")}>
            <InspectorOpener />
          </FocusLayer>
        </FocusLayer>
      </SpatialFocusProvider>,
    );
    await flushSetup();

    expect(inspectorLayerKey).not.toBeNull();
    const inspectorPush = pushedLayers().find((p) => p.name === "inspector")!;
    expect(inspectorPush.key).toBe(inspectorLayerKey);

    // Mount the dialog with the captured inspector key. The dialog is a
    // tree-detached `render` to mirror a portaled overlay — the React
    // ancestor chain at the dialog's mount point has no `<FocusLayer>`
    // at all, so the explicit `parentLayerKey` is the only thing that
    // ties it to the inspector.
    const dialog = render(
      <SpatialFocusProvider>
        <FocusLayer
          name={asLayerName("dialog")}
          parentLayerKey={inspectorLayerKey}
        >
          {null}
        </FocusLayer>
      </SpatialFocusProvider>,
    );
    await flushSetup();

    const dialogPush = pushedLayers().find((p) => p.name === "dialog")!;
    expect(dialogPush.parent).toBe(inspectorLayerKey);
    // And critically, NOT the window root.
    const windowPush = pushedLayers().find((p) => p.name === "window")!;
    expect(dialogPush.parent).not.toBe(windowPush.key);

    dialog.unmount();
    unmount();
  });

  it("palette layer's parent is the window root when opened from app-shell context", async () => {
    // Mirrors the `AppShell` -> `CommandPalette` topology: the palette
    // sits one level deep under the window layer, and explicitly receives
    // the window's key so the portaled overlay roots correctly.
    let windowLayerKey: LayerKey | null = null;
    function PaletteOpener({ open }: { open: boolean }) {
      windowLayerKey = useCurrentLayerKey();
      if (!open) return null;
      return (
        <FocusLayer
          name={asLayerName("palette")}
          parentLayerKey={windowLayerKey}
        >
          {null}
        </FocusLayer>
      );
    }

    const { rerender, unmount } = render(
      <SpatialFocusProvider>
        <FocusLayer name={asLayerName("window")}>
          <PaletteOpener open={false} />
        </FocusLayer>
      </SpatialFocusProvider>,
    );
    await flushSetup();

    // Palette is closed → no palette layer should have been pushed.
    expect(pushedLayers().find((p) => p.name === "palette")).toBeUndefined();

    // Open the palette.
    rerender(
      <SpatialFocusProvider>
        <FocusLayer name={asLayerName("window")}>
          <PaletteOpener open={true} />
        </FocusLayer>
      </SpatialFocusProvider>,
    );
    await flushSetup();

    const palette = pushedLayers().find((p) => p.name === "palette")!;
    expect(palette.parent).toBe(windowLayerKey);

    // Close the palette → its layer is popped.
    mockInvoke.mockClear();
    rerender(
      <SpatialFocusProvider>
        <FocusLayer name={asLayerName("window")}>
          <PaletteOpener open={false} />
        </FocusLayer>
      </SpatialFocusProvider>,
    );
    await flushSetup();

    const pops = mockInvoke.mock.calls.filter(
      (c) => c[0] === "spatial_pop_layer",
    );
    expect(pops).toHaveLength(1);
    expect((pops[0][1] as { key: LayerKey }).key).toBe(palette.key);

    unmount();
  });
});

describe("useCurrentLayerKey", () => {
  it("throws when called outside any <FocusLayer>", () => {
    // renderHook surfaces the throw; assert directly on the call.
    expect(() =>
      renderHook(() => useCurrentLayerKey(), {
        wrapper: ({ children }) => (
          <SpatialFocusProvider>{children}</SpatialFocusProvider>
        ),
      }),
    ).toThrow(/useCurrentLayerKey must be called inside a <FocusLayer>/);
  });

  it("returns the LayerKey provided by FocusLayerContext directly", () => {
    const injected = asLayerKey("injected-key");
    const { result } = renderHook(() => useCurrentLayerKey(), {
      wrapper: ({ children }) => (
        <FocusLayerContext.Provider value={injected}>
          {children}
        </FocusLayerContext.Provider>
      ),
    });
    expect(result.current).toBe(injected);
  });
});
