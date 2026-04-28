/**
 * Tests for `SpatialFocusProvider` — the per-window claim registry that
 * mirrors the Rust `SpatialState` over the `focus-changed` Tauri event.
 *
 * Coverage targets the four React-side test cases listed on the spatial-
 * nav focus-claim card:
 *
 * - Claim registry ignores events for unknown keys.
 * - Scope click invokes `spatial_focus` with its branded `SpatialKey`.
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
import type {
  FocusChangedPayload,
  LayerKey,
  Moniker,
  Rect,
  SpatialKey,
} from "@/types/spatial";
import { asLayerKey, asMoniker, asPixels, asSpatialKey } from "@/types/spatial";

/* ---- Helpers ---- */

function wrapper({ children }: { children: ReactNode }) {
  return <SpatialFocusProvider>{children}</SpatialFocusProvider>;
}

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
    const knownKey: SpatialKey = asSpatialKey("known");

    const { unmount } = renderHook(() => useFocusClaim(knownKey, claimSpy), {
      wrapper,
    });
    await flushListenSetup();

    // Dispatch an event whose `next_key` is NOT registered. The unknown
    // lookup must be a silent no-op — no claim fires anywhere.
    act(() => {
      listenHandlers["focus-changed"]?.({
        payload: makePayload({ next_key: asSpatialKey("ghost") }),
      });
    });

    expect(claimSpy).not.toHaveBeenCalled();

    // The known scope can still receive its own events — sanity-check
    // that the registry isn't broken by the unknown lookup.
    act(() => {
      listenHandlers["focus-changed"]?.({
        payload: makePayload({ next_key: knownKey }),
      });
    });
    expect(claimSpy).toHaveBeenCalledWith(true);

    unmount();
  });

  it("invokes spatial_focus with the branded SpatialKey on focus()", async () => {
    const { result, unmount } = renderHook(() => useSpatialFocusActions(), {
      wrapper,
    });
    await flushListenSetup();

    const key: SpatialKey = asSpatialKey("01ABC");
    await act(async () => {
      await result.current.focus(key);
    });

    expect(mockInvoke).toHaveBeenCalledWith("spatial_focus", { key });

    unmount();
  });

  it("invokes spatial_register_scope with the full kernel-types record", async () => {
    const { result, unmount } = renderHook(() => useSpatialFocusActions(), {
      wrapper,
    });
    await flushListenSetup();

    const key: SpatialKey = asSpatialKey("k1");
    const moniker: Moniker = asMoniker("task:01ABC");
    const rect: Rect = {
      x: asPixels(0),
      y: asPixels(0),
      width: asPixels(100),
      height: asPixels(50),
    };
    const layerKey: LayerKey = asLayerKey("L1");
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
      key,
      moniker,
      rect,
      layerKey,
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
    const key: SpatialKey = asSpatialKey("scope-1");

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
        payload: makePayload({ next_key: key }),
      });
    });
    expect(claimSpy).not.toHaveBeenCalled();
  });

  it("dispatches false to prev_key and true to next_key on focus transfer", async () => {
    const aKey: SpatialKey = asSpatialKey("a");
    const bKey: SpatialKey = asSpatialKey("b");
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
        payload: makePayload({ prev_key: aKey, next_key: bKey }),
      });
    });

    expect(aSpy).toHaveBeenCalledWith(false);
    expect(bSpy).toHaveBeenCalledWith(true);

    unmount();
  });

  it("does not break when a registered scope re-registers under the same key", async () => {
    const key: SpatialKey = asSpatialKey("reused");
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
        payload: makePayload({ next_key: key }),
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

    const key: SpatialKey = asSpatialKey("nav-from");
    await act(async () => {
      await result.current.navigate(key, "right");
    });

    expect(mockInvoke).toHaveBeenCalledWith("spatial_navigate", {
      key,
      direction: "right",
    });

    unmount();
  });
});

describe("useFocusClaim listener identity", () => {
  it("reads the latest listener through the ref without re-registering", async () => {
    const key: SpatialKey = asSpatialKey("k");
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
        payload: makePayload({ next_key: key }),
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
    const targetMoniker: Moniker = asMoniker("ui:target");
    mockInvoke.mockImplementationOnce(() => Promise.resolve(targetMoniker));

    const { result, unmount } = renderHook(() => useSpatialFocusActions(), {
      wrapper,
    });
    await flushListenSetup();

    const key: SpatialKey = asSpatialKey("zone-key");
    const focusedMoniker: Moniker = asMoniker("ui:zone");
    let returned: Moniker | undefined;
    await act(async () => {
      returned = await result.current.drillIn(key, focusedMoniker);
    });

    expect(mockInvoke).toHaveBeenCalledWith("spatial_drill_in", {
      key,
      focusedMoniker,
    });
    expect(returned).toBe(targetMoniker);

    unmount();
  });

  it("echoes the focused moniker when the registry has nothing to descend into", async () => {
    // Under the no-silent-dropout contract the kernel echoes the
    // focused moniker (rather than returning null) when there's
    // nothing to descend into. The React layer just passes that
    // through verbatim.
    const focusedMoniker: Moniker = asMoniker("ui:leaf");
    mockInvoke.mockImplementationOnce(() => Promise.resolve(focusedMoniker));

    const { result, unmount } = renderHook(() => useSpatialFocusActions(), {
      wrapper,
    });
    await flushListenSetup();

    let returned: Moniker | undefined;
    await act(async () => {
      returned = await result.current.drillIn(
        asSpatialKey("leaf"),
        focusedMoniker,
      );
    });

    expect(returned).toBe(focusedMoniker);

    unmount();
  });
});

describe("drillOut", () => {
  it("invokes spatial_drill_out with the focused (key, moniker) pair and returns the parent moniker", async () => {
    const parentMoniker: Moniker = asMoniker("ui:parent-zone");
    mockInvoke.mockImplementationOnce(() => Promise.resolve(parentMoniker));

    const { result, unmount } = renderHook(() => useSpatialFocusActions(), {
      wrapper,
    });
    await flushListenSetup();

    const key: SpatialKey = asSpatialKey("leaf-key");
    const focusedMoniker: Moniker = asMoniker("ui:leaf");
    let returned: Moniker | undefined;
    await act(async () => {
      returned = await result.current.drillOut(key, focusedMoniker);
    });

    expect(mockInvoke).toHaveBeenCalledWith("spatial_drill_out", {
      key,
      focusedMoniker,
    });
    expect(returned).toBe(parentMoniker);

    unmount();
  });

  it("echoes the focused moniker when the focused scope is at the layer root", async () => {
    // Under the no-silent-dropout contract the kernel echoes the
    // focused moniker (rather than returning null) at the layer root.
    // The React caller compares the result against the focused moniker
    // and dispatches `app.dismiss` on equality.
    const focusedMoniker: Moniker = asMoniker("ui:root-leaf");
    mockInvoke.mockImplementationOnce(() => Promise.resolve(focusedMoniker));

    const { result, unmount } = renderHook(() => useSpatialFocusActions(), {
      wrapper,
    });
    await flushListenSetup();

    let returned: Moniker | undefined;
    await act(async () => {
      returned = await result.current.drillOut(
        asSpatialKey("root-leaf"),
        focusedMoniker,
      );
    });

    expect(returned).toBe(focusedMoniker);

    unmount();
  });
});

describe("focusedKey", () => {
  it("returns null before any focus-changed event arrives", async () => {
    const { result, unmount } = renderHook(() => useSpatialFocusActions(), {
      wrapper,
    });
    await flushListenSetup();

    expect(result.current.focusedKey()).toBeNull();

    unmount();
  });

  it("tracks the latest next_key from focus-changed events", async () => {
    const { result, unmount } = renderHook(() => useSpatialFocusActions(), {
      wrapper,
    });
    await flushListenSetup();

    const aKey: SpatialKey = asSpatialKey("a");
    const bKey: SpatialKey = asSpatialKey("b");

    act(() => {
      listenHandlers["focus-changed"]?.({
        payload: makePayload({ next_key: aKey }),
      });
    });
    expect(result.current.focusedKey()).toBe(aKey);

    act(() => {
      listenHandlers["focus-changed"]?.({
        payload: makePayload({ prev_key: aKey, next_key: bKey }),
      });
    });
    expect(result.current.focusedKey()).toBe(bKey);

    unmount();
  });

  it("clears to null when focus-changed reports next_key as null", async () => {
    const { result, unmount } = renderHook(() => useSpatialFocusActions(), {
      wrapper,
    });
    await flushListenSetup();

    const key: SpatialKey = asSpatialKey("k");
    act(() => {
      listenHandlers["focus-changed"]?.({
        payload: makePayload({ next_key: key }),
      });
    });
    expect(result.current.focusedKey()).toBe(key);

    act(() => {
      listenHandlers["focus-changed"]?.({
        payload: makePayload({ prev_key: key, next_key: null }),
      });
    });
    expect(result.current.focusedKey()).toBeNull();

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
      prev_key: asSpatialKey("a"),
      next_key: asSpatialKey("b"),
      next_moniker: asMoniker("task:b"),
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
        payload: makePayload({ next_key: asSpatialKey("a") }),
      });
    });
    expect(subscriber).toHaveBeenCalledTimes(1);

    unsub();

    act(() => {
      listenHandlers["focus-changed"]?.({
        payload: makePayload({ next_key: asSpatialKey("b") }),
      });
    });
    expect(subscriber).toHaveBeenCalledTimes(1);

    unmount();
  });

  it("delivers payloads with next_moniker so consumers can bridge to entity-focus", async () => {
    const { result, unmount } = renderHook(() => useSpatialFocusActions(), {
      wrapper,
    });
    await flushListenSetup();

    const seen: Array<{
      key: SpatialKey | null;
      moniker: Moniker | null;
    }> = [];
    result.current.subscribeFocusChanged((payload) => {
      seen.push({ key: payload.next_key, moniker: payload.next_moniker });
    });

    act(() => {
      listenHandlers["focus-changed"]?.({
        payload: makePayload({
          next_key: asSpatialKey("k1"),
          next_moniker: asMoniker("task:01ABC"),
        }),
      });
    });
    act(() => {
      listenHandlers["focus-changed"]?.({
        payload: makePayload({
          prev_key: asSpatialKey("k1"),
          next_key: null,
          next_moniker: null,
        }),
      });
    });

    expect(seen).toEqual([
      { key: asSpatialKey("k1"), moniker: asMoniker("task:01ABC") },
      { key: null, moniker: null },
    ]);

    unmount();
  });
});
