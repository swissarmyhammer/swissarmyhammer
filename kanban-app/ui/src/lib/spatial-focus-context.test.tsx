/**
 * Tests for `SpatialFocusProvider` — the per-window claim registry that
 * mirrors the Rust `SpatialState` over the `focus-changed` Tauri event.
 *
 * Coverage targets the four React-side test cases listed on the spatial-
 * nav focus-claim card:
 *
 * - Claim registry ignores events for unknown keys.
 * - Scope click invokes `spatial_focus` with its branded `FullyQualifiedMoniker`.
 * - Provider unmount removes the listener.
 * - Scope unmount removes from claim registry.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, render, act } from "@testing-library/react";
import { useEffect, type ReactNode } from "react";

/* ---- Tauri mocks ---- */

const mockInvoke = vi.fn(
  (..._args: unknown[]): Promise<unknown> => Promise.resolve(),
);
let listenHandlers: Record<string, (event: { payload: unknown }) => void> = {};
let listenUnsubscribers: Record<string, ReturnType<typeof vi.fn>> = {};

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn((event: string, handler: (e: { payload: unknown }) => void) => {
    listenHandlers[event] = handler;
    const unlisten = vi.fn(() => {
      delete listenHandlers[event];
    });
    listenUnsubscribers[event] = unlisten;
    return Promise.resolve(unlisten);
  }),
}));

import {
  SpatialFocusProvider,
  useFocusClaim,
  useSpatialFocusActions,
} from "./spatial-focus-context";
import { LayerScopeRegistry } from "./layer-scope-registry-context";
import type {
  FocusChangedPayload,
  Rect,
  FullyQualifiedMoniker,
  SegmentMoniker,
} from "@/types/spatial";
import { asFq, asSegment, asPixels } from "@/types/spatial";

/* ---- Helpers ---- */

function wrapper({ children }: { children: ReactNode }) {
  return <SpatialFocusProvider>{children}</SpatialFocusProvider>;
}

function makePayload(
  overrides: Partial<FocusChangedPayload> = {},
): FocusChangedPayload {
  return {
    window_label: "main" as FocusChangedPayload["window_label"],
    prev_fq: null,
    next_fq: null,
    next_segment: null,
    ...overrides,
  };
}

/**
 * Wait one microtask for the provider's `listen()` promise to resolve and
 * register its handler in the mock map. The provider's `useEffect` calls
 * `listen(...).then(...)` to capture the unlisten callback; the next tick
 * is when the handler is reachable from `listenHandlers["focus-changed"]`.
 */
async function flushListenSetup() {
  await act(async () => {
    await Promise.resolve();
  });
}

beforeEach(() => {
  mockInvoke.mockClear();
  listenHandlers = {};
  listenUnsubscribers = {};
});

/* ---- Tests ---- */

describe("SpatialFocusProvider", () => {
  it("ignores focus-changed events for unknown keys", async () => {
    const claimSpy = vi.fn();
    const knownKey: FullyQualifiedMoniker = asFq("known");

    const { unmount } = renderHook(() => useFocusClaim(knownKey, claimSpy), {
      wrapper,
    });
    await flushListenSetup();

    // Dispatch an event whose `next_fq` is NOT registered. The unknown
    // lookup must be a silent no-op — no claim fires anywhere.
    act(() => {
      listenHandlers["focus-changed"]?.({
        payload: makePayload({ next_fq: asFq("ghost") }),
      });
    });

    expect(claimSpy).not.toHaveBeenCalled();

    // The known scope can still receive its own events — sanity-check
    // that the registry isn't broken by the unknown lookup.
    act(() => {
      listenHandlers["focus-changed"]?.({
        payload: makePayload({ next_fq: knownKey }),
      });
    });
    expect(claimSpy).toHaveBeenCalledWith(true);

    unmount();
  });

  it("invokes spatial_focus with the branded FullyQualifiedMoniker on focus()", async () => {
    const { result, unmount } = renderHook(() => useSpatialFocusActions(), {
      wrapper,
    });
    await flushListenSetup();

    const key: FullyQualifiedMoniker = asFq("01ABC");
    await act(async () => {
      await result.current.focus(key);
    });

    expect(mockInvoke).toHaveBeenCalledWith(
      "spatial_focus",
      expect.objectContaining({ fq: key }),
    );

    unmount();
  });

  it("invokes spatial_register_scope with the full kernel-types record", async () => {
    const { result, unmount } = renderHook(() => useSpatialFocusActions(), {
      wrapper,
    });
    await flushListenSetup();

    const key: FullyQualifiedMoniker = asFq("k1");
    const moniker: SegmentMoniker = asSegment("task:01ABC");
    const rect: Rect = {
      x: asPixels(0),
      y: asPixels(0),
      width: asPixels(100),
      height: asPixels(50),
    };
    const layerKey: FullyQualifiedMoniker = asFq("L1");
    await act(async () => {
      await result.current.registerScope(
        key,
        moniker,
        rect,
        layerKey,
        null,
        {},
      );
    });

    expect(mockInvoke).toHaveBeenCalledWith("spatial_register_scope", {
      fq: key,
      segment: moniker,
      rect,
      layerFq: layerKey,
      parentZone: null,
      overrides: {},
    });

    unmount();
  });

  it("removes the focus-changed listener on provider unmount", async () => {
    const { unmount } = render(
      <SpatialFocusProvider>{null}</SpatialFocusProvider>,
    );
    await flushListenSetup();

    const unlisten = listenUnsubscribers["focus-changed"];
    expect(unlisten).toBeDefined();
    expect(unlisten).not.toHaveBeenCalled();

    unmount();
    expect(unlisten).toHaveBeenCalledTimes(1);
    expect(listenHandlers["focus-changed"]).toBeUndefined();
  });

  it("removes a scope from the claim registry on unmount", async () => {
    const claimSpy = vi.fn();
    const key: FullyQualifiedMoniker = asFq("scope-1");

    function Probe() {
      useFocusClaim(key, claimSpy);
      return null;
    }

    let actionsRef: ReturnType<typeof useSpatialFocusActions> | null = null;
    function Inspector() {
      actionsRef = useSpatialFocusActions();
      return null;
    }

    const { rerender } = render(
      <SpatialFocusProvider>
        <Inspector />
        <Probe />
      </SpatialFocusProvider>,
    );
    await flushListenSetup();

    // Confirm the claim is in the registry.
    expect(actionsRef!.hasClaim(key)).toBe(true);

    // Unmount only the Probe, leaving the provider mounted.
    rerender(
      <SpatialFocusProvider>
        <Inspector />
      </SpatialFocusProvider>,
    );

    expect(actionsRef!.hasClaim(key)).toBe(false);

    // After unmount, dispatching the same event must not call the claim.
    act(() => {
      listenHandlers["focus-changed"]?.({
        payload: makePayload({ next_fq: key }),
      });
    });
    expect(claimSpy).not.toHaveBeenCalled();
  });

  it("dispatches false to prev_fq and true to next_fq on focus transfer", async () => {
    const aKey: FullyQualifiedMoniker = asFq("a");
    const bKey: FullyQualifiedMoniker = asFq("b");
    const aSpy = vi.fn();
    const bSpy = vi.fn();

    function Probes() {
      useFocusClaim(aKey, aSpy);
      useFocusClaim(bKey, bSpy);
      return null;
    }

    const { unmount } = render(
      <SpatialFocusProvider>
        <Probes />
      </SpatialFocusProvider>,
    );
    await flushListenSetup();

    act(() => {
      listenHandlers["focus-changed"]?.({
        payload: makePayload({ prev_fq: aKey, next_fq: bKey }),
      });
    });

    expect(aSpy).toHaveBeenCalledWith(false);
    expect(bSpy).toHaveBeenCalledWith(true);

    unmount();
  });

  it("does not break when a registered scope re-registers under the same key", async () => {
    const key: FullyQualifiedMoniker = asFq("reused");
    const firstSpy = vi.fn();
    const secondSpy = vi.fn();

    function Probe({ listener }: { listener: (focused: boolean) => void }) {
      useFocusClaim(key, listener);
      return null;
    }

    const { rerender, unmount } = render(
      <SpatialFocusProvider>
        <Probe listener={firstSpy} />
      </SpatialFocusProvider>,
    );
    await flushListenSetup();

    // Render-pass switching to a new listener should leave the latest one
    // active; the stable shim reads through a ref so re-registration is
    // not necessary.
    rerender(
      <SpatialFocusProvider>
        <Probe listener={secondSpy} />
      </SpatialFocusProvider>,
    );

    act(() => {
      listenHandlers["focus-changed"]?.({
        payload: makePayload({ next_fq: key }),
      });
    });

    expect(firstSpy).not.toHaveBeenCalled();
    expect(secondSpy).toHaveBeenCalledWith(true);

    unmount();
  });

  it("invokes spatial_navigate with the direction string literal", async () => {
    const { result, unmount } = renderHook(() => useSpatialFocusActions(), {
      wrapper,
    });
    await flushListenSetup();

    const key: FullyQualifiedMoniker = asFq("nav-from");
    await act(async () => {
      await result.current.navigate(key, "right");
    });

    // No layer registry has been registered for this FQM, so the
    // snapshot field is `undefined` — the kernel falls back to the
    // registry path.
    expect(mockInvoke).toHaveBeenCalledWith("spatial_navigate", {
      focusedFq: key,
      direction: "right",
      snapshot: undefined,
    });

    unmount();
  });

  it("invokes spatial_navigate with a populated snapshot when the focused FQ is in a registered layer registry", async () => {
    const { result, unmount } = renderHook(() => useSpatialFocusActions(), {
      wrapper,
    });
    await flushListenSetup();

    const layerFq: FullyQualifiedMoniker = asFq("/window");
    const focused: FullyQualifiedMoniker = asFq("/window/zone/leaf-a");
    const sibling: FullyQualifiedMoniker = asFq("/window/zone/leaf-b");
    const zone: FullyQualifiedMoniker = asFq("/window/zone");

    const focusedNode = document.createElement("div");
    focusedNode.getBoundingClientRect = () =>
      ({ x: 10, y: 20, width: 30, height: 40, top: 20, right: 40, bottom: 60, left: 10, toJSON: () => "" }) as DOMRect;
    const siblingNode = document.createElement("div");
    siblingNode.getBoundingClientRect = () =>
      ({ x: 100, y: 20, width: 30, height: 40, top: 20, right: 130, bottom: 60, left: 100, toJSON: () => "" }) as DOMRect;

    const registry = new LayerScopeRegistry(layerFq);
    registry.add(focused, {
      ref: { current: focusedNode },
      parentZone: zone,
      navOverride: {},
      segment: asSegment("leaf-a"),
    });
    registry.add(sibling, {
      ref: { current: siblingNode },
      parentZone: zone,
      navOverride: {},
      segment: asSegment("leaf-b"),
    });

    const dispose = result.current.registerLayerRegistry(layerFq, registry);

    await act(async () => {
      await result.current.navigate(focused, "right");
    });

    expect(mockInvoke).toHaveBeenCalledWith(
      "spatial_navigate",
      expect.objectContaining({
        focusedFq: focused,
        direction: "right",
        snapshot: expect.objectContaining({
          layer_fq: layerFq,
          scopes: expect.arrayContaining([
            expect.objectContaining({
              fq: focused,
              parent_zone: zone,
              nav_override: {},
            }),
            expect.objectContaining({
              fq: sibling,
              parent_zone: zone,
              nav_override: {},
            }),
          ]),
        }),
      }),
    );

    dispose();
    unmount();
  });

  it("invokes spatial_focus with a populated snapshot when the target FQ is in a registered layer registry", async () => {
    const { result, unmount } = renderHook(() => useSpatialFocusActions(), {
      wrapper,
    });
    await flushListenSetup();

    const layerFq: FullyQualifiedMoniker = asFq("/window");
    const target: FullyQualifiedMoniker = asFq("/window/zone/card");
    const zone: FullyQualifiedMoniker = asFq("/window/zone");

    const targetNode = document.createElement("div");
    targetNode.getBoundingClientRect = () =>
      ({ x: 10, y: 20, width: 30, height: 40, top: 20, right: 40, bottom: 60, left: 10, toJSON: () => "" }) as DOMRect;

    const registry = new LayerScopeRegistry(layerFq);
    registry.add(target, {
      ref: { current: targetNode },
      parentZone: zone,
      navOverride: {},
      segment: asSegment("card"),
    });

    const dispose = result.current.registerLayerRegistry(layerFq, registry);

    await act(async () => {
      await result.current.focus(target);
    });

    expect(mockInvoke).toHaveBeenCalledWith(
      "spatial_focus",
      expect.objectContaining({
        fq: target,
        snapshot: expect.objectContaining({
          layer_fq: layerFq,
          scopes: expect.arrayContaining([
            expect.objectContaining({
              fq: target,
              parent_zone: zone,
            }),
          ]),
        }),
      }),
    );

    dispose();
    unmount();
  });

  it("invokes spatial_focus with snapshot: undefined when no layer registry contains the target FQ", async () => {
    const { result, unmount } = renderHook(() => useSpatialFocusActions(), {
      wrapper,
    });
    await flushListenSetup();

    const target: FullyQualifiedMoniker = asFq("/window/orphan");

    await act(async () => {
      await result.current.focus(target);
    });

    expect(mockInvoke).toHaveBeenCalledWith("spatial_focus", {
      fq: target,
      snapshot: undefined,
    });

    unmount();
  });

  it("popLayer rounds-tripping through spatial_focus when the kernel returns a next_fq", async () => {
    const { result, unmount } = renderHook(() => useSpatialFocusActions(), {
      wrapper,
    });
    await flushListenSetup();

    const layerFq: FullyQualifiedMoniker = asFq("/window");
    const restored: FullyQualifiedMoniker = asFq("/window/restored");
    const popped: FullyQualifiedMoniker = asFq("/window/dialog");

    // Register a layer registry for the parent layer so the round-trip
    // builds a populated snapshot for the restored FQ.
    const restoredNode = document.createElement("div");
    restoredNode.getBoundingClientRect = () =>
      ({ x: 0, y: 0, width: 10, height: 10, top: 0, right: 10, bottom: 10, left: 0, toJSON: () => "" }) as DOMRect;
    const registry = new LayerScopeRegistry(layerFq);
    registry.add(restored, {
      ref: { current: restoredNode },
      parentZone: null,
      navOverride: {},
      segment: asSegment("restored"),
    });
    const dispose = result.current.registerLayerRegistry(layerFq, registry);

    // The kernel returns the layer's `last_focused`; mock that.
    mockInvoke.mockImplementation(async (...args: unknown[]) => {
      const cmd = args[0] as string;
      if (cmd === "spatial_pop_layer") return restored;
      return undefined;
    });

    await act(async () => {
      await result.current.popLayer(popped);
    });

    expect(mockInvoke).toHaveBeenCalledWith("spatial_pop_layer", { fq: popped });
    expect(mockInvoke).toHaveBeenCalledWith(
      "spatial_focus",
      expect.objectContaining({
        fq: restored,
        snapshot: expect.objectContaining({
          layer_fq: layerFq,
          scopes: expect.arrayContaining([
            expect.objectContaining({ fq: restored }),
          ]),
        }),
      }),
    );

    // Pin call order: spatial_pop_layer must precede spatial_focus.
    const popIdx = mockInvoke.mock.calls.findIndex(
      ([cmd]) => cmd === "spatial_pop_layer",
    );
    const focusIdx = mockInvoke.mock.calls.findIndex(
      ([cmd]) => cmd === "spatial_focus",
    );
    expect(popIdx).toBeGreaterThanOrEqual(0);
    expect(focusIdx).toBeGreaterThanOrEqual(0);
    expect(
      mockInvoke.mock.invocationCallOrder[popIdx],
    ).toBeLessThan(mockInvoke.mock.invocationCallOrder[focusIdx]);

    dispose();
    unmount();
  });

  it("popLayer does not round-trip to spatial_focus when the kernel returns null", async () => {
    const { result, unmount } = renderHook(() => useSpatialFocusActions(), {
      wrapper,
    });
    await flushListenSetup();

    const popped: FullyQualifiedMoniker = asFq("/window/dialog");

    mockInvoke.mockImplementation(async (...args: unknown[]) => {
      const cmd = args[0] as string;
      if (cmd === "spatial_pop_layer") return null;
      return undefined;
    });

    await act(async () => {
      await result.current.popLayer(popped);
    });

    expect(mockInvoke).toHaveBeenCalledWith("spatial_pop_layer", { fq: popped });
    const focusCalls = mockInvoke.mock.calls.filter(
      ([cmd]) => cmd === "spatial_focus",
    );
    expect(focusCalls).toHaveLength(0);

    unmount();
  });
});

describe("useFocusClaim listener identity", () => {
  it("reads the latest listener through the ref without re-registering", async () => {
    const key: FullyQualifiedMoniker = asFq("k");
    let calls: Array<[number, boolean]> = [];

    function Probe({ id }: { id: number }) {
      // Each render pass passes a fresh listener; the hook must call
      // through to the latest one without paying for re-registration.
      useFocusClaim(key, (focused) => {
        calls.push([id, focused]);
      });
      // Touch state to force renders
      useEffect(() => {}, []);
      return null;
    }

    const { rerender, unmount } = render(
      <SpatialFocusProvider>
        <Probe id={1} />
      </SpatialFocusProvider>,
    );
    await flushListenSetup();

    rerender(
      <SpatialFocusProvider>
        <Probe id={2} />
      </SpatialFocusProvider>,
    );
    rerender(
      <SpatialFocusProvider>
        <Probe id={3} />
      </SpatialFocusProvider>,
    );

    act(() => {
      listenHandlers["focus-changed"]?.({
        payload: makePayload({ next_fq: key }),
      });
    });

    // Only the latest listener (id=3) should fire.
    expect(calls).toEqual([[3, true]]);

    unmount();
  });
});

/* ---- drillIn / drillOut / focusedKey ---- */

describe("drillIn", () => {
  it("invokes spatial_drill_in with the focused (key, moniker) pair and returns the moniker", async () => {
    const targetMoniker: FullyQualifiedMoniker = asFq("ui:target");
    mockInvoke.mockImplementationOnce(() => Promise.resolve(targetMoniker));

    const { result, unmount } = renderHook(() => useSpatialFocusActions(), {
      wrapper,
    });
    await flushListenSetup();

    const key: FullyQualifiedMoniker = asFq("zone-key");
    const focusedFq: FullyQualifiedMoniker = asFq("ui:zone");
    let returned: FullyQualifiedMoniker | undefined;
    await act(async () => {
      returned = await result.current.drillIn(key, focusedFq);
    });

    expect(mockInvoke).toHaveBeenCalledWith("spatial_drill_in", {
      fq: key,
      focusedFq,
    });
    expect(returned).toBe(targetMoniker);

    unmount();
  });

  it("echoes the focused moniker when the registry has nothing to descend into", async () => {
    // Under the no-silent-dropout contract the kernel echoes the
    // focused moniker (rather than returning null) when there's
    // nothing to descend into. The React layer just passes that
    // through verbatim.
    const focusedFq: FullyQualifiedMoniker = asFq("ui:leaf");
    mockInvoke.mockImplementationOnce(() => Promise.resolve(focusedFq));

    const { result, unmount } = renderHook(() => useSpatialFocusActions(), {
      wrapper,
    });
    await flushListenSetup();

    let returned: FullyQualifiedMoniker | undefined;
    await act(async () => {
      returned = await result.current.drillIn(asFq("leaf"), focusedFq);
    });

    expect(returned).toBe(focusedFq);

    unmount();
  });
});

describe("drillOut", () => {
  it("invokes spatial_drill_out with the focused (key, moniker) pair and returns the parent moniker", async () => {
    const parentMoniker: FullyQualifiedMoniker = asFq("ui:parent-zone");
    mockInvoke.mockImplementationOnce(() => Promise.resolve(parentMoniker));

    const { result, unmount } = renderHook(() => useSpatialFocusActions(), {
      wrapper,
    });
    await flushListenSetup();

    const key: FullyQualifiedMoniker = asFq("leaf-key");
    const focusedFq: FullyQualifiedMoniker = asFq("ui:leaf");
    let returned: FullyQualifiedMoniker | undefined;
    await act(async () => {
      returned = await result.current.drillOut(key, focusedFq);
    });

    expect(mockInvoke).toHaveBeenCalledWith("spatial_drill_out", {
      fq: key,
      focusedFq,
    });
    expect(returned).toBe(parentMoniker);

    unmount();
  });

  it("echoes the focused moniker when the focused scope is at the layer root", async () => {
    // Under the no-silent-dropout contract the kernel echoes the
    // focused moniker (rather than returning null) at the layer root.
    // The React caller compares the result against the focused moniker
    // and dispatches `app.dismiss` on equality.
    const focusedFq: FullyQualifiedMoniker = asFq("ui:root-leaf");
    mockInvoke.mockImplementationOnce(() => Promise.resolve(focusedFq));

    const { result, unmount } = renderHook(() => useSpatialFocusActions(), {
      wrapper,
    });
    await flushListenSetup();

    let returned: FullyQualifiedMoniker | undefined;
    await act(async () => {
      returned = await result.current.drillOut(asFq("root-leaf"), focusedFq);
    });

    expect(returned).toBe(focusedFq);

    unmount();
  });
});

describe("focusedKey", () => {
  it("returns null before any focus-changed event arrives", async () => {
    const { result, unmount } = renderHook(() => useSpatialFocusActions(), {
      wrapper,
    });
    await flushListenSetup();

    expect(result.current.focusedFq()).toBeNull();

    unmount();
  });

  it("tracks the latest next_fq from focus-changed events", async () => {
    const { result, unmount } = renderHook(() => useSpatialFocusActions(), {
      wrapper,
    });
    await flushListenSetup();

    const aKey: FullyQualifiedMoniker = asFq("a");
    const bKey: FullyQualifiedMoniker = asFq("b");

    act(() => {
      listenHandlers["focus-changed"]?.({
        payload: makePayload({ next_fq: aKey }),
      });
    });
    expect(result.current.focusedFq()).toBe(aKey);

    act(() => {
      listenHandlers["focus-changed"]?.({
        payload: makePayload({ prev_fq: aKey, next_fq: bKey }),
      });
    });
    expect(result.current.focusedFq()).toBe(bKey);

    unmount();
  });

  it("clears to null when focus-changed reports next_fq as null", async () => {
    const { result, unmount } = renderHook(() => useSpatialFocusActions(), {
      wrapper,
    });
    await flushListenSetup();

    const key: FullyQualifiedMoniker = asFq("k");
    act(() => {
      listenHandlers["focus-changed"]?.({
        payload: makePayload({ next_fq: key }),
      });
    });
    expect(result.current.focusedFq()).toBe(key);

    act(() => {
      listenHandlers["focus-changed"]?.({
        payload: makePayload({ prev_fq: key, next_fq: null }),
      });
    });
    expect(result.current.focusedFq()).toBeNull();

    unmount();
  });
});

/* ---- subscribeFocusChanged ---- */

describe("subscribeFocusChanged", () => {
  it("delivers each focus-changed payload to every registered subscriber", async () => {
    const { result, unmount } = renderHook(() => useSpatialFocusActions(), {
      wrapper,
    });
    await flushListenSetup();

    const subscriberA = vi.fn();
    const subscriberB = vi.fn();
    const unsubA = result.current.subscribeFocusChanged(subscriberA);
    const unsubB = result.current.subscribeFocusChanged(subscriberB);

    const payload = makePayload({
      prev_fq: asFq("a"),
      next_fq: asFq("b"),
      next_segment: asSegment("task:b"),
    });
    act(() => {
      listenHandlers["focus-changed"]?.({ payload });
    });

    expect(subscriberA).toHaveBeenCalledWith(payload);
    expect(subscriberB).toHaveBeenCalledWith(payload);

    unsubA();
    unsubB();
    unmount();
  });

  it("stops calling a subscriber after its unsubscribe runs", async () => {
    const { result, unmount } = renderHook(() => useSpatialFocusActions(), {
      wrapper,
    });
    await flushListenSetup();

    const subscriber = vi.fn();
    const unsub = result.current.subscribeFocusChanged(subscriber);

    act(() => {
      listenHandlers["focus-changed"]?.({
        payload: makePayload({ next_fq: asFq("a") }),
      });
    });
    expect(subscriber).toHaveBeenCalledTimes(1);

    unsub();

    act(() => {
      listenHandlers["focus-changed"]?.({
        payload: makePayload({ next_fq: asFq("b") }),
      });
    });
    expect(subscriber).toHaveBeenCalledTimes(1);

    unmount();
  });

  it("delivers payloads with next_segment so consumers can bridge to entity-focus", async () => {
    const { result, unmount } = renderHook(() => useSpatialFocusActions(), {
      wrapper,
    });
    await flushListenSetup();

    const seen: Array<{
      key: FullyQualifiedMoniker | null;
      moniker: SegmentMoniker | null;
    }> = [];
    result.current.subscribeFocusChanged((payload) => {
      seen.push({ key: payload.next_fq, moniker: payload.next_segment });
    });

    act(() => {
      listenHandlers["focus-changed"]?.({
        payload: makePayload({
          next_fq: asFq("k1"),
          next_segment: asSegment("task:01ABC"),
        }),
      });
    });
    act(() => {
      listenHandlers["focus-changed"]?.({
        payload: makePayload({
          prev_fq: asFq("k1"),
          next_fq: null,
          next_segment: null,
        }),
      });
    });

    expect(seen).toEqual([
      { key: asFq("k1"), moniker: asSegment("task:01ABC") },
      { key: null, moniker: null },
    ]);

    unmount();
  });
});
