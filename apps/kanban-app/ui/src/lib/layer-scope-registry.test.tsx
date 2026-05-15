/**
 * Tests for the React-side `LayerScopeRegistry` — step 1 of the
 * spatial-nav redesign described in card `01KQTC1VNQM9KC90S65P7QX9N1`.
 *
 * The registry is additive in step 1: it lives **alongside** the
 * existing kernel sync, populated on `<FocusScope>` mount and drained on
 * unmount. The kernel sync is unchanged. These tests cover three layers
 * of behaviour:
 *
 * 1. **Direct unit tests** of the `LayerScopeRegistry` class — `add`,
 *    `delete`, `has`, `entries`, and `buildSnapshot`.
 * 2. **Integration tests** that mount real `<FocusLayer>` /
 *    `<FocusScope>` trees and assert the registry tracks mount/unmount
 *    correctly with the right `parentZone` chains and re-render
 *    semantics.
 * 3. **The parity test** — at any moment the registry's FQ set must
 *    equal the kernel-side `spatial_register_scope` /
 *    `spatial_unregister_scope` net set. This is the diagnostic that
 *    proves the dual-source model works before the cutover in later
 *    steps.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, act } from "@testing-library/react";
import { type ReactNode, type RefObject } from "react";

/* -------------------------------------------------------------------------- */
/* Tauri mocks                                                                */
/* -------------------------------------------------------------------------- */

const mockInvoke = vi.fn(
  (..._args: unknown[]): Promise<unknown> => Promise.resolve(),
);
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

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ label: "main" }),
}));

import {
  LayerScopeRegistry,
  useOptionalLayerScopeRegistry,
  type ScopeEntry,
} from "./layer-scope-registry-context";
import { SpatialFocusProvider } from "./spatial-focus-context";
import { FocusLayer } from "@/components/focus-layer";
import { FocusScope } from "@/components/focus-scope";
import {
  asPixels,
  asSegment,
  composeFq,
  fqRoot,
  type FocusOverrides,
  type FullyQualifiedMoniker,
  type SegmentMoniker,
} from "@/types/spatial";

/* -------------------------------------------------------------------------- */
/* Helpers                                                                    */
/* -------------------------------------------------------------------------- */

/** Microtask flush so the provider's `listen()` setup completes. */
async function flushSetup() {
  await act(async () => {
    await Promise.resolve();
  });
}

/**
 * Build a `ScopeEntry` whose `ref.current` is a real DOM node — used
 * by the direct unit tests so `buildSnapshot` can read a live
 * `getBoundingClientRect`. `lastKnownRect` defaults to `null` so unit
 * tests that don't care about the cached rect get the same default
 * `<FocusScope>` produces between mount and the first rect sample.
 */
function makeEntry(
  parentZone: FullyQualifiedMoniker | null = null,
  navOverride?: FocusOverrides,
  segment: SegmentMoniker = asSegment("scope"),
): { entry: ScopeEntry; node: HTMLDivElement } {
  const node = document.createElement("div");
  // happy-dom's getBoundingClientRect returns zeros without explicit
  // layout; the snapshot just records whatever the DOM returns, so we
  // accept zeros for the unit tests.
  const ref: RefObject<HTMLElement | null> = { current: node };
  return {
    entry: { ref, parentZone, navOverride, segment, lastKnownRect: null },
    node,
  };
}

/**
 * Capture every kernel-side `spatial_register_scope` /
 * `spatial_unregister_scope` call as the FQ set the kernel believes is
 * registered. Reset between tests via the `beforeEach` hook below.
 */
function kernelRegisteredFqs(layerFq?: FullyQualifiedMoniker): Set<string> {
  const live = new Set<string>();
  for (const call of mockInvoke.mock.calls) {
    if (call[0] === "spatial_register_scope") {
      const args = call[1] as {
        fq: FullyQualifiedMoniker;
        layerFq: FullyQualifiedMoniker;
      };
      if (layerFq && args.layerFq !== layerFq) continue;
      live.add(args.fq);
    } else if (call[0] === "spatial_unregister_scope") {
      const args = call[1] as { fq: FullyQualifiedMoniker };
      live.delete(args.fq);
    }
  }
  return live;
}

beforeEach(() => {
  mockInvoke.mockClear();
  for (const k of Object.keys(listenHandlers)) delete listenHandlers[k];
});

/* -------------------------------------------------------------------------- */
/* Direct unit tests                                                          */
/* -------------------------------------------------------------------------- */

describe("LayerScopeRegistry (unit)", () => {
  const layerFq = fqRoot(asSegment("window"));

  it("starts empty", () => {
    const reg = new LayerScopeRegistry(layerFq);
    expect(reg.size).toBe(0);
    expect(Array.from(reg.entries())).toEqual([]);
  });

  it("add / has / delete are O(1) and behave like a Map", () => {
    const reg = new LayerScopeRegistry(layerFq);
    const fq = composeFq(layerFq, asSegment("a"));
    const { entry } = makeEntry();

    reg.add(fq, entry);
    expect(reg.has(fq)).toBe(true);
    expect(reg.size).toBe(1);

    reg.delete(fq);
    expect(reg.has(fq)).toBe(false);
    expect(reg.size).toBe(0);
  });

  it("delete is a no-op for unknown FQs", () => {
    const reg = new LayerScopeRegistry(layerFq);
    const ghost = composeFq(layerFq, asSegment("ghost"));
    expect(() => reg.delete(ghost)).not.toThrow();
    expect(reg.size).toBe(0);
  });

  it("onDeleted fires after the entry leaves the map", () => {
    const reg = new LayerScopeRegistry(layerFq);
    const fq = composeFq(layerFq, asSegment("a"));
    const { entry } = makeEntry();
    reg.add(fq, entry);

    let observedSize: number | null = null;
    let observedFq: FullyQualifiedMoniker | null = null;
    const unsubscribe = reg.onDeleted((deletedFq) => {
      observedFq = deletedFq;
      observedSize = reg.size;
    });

    reg.delete(fq);
    // The map must already have shrunk by the time the listener fires —
    // a snapshot built inside the listener correctly excludes `fq`.
    expect(observedFq).toBe(fq);
    expect(observedSize).toBe(0);

    unsubscribe();
  });

  it("onDeleted is silent for delete of an unknown FQ", () => {
    const reg = new LayerScopeRegistry(layerFq);
    const ghost = composeFq(layerFq, asSegment("ghost"));
    const listener = vi.fn();
    reg.onDeleted(listener);

    reg.delete(ghost);
    expect(listener).not.toHaveBeenCalled();
  });

  it("onDeleted unsubscribe stops further notifications", () => {
    const reg = new LayerScopeRegistry(layerFq);
    const fq = composeFq(layerFq, asSegment("a"));
    const { entry } = makeEntry();
    const listener = vi.fn();
    const unsubscribe = reg.onDeleted(listener);
    unsubscribe();

    reg.add(fq, entry);
    reg.delete(fq);
    expect(listener).not.toHaveBeenCalled();
  });

  it("onDeleted isolates listener exceptions: a throwing listener does not stop other listeners and is logged", () => {
    // Pin the documented contract: the registry catches per-listener
    // exceptions so a single misbehaving subscriber cannot break the
    // cleanup path of an unrelated scope. The diagnostic prefix is
    // observable via `console.error` so dev builds surface the
    // misbehaving listener.
    const reg = new LayerScopeRegistry(layerFq);
    const fq = composeFq(layerFq, asSegment("a"));
    const { entry } = makeEntry();
    reg.add(fq, entry);

    const consoleErrorSpy = vi
      .spyOn(console, "error")
      .mockImplementation(() => {});
    const throwingListener = vi.fn(() => {
      throw new Error("listener boom");
    });
    const survivingListener = vi.fn();
    reg.onDeleted(throwingListener);
    reg.onDeleted(survivingListener);

    expect(() => reg.delete(fq)).not.toThrow();

    expect(throwingListener).toHaveBeenCalledTimes(1);
    expect(survivingListener).toHaveBeenCalledTimes(1);
    expect(consoleErrorSpy).toHaveBeenCalledWith(
      "[LayerScopeRegistry] deleted listener threw",
      expect.any(Error),
    );

    consoleErrorSpy.mockRestore();
  });

  it("updateRect caches the rect on the matching entry", () => {
    const reg = new LayerScopeRegistry(layerFq);
    const fq = composeFq(layerFq, asSegment("a"));
    const { entry } = makeEntry();
    reg.add(fq, entry);

    expect(entry.lastKnownRect).toBeNull();

    const rect = {
      x: asPixels(5),
      y: asPixels(6),
      width: asPixels(7),
      height: asPixels(8),
    };
    reg.updateRect(fq, rect);
    expect(entry.lastKnownRect).toEqual(rect);

    // A second call replaces the cached value.
    const newer = {
      x: asPixels(50),
      y: asPixels(60),
      width: asPixels(70),
      height: asPixels(80),
    };
    reg.updateRect(fq, newer);
    expect(entry.lastKnownRect).toEqual(newer);
  });

  it("updateRect is a no-op for an unknown FQ", () => {
    const reg = new LayerScopeRegistry(layerFq);
    const ghost = composeFq(layerFq, asSegment("ghost"));
    const rect = {
      x: asPixels(1),
      y: asPixels(2),
      width: asPixels(3),
      height: asPixels(4),
    };
    expect(() => reg.updateRect(ghost, rect)).not.toThrow();
    expect(reg.size).toBe(0);
  });

  it("delete listener observes the cached rect on the deleted entry", () => {
    const reg = new LayerScopeRegistry(layerFq);
    const fq = composeFq(layerFq, asSegment("a"));
    const { entry } = makeEntry();
    reg.add(fq, entry);

    const cached = {
      x: asPixels(11),
      y: asPixels(22),
      width: asPixels(33),
      height: asPixels(44),
    };
    reg.updateRect(fq, cached);

    let observedRect: typeof cached | null = null;
    reg.onDeleted((_fq, e) => {
      observedRect = e.lastKnownRect as typeof cached | null;
    });
    reg.delete(fq);

    expect(observedRect).toEqual(cached);
  });

  it("re-adding an FQ replaces the previous entry", () => {
    const reg = new LayerScopeRegistry(layerFq);
    const fq = composeFq(layerFq, asSegment("a"));
    const { entry: first } = makeEntry();
    const { entry: second } = makeEntry();

    reg.add(fq, first);
    reg.add(fq, second);

    expect(reg.size).toBe(1);
    const [entryFromMap] = Array.from(reg.entries()).map(([, e]) => e);
    expect(entryFromMap).toBe(second);
  });

  it("buildSnapshot walks every registered entry", () => {
    const reg = new LayerScopeRegistry(layerFq);
    const aFq = composeFq(layerFq, asSegment("a"));
    const bFq = composeFq(layerFq, asSegment("b"));
    const { entry: a } = makeEntry(layerFq);
    const { entry: b } = makeEntry(aFq, { up: null }, asSegment("b"));
    reg.add(aFq, a);
    reg.add(bFq, b);

    const snap = reg.buildSnapshot(layerFq);
    expect(snap.layer_fq).toBe(layerFq);
    expect(snap.scopes.map((s) => s.fq).sort()).toEqual([aFq, bFq].sort());

    const aSnap = snap.scopes.find((s) => s.fq === aFq)!;
    expect(aSnap.parent_zone).toBe(layerFq);
    expect(aSnap.nav_override).toEqual({});

    const bSnap = snap.scopes.find((s) => s.fq === bFq)!;
    expect(bSnap.parent_zone).toBe(aFq);
    expect(bSnap.nav_override).toEqual({ up: null });
  });

  it("buildSnapshot skips entries whose ref.current is null (transient unmount)", () => {
    const reg = new LayerScopeRegistry(layerFq);
    const aFq = composeFq(layerFq, asSegment("a"));
    const bFq = composeFq(layerFq, asSegment("b"));
    const { entry: live } = makeEntry();
    // Create an entry whose ref node is null — simulating the brief
    // window between React scheduling unmount and the cleanup running.
    const detached: ScopeEntry = {
      ref: { current: null },
      parentZone: null,
      segment: asSegment("b"),
      lastKnownRect: null,
    };
    reg.add(aFq, live);
    reg.add(bFq, detached);

    const snap = reg.buildSnapshot(layerFq);
    expect(snap.scopes.map((s) => s.fq)).toEqual([aFq]);
  });

  it("buildSnapshot reads rect at call time, not at register time", () => {
    const reg = new LayerScopeRegistry(layerFq);
    const fq = composeFq(layerFq, asSegment("a"));
    const { entry, node } = makeEntry();

    // Stub `getBoundingClientRect` so the test controls what comes back
    // and asserts the snapshot reflects the *latest* value.
    let rect = { x: 1, y: 2, width: 3, height: 4 };
    node.getBoundingClientRect = () =>
      ({
        x: rect.x,
        y: rect.y,
        width: rect.width,
        height: rect.height,
        top: rect.y,
        left: rect.x,
        right: rect.x + rect.width,
        bottom: rect.y + rect.height,
        toJSON: () => rect,
      }) as DOMRect;

    reg.add(fq, entry);
    const snapBefore = reg.buildSnapshot(layerFq);
    expect(snapBefore.scopes[0].rect).toEqual({
      x: asPixels(1),
      y: asPixels(2),
      width: asPixels(3),
      height: asPixels(4),
    });

    rect = { x: 10, y: 20, width: 30, height: 40 };
    const snapAfter = reg.buildSnapshot(layerFq);
    expect(snapAfter.scopes[0].rect).toEqual({
      x: asPixels(10),
      y: asPixels(20),
      width: asPixels(30),
      height: asPixels(40),
    });
  });
});

/* -------------------------------------------------------------------------- */
/* Context integration                                                        */
/* -------------------------------------------------------------------------- */

function wrapInProviders(children: ReactNode) {
  return (
    <SpatialFocusProvider>
      <FocusLayer name={asSegment("window")}>{children}</FocusLayer>
    </SpatialFocusProvider>
  );
}

/**
 * Render-prop helper: captures the registry the layer publishes so
 * tests can inspect it directly. Sits inside the layer so it sees the
 * inner layer's registry, not a parent's.
 */
function CaptureRegistry({
  out,
}: {
  out: { current: LayerScopeRegistry | null };
}) {
  out.current = useOptionalLayerScopeRegistry();
  return null;
}

describe("<FocusLayer> + LayerScopeRegistry context", () => {
  it("provides a non-null registry inside a layer", async () => {
    const captured: { current: LayerScopeRegistry | null } = { current: null };
    render(
      wrapInProviders(<CaptureRegistry out={captured} />),
    );
    await flushSetup();
    expect(captured.current).not.toBeNull();
    expect(captured.current?.layerFq).toBe(fqRoot(asSegment("window")));
  });

  it("returns null outside any layer", () => {
    const captured: { current: LayerScopeRegistry | null } = { current: null };
    render(
      <SpatialFocusProvider>
        <CaptureRegistry out={captured} />
      </SpatialFocusProvider>,
    );
    expect(captured.current).toBeNull();
  });

  it("nested layers each get their own registry — registries do not cross modal boundaries", async () => {
    const outer: { current: LayerScopeRegistry | null } = { current: null };
    const inner: { current: LayerScopeRegistry | null } = { current: null };

    function CaptureOuter() {
      outer.current = useOptionalLayerScopeRegistry();
      return null;
    }
    function CaptureInner() {
      inner.current = useOptionalLayerScopeRegistry();
      return null;
    }

    render(
      <SpatialFocusProvider>
        <FocusLayer name={asSegment("window")}>
          <CaptureOuter />
          <FocusLayer name={asSegment("inspector")}>
            <CaptureInner />
          </FocusLayer>
        </FocusLayer>
      </SpatialFocusProvider>,
    );
    await flushSetup();

    expect(outer.current).not.toBeNull();
    expect(inner.current).not.toBeNull();
    expect(outer.current).not.toBe(inner.current);
    expect(outer.current?.layerFq).toBe(fqRoot(asSegment("window")));
    expect(inner.current?.layerFq).toBe(
      composeFq(fqRoot(asSegment("window")), asSegment("inspector")),
    );
  });
});

/* -------------------------------------------------------------------------- */
/* <FocusScope> registration via the registry                                 */
/* -------------------------------------------------------------------------- */

/**
 * Helper: render a `<FocusLayer>` whose body holds the given children
 * AND a registry capture so the test can read the registry contents.
 */
function renderWithCapture(children: ReactNode) {
  const captured: { current: LayerScopeRegistry | null } = { current: null };
  const utils = render(
    <SpatialFocusProvider>
      <FocusLayer name={asSegment("window")}>
        <CaptureRegistry out={captured} />
        {children}
      </FocusLayer>
    </SpatialFocusProvider>,
  );
  return { ...utils, captured };
}

describe("<FocusScope> populates its enclosing LayerScopeRegistry", () => {
  it("registers itself on mount with the right FQM", async () => {
    const { captured } = renderWithCapture(
      <FocusScope moniker={asSegment("task:T1")} commands={[]}>
        <span>card</span>
      </FocusScope>,
    );
    await flushSetup();

    const layerFq = fqRoot(asSegment("window"));
    const expectedFq = composeFq(layerFq, asSegment("task:T1"));
    expect(captured.current?.has(expectedFq)).toBe(true);
    expect(captured.current?.size).toBe(1);
  });

  it("captures the right parentZone for nested scopes", async () => {
    // FocusScope itself is the entity-bound "parent zone" for its
    // descendants — every <FocusScope> pushes its FQM to
    // `FocusScopeContext`, and a nested `<FocusScope>` reads that FQM
    // before pushing its own.
    const { captured } = renderWithCapture(
      <FocusScope moniker={asSegment("zone:outer")} commands={[]}>
        <FocusScope moniker={asSegment("leaf:inner")} commands={[]}>
          <span>leaf</span>
        </FocusScope>
      </FocusScope>,
    );
    await flushSetup();

    const layerFq = fqRoot(asSegment("window"));
    const outerFq = composeFq(layerFq, asSegment("zone:outer"));
    const innerFq = composeFq(outerFq, asSegment("leaf:inner"));

    const entries = Array.from(captured.current!.entries());
    const outerEntry = entries.find(([fq]) => fq === outerFq)?.[1];
    const innerEntry = entries.find(([fq]) => fq === innerFq)?.[1];

    expect(outerEntry).toBeDefined();
    expect(innerEntry).toBeDefined();
    // Outer scope is registered directly under the layer — its
    // enclosing focus-scope is `null` because no `<FocusScope>` wraps
    // it.
    expect(outerEntry!.parentZone).toBeNull();
    // Inner scope's enclosing focus-scope IS the outer scope, by
    // FocusScopeContext.
    expect(innerEntry!.parentZone).toBe(outerFq);
  });

  it("removes its entry on unmount", async () => {
    function Toggleable({ show }: { show: boolean }) {
      return show ? (
        <FocusScope moniker={asSegment("task:T1")} commands={[]}>
          <span>card</span>
        </FocusScope>
      ) : null;
    }

    const captured: { current: LayerScopeRegistry | null } = { current: null };
    const { rerender } = render(
      <SpatialFocusProvider>
        <FocusLayer name={asSegment("window")}>
          <CaptureRegistry out={captured} />
          <Toggleable show={true} />
        </FocusLayer>
      </SpatialFocusProvider>,
    );
    await flushSetup();

    const layerFq = fqRoot(asSegment("window"));
    const fq = composeFq(layerFq, asSegment("task:T1"));
    expect(captured.current?.has(fq)).toBe(true);

    rerender(
      <SpatialFocusProvider>
        <FocusLayer name={asSegment("window")}>
          <CaptureRegistry out={captured} />
          <Toggleable show={false} />
        </FocusLayer>
      </SpatialFocusProvider>,
    );
    await flushSetup();

    expect(captured.current?.has(fq)).toBe(false);
    expect(captured.current?.size).toBe(0);
  });

  it("registry shrinks correctly when a subset of scopes unmount", async () => {
    function Three({ which }: { which: Array<"a" | "b" | "c"> }) {
      return (
        <>
          {which.includes("a") && (
            <FocusScope moniker={asSegment("a")} commands={[]}>
              <span>a</span>
            </FocusScope>
          )}
          {which.includes("b") && (
            <FocusScope moniker={asSegment("b")} commands={[]}>
              <span>b</span>
            </FocusScope>
          )}
          {which.includes("c") && (
            <FocusScope moniker={asSegment("c")} commands={[]}>
              <span>c</span>
            </FocusScope>
          )}
        </>
      );
    }

    const captured: { current: LayerScopeRegistry | null } = { current: null };
    const { rerender } = render(
      <SpatialFocusProvider>
        <FocusLayer name={asSegment("window")}>
          <CaptureRegistry out={captured} />
          <Three which={["a", "b", "c"]} />
        </FocusLayer>
      </SpatialFocusProvider>,
    );
    await flushSetup();

    const layerFq = fqRoot(asSegment("window"));
    expect(captured.current?.size).toBe(3);
    expect(captured.current?.has(composeFq(layerFq, asSegment("a")))).toBe(
      true,
    );
    expect(captured.current?.has(composeFq(layerFq, asSegment("b")))).toBe(
      true,
    );
    expect(captured.current?.has(composeFq(layerFq, asSegment("c")))).toBe(
      true,
    );

    rerender(
      <SpatialFocusProvider>
        <FocusLayer name={asSegment("window")}>
          <CaptureRegistry out={captured} />
          <Three which={["a", "c"]} />
        </FocusLayer>
      </SpatialFocusProvider>,
    );
    await flushSetup();

    expect(captured.current?.size).toBe(2);
    expect(captured.current?.has(composeFq(layerFq, asSegment("a")))).toBe(
      true,
    );
    expect(captured.current?.has(composeFq(layerFq, asSegment("b")))).toBe(
      false,
    );
    expect(captured.current?.has(composeFq(layerFq, asSegment("c")))).toBe(
      true,
    );
  });

  it("re-renders with a changed parentZone update the registry entry", async () => {
    // Reparenting a scope: render the leaf either inside an outer
    // <FocusScope> (so `parentZone` is the outer's FQ) or directly
    // under the layer (so `parentZone` is null). The registry entry's
    // `parentZone` should follow the change.
    function Tree({ wrapped }: { wrapped: boolean }) {
      const leaf = (
        <FocusScope moniker={asSegment("leaf")} commands={[]}>
          <span>leaf</span>
        </FocusScope>
      );
      return wrapped ? (
        <FocusScope moniker={asSegment("zone")} commands={[]}>
          {leaf}
        </FocusScope>
      ) : (
        leaf
      );
    }

    const captured: { current: LayerScopeRegistry | null } = { current: null };
    const { rerender } = render(
      <SpatialFocusProvider>
        <FocusLayer name={asSegment("window")}>
          <CaptureRegistry out={captured} />
          <Tree wrapped={true} />
        </FocusLayer>
      </SpatialFocusProvider>,
    );
    await flushSetup();

    const layerFq = fqRoot(asSegment("window"));
    const zoneFq = composeFq(layerFq, asSegment("zone"));
    const wrappedLeafFq = composeFq(zoneFq, asSegment("leaf"));
    const unwrappedLeafFq = composeFq(layerFq, asSegment("leaf"));

    let entries = Array.from(captured.current!.entries());
    let leafEntry = entries.find(([fq]) => fq === wrappedLeafFq)?.[1];
    expect(leafEntry?.parentZone).toBe(zoneFq);

    rerender(
      <SpatialFocusProvider>
        <FocusLayer name={asSegment("window")}>
          <CaptureRegistry out={captured} />
          <Tree wrapped={false} />
        </FocusLayer>
      </SpatialFocusProvider>,
    );
    await flushSetup();

    // The leaf's FQ changed (composeFq target moved), so the registry
    // tracks the new entry under a fresh key. Either way, the
    // `parentZone` reflects the new placement.
    entries = Array.from(captured.current!.entries());
    leafEntry = entries.find(([fq]) => fq === unwrappedLeafFq)?.[1];
    expect(leafEntry).toBeDefined();
    expect(leafEntry!.parentZone).toBeNull();
    // The wrapped variant's entry must be gone — otherwise we'd be
    // leaking stale entries across reparents.
    expect(captured.current!.has(wrappedLeafFq)).toBe(false);
  });

  it("re-renders with a changed navOverride update the registry entry (live-read contract)", async () => {
    // Step 1's deliberate behavior improvement: navOverride changes
    // mid-life ARE visible in the registry, unlike the kernel-sync
    // path which reads it through a ref and ignores changes.
    function Probe({ override }: { override: FocusOverrides | undefined }) {
      return (
        <FocusScope moniker={asSegment("a")} navOverride={override} commands={[]}>
          <span>a</span>
        </FocusScope>
      );
    }

    const captured: { current: LayerScopeRegistry | null } = { current: null };
    const { rerender } = render(
      <SpatialFocusProvider>
        <FocusLayer name={asSegment("window")}>
          <CaptureRegistry out={captured} />
          <Probe override={undefined} />
        </FocusLayer>
      </SpatialFocusProvider>,
    );
    await flushSetup();

    const layerFq = fqRoot(asSegment("window"));
    const fq = composeFq(layerFq, asSegment("a"));
    let entry = Array.from(captured.current!.entries()).find(
      ([k]) => k === fq,
    )?.[1];
    expect(entry?.navOverride).toBeUndefined();

    const newOverride: FocusOverrides = { up: null };
    rerender(
      <SpatialFocusProvider>
        <FocusLayer name={asSegment("window")}>
          <CaptureRegistry out={captured} />
          <Probe override={newOverride} />
        </FocusLayer>
      </SpatialFocusProvider>,
    );
    await flushSetup();

    entry = Array.from(captured.current!.entries()).find(
      ([k]) => k === fq,
    )?.[1];
    expect(entry?.navOverride).toEqual({ up: null });
  });
});

/* -------------------------------------------------------------------------- */
/* Parity test — the diagnostic that proves the dual-source model works       */
/* -------------------------------------------------------------------------- */

describe("LayerScopeRegistry / kernel sync parity", () => {
  it("registry FQ set matches the kernel-side spatial_register_scope net set", async () => {
    // Mount three scopes, then unmount one. After each step, the
    // React-side registry's FQ set must equal the kernel-side net
    // (registers minus unregisters) for the same layer. This is the
    // diagnostic from card 01KQTC1VNQM9KC90S65P7QX9N1's stage 1 plan.
    function Tree({ which }: { which: Array<"a" | "b" | "c"> }) {
      return (
        <>
          {which.includes("a") && (
            <FocusScope moniker={asSegment("a")} commands={[]}>
              <span>a</span>
            </FocusScope>
          )}
          {which.includes("b") && (
            <FocusScope moniker={asSegment("b")} commands={[]}>
              <span>b</span>
            </FocusScope>
          )}
          {which.includes("c") && (
            <FocusScope moniker={asSegment("c")} commands={[]}>
              <span>c</span>
            </FocusScope>
          )}
        </>
      );
    }

    const captured: { current: LayerScopeRegistry | null } = { current: null };
    const layerFq = fqRoot(asSegment("window"));

    const { rerender, unmount } = render(
      <SpatialFocusProvider>
        <FocusLayer name={asSegment("window")}>
          <CaptureRegistry out={captured} />
          <Tree which={["a", "b", "c"]} />
        </FocusLayer>
      </SpatialFocusProvider>,
    );
    await flushSetup();

    // Both views agree at the "all three mounted" snapshot.
    const reactSet1 = new Set(
      Array.from(captured.current!.entries()).map(([fq]) => fq),
    );
    const kernelSet1 = kernelRegisteredFqs(layerFq);
    expect(reactSet1).toEqual(kernelSet1);
    expect(reactSet1.size).toBe(3);

    // Drop "b" — both views must shrink in lockstep.
    rerender(
      <SpatialFocusProvider>
        <FocusLayer name={asSegment("window")}>
          <CaptureRegistry out={captured} />
          <Tree which={["a", "c"]} />
        </FocusLayer>
      </SpatialFocusProvider>,
    );
    await flushSetup();

    const reactSet2 = new Set(
      Array.from(captured.current!.entries()).map(([fq]) => fq),
    );
    const kernelSet2 = kernelRegisteredFqs(layerFq);
    expect(reactSet2).toEqual(kernelSet2);
    expect(reactSet2.size).toBe(2);
    expect(reactSet2.has(composeFq(layerFq, asSegment("b")))).toBe(false);

    // Drop everything.
    rerender(
      <SpatialFocusProvider>
        <FocusLayer name={asSegment("window")}>
          <CaptureRegistry out={captured} />
          <Tree which={[]} />
        </FocusLayer>
      </SpatialFocusProvider>,
    );
    await flushSetup();

    const reactSet3 = new Set(
      Array.from(captured.current!.entries()).map(([fq]) => fq),
    );
    const kernelSet3 = kernelRegisteredFqs(layerFq);
    expect(reactSet3).toEqual(kernelSet3);
    expect(reactSet3.size).toBe(0);

    unmount();
  });

  it("parity holds for a representative kanban-board topology (board → column → card)", async () => {
    // Mirror a realistic kanban scene: a column zone with three card
    // leaves under it. Both sources of truth must agree on the FQ
    // set for the layer.
    const { unmount } = render(
      <SpatialFocusProvider>
        <FocusLayer name={asSegment("window")}>
          <FocusScope moniker={asSegment("column:todo")} commands={[]}>
            <FocusScope moniker={asSegment("card:T1")} commands={[]}>
              <span>T1</span>
            </FocusScope>
            <FocusScope moniker={asSegment("card:T2")} commands={[]}>
              <span>T2</span>
            </FocusScope>
            <FocusScope moniker={asSegment("card:T3")} commands={[]}>
              <span>T3</span>
            </FocusScope>
          </FocusScope>
        </FocusLayer>
      </SpatialFocusProvider>,
    );

    // Capture the registry by re-rendering with the probe in place.
    // We use a separate render path here purely so the assertion has
    // a handle on the registry without changing the topology under
    // test.
    unmount();

    const captured: { current: LayerScopeRegistry | null } = { current: null };
    const layerFq = fqRoot(asSegment("window"));
    mockInvoke.mockClear();

    const { unmount: u2 } = render(
      <SpatialFocusProvider>
        <FocusLayer name={asSegment("window")}>
          <CaptureRegistry out={captured} />
          <FocusScope moniker={asSegment("column:todo")} commands={[]}>
            <FocusScope moniker={asSegment("card:T1")} commands={[]}>
              <span>T1</span>
            </FocusScope>
            <FocusScope moniker={asSegment("card:T2")} commands={[]}>
              <span>T2</span>
            </FocusScope>
            <FocusScope moniker={asSegment("card:T3")} commands={[]}>
              <span>T3</span>
            </FocusScope>
          </FocusScope>
        </FocusLayer>
      </SpatialFocusProvider>,
    );
    await flushSetup();

    const reactSet = new Set(
      Array.from(captured.current!.entries()).map(([fq]) => fq),
    );
    const kernelSet = kernelRegisteredFqs(layerFq);

    expect(reactSet).toEqual(kernelSet);

    const columnFq = composeFq(layerFq, asSegment("column:todo"));
    expect(reactSet.has(columnFq)).toBe(true);
    expect(reactSet.has(composeFq(columnFq, asSegment("card:T1")))).toBe(true);
    expect(reactSet.has(composeFq(columnFq, asSegment("card:T2")))).toBe(true);
    expect(reactSet.has(composeFq(columnFq, asSegment("card:T3")))).toBe(true);

    u2();
  });
});

/* -------------------------------------------------------------------------- */
/* Smoke: the kernel sync is unchanged by step 1                              */
/* -------------------------------------------------------------------------- */

describe("kernel sync untouched by step 1", () => {
  it("still calls spatial_register_scope on mount and spatial_unregister_scope on unmount", async () => {
    function Toggleable({ show }: { show: boolean }) {
      return show ? (
        <FocusScope moniker={asSegment("x")} commands={[]}>
          <span>x</span>
        </FocusScope>
      ) : null;
    }

    const { rerender } = render(
      <SpatialFocusProvider>
        <FocusLayer name={asSegment("window")}>
          <Toggleable show={true} />
        </FocusLayer>
      </SpatialFocusProvider>,
    );
    await flushSetup();

    expect(
      mockInvoke.mock.calls.filter((c) => c[0] === "spatial_register_scope")
        .length,
    ).toBeGreaterThan(0);

    rerender(
      <SpatialFocusProvider>
        <FocusLayer name={asSegment("window")}>
          <Toggleable show={false} />
        </FocusLayer>
      </SpatialFocusProvider>,
    );
    await flushSetup();

    expect(
      mockInvoke.mock.calls.filter((c) => c[0] === "spatial_unregister_scope")
        .length,
    ).toBeGreaterThan(0);
  });
});

