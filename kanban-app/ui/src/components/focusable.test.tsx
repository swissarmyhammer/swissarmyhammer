/**
 * Tests for the `<Focusable>` primitive.
 *
 * The leaf focusable is responsible for:
 *
 *  - Minting a fresh `SpatialKey` per instance (stable across re-renders).
 *  - Calling `spatial_register_focusable` on mount with the kernel-types
 *    record (key, moniker, rect, layerKey, parentZone, overrides).
 *  - Calling `spatial_unregister_scope` on unmount.
 *  - Calling `spatial_focus` on click.
 *  - Subscribing to per-key focus claims so its `data-focused` attribute
 *    flips when the registry dispatches.
 *  - Throwing when used outside any `<FocusLayer>`.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { fireEvent, render, act, waitFor } from "@testing-library/react";
import { createRef } from "react";

const mockInvoke = vi.fn((..._args: unknown[]) => Promise.resolve());
let listenHandlers: Record<string, (event: { payload: unknown }) => void> = {};

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

import { Focusable } from "./focusable";
import { FocusZone } from "./focus-zone";
import { FocusLayer } from "./focus-layer";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import {
  asLayerName,
  asMoniker,
  type FocusChangedPayload,
  type LayerKey,
  type SpatialKey,
} from "@/types/spatial";

async function flushSetup() {
  await act(async () => {
    await Promise.resolve();
  });
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

beforeEach(() => {
  mockInvoke.mockClear();
  listenHandlers = {};
});

function pushedLayerArgs() {
  const calls = mockInvoke.mock.calls.filter(
    (c) => c[0] === "spatial_push_layer",
  );
  if (calls.length === 0) {
    throw new Error("expected spatial_push_layer call");
  }
  return calls[calls.length - 1][1] as { key: LayerKey };
}

function lastRegisterFocusableArgs() {
  const calls = mockInvoke.mock.calls.filter(
    (c) => c[0] === "spatial_register_focusable",
  );
  if (calls.length === 0) {
    throw new Error("expected spatial_register_focusable call");
  }
  return calls[calls.length - 1][1] as Record<string, unknown>;
}

describe("<Focusable>", () => {
  it("registers via spatial_register_focusable with branded args on mount", async () => {
    const { unmount } = render(
      <SpatialFocusProvider>
        <FocusLayer name={asLayerName("window")}>
          <Focusable moniker={asMoniker("task:01ABC")}>
            <span>card</span>
          </Focusable>
        </FocusLayer>
      </SpatialFocusProvider>,
    );
    await flushSetup();

    const args = lastRegisterFocusableArgs();
    expect(args).toMatchObject({
      moniker: "task:01ABC",
      parentZone: null,
      overrides: {},
    });
    expect(typeof args.key).toBe("string");
    expect((args.key as string).length).toBeGreaterThan(0);
    expect(args.layerKey).toBe(pushedLayerArgs().key);
    expect(args.rect).toMatchObject({
      x: expect.any(Number),
      y: expect.any(Number),
      width: expect.any(Number),
      height: expect.any(Number),
    });

    unmount();
  });

  it("unregisters via spatial_unregister_scope on unmount", async () => {
    const { unmount } = render(
      <SpatialFocusProvider>
        <FocusLayer name={asLayerName("window")}>
          <Focusable moniker={asMoniker("task:01ABC")}>{null}</Focusable>
        </FocusLayer>
      </SpatialFocusProvider>,
    );
    await flushSetup();

    const registered = lastRegisterFocusableArgs();
    mockInvoke.mockClear();
    unmount();

    const unregisters = mockInvoke.mock.calls.filter(
      (c) => c[0] === "spatial_unregister_scope",
    );
    expect(unregisters).toHaveLength(1);
    expect(unregisters[0][1]).toEqual({ key: registered.key });
  });

  it("clicks invoke spatial_focus on the focusable's key", async () => {
    const { getByText, unmount } = render(
      <SpatialFocusProvider>
        <FocusLayer name={asLayerName("window")}>
          <Focusable moniker={asMoniker("task:01ABC")}>
            <span>card</span>
          </Focusable>
        </FocusLayer>
      </SpatialFocusProvider>,
    );
    await flushSetup();

    const registered = lastRegisterFocusableArgs();
    mockInvoke.mockClear();

    fireEvent.click(getByText("card"));

    const focusCalls = mockInvoke.mock.calls.filter(
      (c) => c[0] === "spatial_focus",
    );
    expect(focusCalls).toHaveLength(1);
    expect(focusCalls[0][1]).toEqual({ key: registered.key });

    unmount();
  });

  it("toggles data-focused via the focus claim registry", async () => {
    const { getByTestId, unmount } = render(
      <SpatialFocusProvider>
        <FocusLayer name={asLayerName("window")}>
          <Focusable moniker={asMoniker("task:01ABC")} data-testid="leaf">
            <span>card</span>
          </Focusable>
        </FocusLayer>
      </SpatialFocusProvider>,
    );
    await flushSetup();

    const args = lastRegisterFocusableArgs();
    const node = getByTestId("leaf");
    expect(node.getAttribute("data-focused")).toBeNull();

    act(() => {
      listenHandlers["focus-changed"]?.({
        payload: makePayload({ next_key: args.key as SpatialKey }),
      });
    });
    await waitFor(() =>
      expect(node.getAttribute("data-focused")).not.toBeNull(),
    );

    act(() => {
      listenHandlers["focus-changed"]?.({
        payload: makePayload({ prev_key: args.key as SpatialKey }),
      });
    });
    await waitFor(() => expect(node.getAttribute("data-focused")).toBeNull());

    unmount();
  });

  it("renders <FocusIndicator> from React state, not from a DOM-attribute read", async () => {
    // Architectural contract: focus state flows Rust → useFocusClaim →
    // React state → <FocusIndicator>. The indicator must render whenever
    // the primitive's `focused` state is true and disappear when it
    // flips back, with no CSS rule reading `[data-focused]` to draw the
    // bar. Driving the test from the Rust-side `focus-changed` event
    // (rather than a click) closes the loop end-to-end: spatial nav
    // arrow-keys are dispatched by Rust, surface as the same event, and
    // must produce the same visible decoration.
    const { container, queryByTestId, unmount } = render(
      <SpatialFocusProvider>
        <FocusLayer name={asLayerName("window")}>
          <Focusable moniker={asMoniker("task:01ABC")} data-testid="leaf">
            <span>card</span>
          </Focusable>
        </FocusLayer>
      </SpatialFocusProvider>,
    );
    await flushSetup();

    // Before any focus claim the indicator must not be present.
    expect(queryByTestId("focus-indicator")).toBeNull();

    const args = lastRegisterFocusableArgs();
    act(() => {
      listenHandlers["focus-changed"]?.({
        payload: makePayload({ next_key: args.key as SpatialKey }),
      });
    });

    // After the claim flips, the indicator renders as a child of the
    // primitive's div — same element that carries `data-focused`. The
    // indicator's parent is the focusable; nothing outside it draws a
    // focus bar.
    await waitFor(() =>
      expect(queryByTestId("focus-indicator")).not.toBeNull(),
    );
    const bar = queryByTestId("focus-indicator")!;
    const leaf = container.querySelector(
      "[data-moniker='task:01ABC']",
    ) as HTMLElement | null;
    expect(leaf).not.toBeNull();
    expect(bar.parentElement).toBe(leaf);

    // Drop focus and the indicator unmounts.
    act(() => {
      listenHandlers["focus-changed"]?.({
        payload: makePayload({ prev_key: args.key as SpatialKey }),
      });
    });
    await waitFor(() => expect(queryByTestId("focus-indicator")).toBeNull());

    unmount();
  });

  it("showFocusBar={false} suppresses the visible indicator while keeping data-focused", async () => {
    // Some entity scopes (e.g. the inspector entity wrapper) want to
    // claim focus for the dispatcher / scope chain without painting a
    // visible bar around their entire body. The contract: data-focused
    // still emits (so tests and e2e selectors keep working), but the
    // <FocusIndicator> child does not render. State is in React, not
    // the DOM — flipping `showFocusBar` is a render-time decision.
    const { getByTestId, queryByTestId, unmount } = render(
      <SpatialFocusProvider>
        <FocusLayer name={asLayerName("window")}>
          <Focusable
            moniker={asMoniker("task:01ABC")}
            showFocusBar={false}
            data-testid="leaf"
          >
            <span>card</span>
          </Focusable>
        </FocusLayer>
      </SpatialFocusProvider>,
    );
    await flushSetup();

    const args = lastRegisterFocusableArgs();
    act(() => {
      listenHandlers["focus-changed"]?.({
        payload: makePayload({ next_key: args.key as SpatialKey }),
      });
    });

    await waitFor(() =>
      expect(getByTestId("leaf").getAttribute("data-focused")).not.toBeNull(),
    );
    // No bar — even though the primitive is focused.
    expect(queryByTestId("focus-indicator")).toBeNull();

    unmount();
  });

  it("throws when mounted outside any FocusLayer", () => {
    const spy = vi.spyOn(console, "error").mockImplementation(() => {});
    expect(() =>
      render(
        <SpatialFocusProvider>
          <Focusable moniker={asMoniker("task:01ABC")}>{null}</Focusable>
        </SpatialFocusProvider>,
      ),
    ).toThrow(/<FocusLayer>/);
    spy.mockRestore();
  });

  it("rejects an onClick passthrough at the type level", () => {
    // The primitive owns the click handler; passing onClick must be a
    // compile-time error so a consumer cannot silently disable focus-on-click.
    // This is a type-only assertion — the cast keeps the test running while
    // ts-expect-error checks the type rejection.
    const _check = (
      <Focusable
        moniker={asMoniker("task:01ABC")}
        // @ts-expect-error onClick is omitted from Focusable's passthrough type.
        onClick={() => {}}
      >
        {null}
      </Focusable>
    );
    expect(_check).toBeTruthy();
  });

  it("clicking a leaf inside a zone fires spatial_focus exactly once (with the leaf's key)", async () => {
    // Regression: the leaf click must not bubble to an enclosing
    // `<FocusZone>` and re-fire `spatial_focus` with the zone's key —
    // that would clobber the user's intent and race focus state.
    // See `<FocusScope>` for the long-standing convention this mirrors.
    const { getByText, unmount } = render(
      <SpatialFocusProvider>
        <FocusLayer name={asLayerName("window")}>
          <FocusZone moniker={asMoniker("ui:zone")}>
            <Focusable moniker={asMoniker("task:01ABC")}>
              <span>leaf</span>
            </Focusable>
          </FocusZone>
        </FocusLayer>
      </SpatialFocusProvider>,
    );
    await flushSetup();

    const leafArgs = lastRegisterFocusableArgs();
    mockInvoke.mockClear();

    fireEvent.click(getByText("leaf"));

    const focusCalls = mockInvoke.mock.calls.filter(
      (c) => c[0] === "spatial_focus",
    );
    expect(focusCalls).toHaveLength(1);
    expect(focusCalls[0][1]).toEqual({ key: leafArgs.key });

    unmount();
  });

  it("forwards an external RefObject to the primitive's root div", async () => {
    // The optional `ref` prop must point at the same `<div>` that carries
    // `data-moniker` and the click/ResizeObserver wiring — the primitive's
    // internal ref. Pins the merged callback-ref contract so callers can
    // call e.g. `scrollIntoView` on the focus target.
    const externalRef = createRef<HTMLDivElement>();
    const { getByTestId, unmount } = render(
      <SpatialFocusProvider>
        <FocusLayer name={asLayerName("window")}>
          <Focusable
            moniker={asMoniker("task:01ABC")}
            ref={externalRef}
            data-testid="leaf"
          >
            <span>card</span>
          </Focusable>
        </FocusLayer>
      </SpatialFocusProvider>,
    );
    await flushSetup();

    const node = getByTestId("leaf");
    expect(externalRef.current).toBe(node);
    expect(externalRef.current?.getAttribute("data-moniker")).toBe(
      "task:01ABC",
    );

    unmount();
    // Callback-ref cleanup nulls out the external ref on unmount.
    expect(externalRef.current).toBeNull();
  });

  it("forwards an external callback ref to the primitive's root div", async () => {
    // Same contract as the RefObject test, but for the function-ref form.
    // The primitive must call the external callback with the DOM node on
    // mount and with `null` on unmount — without breaking the internal
    // ref it relies on for ResizeObserver and click wiring.
    const calls: Array<HTMLDivElement | null> = [];
    const externalRef = (node: HTMLDivElement | null) => {
      calls.push(node);
    };
    const { getByTestId, unmount } = render(
      <SpatialFocusProvider>
        <FocusLayer name={asLayerName("window")}>
          <Focusable
            moniker={asMoniker("task:01ABC")}
            ref={externalRef}
            data-testid="leaf"
          >
            <span>card</span>
          </Focusable>
        </FocusLayer>
      </SpatialFocusProvider>,
    );
    await flushSetup();

    const node = getByTestId("leaf");
    expect(calls[0]).toBe(node);
    // Internal ref is still wired: spatial_register_focusable ran and a
    // click would still fire spatial_focus.
    const registered = lastRegisterFocusableArgs();
    expect(registered.moniker).toBe("task:01ABC");

    unmount();
    expect(calls[calls.length - 1]).toBeNull();
  });

  it("integration: nested layer/zone/focusable register a coherent hierarchy", async () => {
    const { unmount } = render(
      <SpatialFocusProvider>
        <FocusLayer name={asLayerName("window")}>
          <FocusZone moniker={asMoniker("ui:outer")}>
            <Focusable moniker={asMoniker("task:01ABC")}>
              <span>card</span>
            </Focusable>
          </FocusZone>
        </FocusLayer>
      </SpatialFocusProvider>,
    );
    await flushSetup();

    const layer = pushedLayerArgs();
    const zoneArgs = mockInvoke.mock.calls.find(
      (c) => c[0] === "spatial_register_zone",
    )?.[1] as Record<string, unknown>;
    const focusableArgs = lastRegisterFocusableArgs();

    expect(zoneArgs.layerKey).toBe(layer.key);
    expect(zoneArgs.parentZone).toBeNull();
    expect(focusableArgs.layerKey).toBe(layer.key);
    expect(focusableArgs.parentZone).toBe(zoneArgs.key);

    unmount();
  });
});
