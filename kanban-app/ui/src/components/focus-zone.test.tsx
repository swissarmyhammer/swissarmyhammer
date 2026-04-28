/**
 * Tests for the `<FocusZone>` primitive.
 *
 * The zone is responsible for:
 *
 *  - Minting a fresh `SpatialKey` per instance (stable across re-renders).
 *  - Calling `spatial_register_zone` on mount with the kernel-types record.
 *  - Calling `spatial_unregister_scope` on unmount.
 *  - Publishing its key via `FocusZoneContext` so descendants pick it up
 *    as their `parent_zone`.
 *  - Falling back to a plain `<div>` (no spatial registration) when
 *    mounted outside a `<FocusLayer>` — same contract `<FocusScope>` has.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { fireEvent, render, act, waitFor } from "@testing-library/react";
import { createRef } from "react";

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

import { FocusZone, FocusZoneContext, useParentZoneKey } from "./focus-zone";
import { FocusLayer } from "./focus-layer";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import {
  asLayerName,
  asMoniker,
  type FocusChangedPayload,
  type SpatialKey,
} from "@/types/spatial";

async function flushSetup() {
  await act(async () => {
    await Promise.resolve();
  });
}

/**
 * Build a `focus-changed` payload with sensible defaults so tests can
 * spell out only the keys that matter for the assertion.
 */
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
  for (const k of Object.keys(listenHandlers)) delete listenHandlers[k];
});

function lastRegisterZoneArgs() {
  const calls = mockInvoke.mock.calls.filter(
    (c) => c[0] === "spatial_register_zone",
  );
  if (calls.length === 0) {
    throw new Error("expected spatial_register_zone call");
  }
  return calls[calls.length - 1][1] as Record<string, unknown>;
}

describe("<FocusZone>", () => {
  it("registers via spatial_register_zone with branded args on mount", async () => {
    const { unmount } = render(
      <SpatialFocusProvider>
        <FocusLayer name={asLayerName("window")}>
          <FocusZone moniker={asMoniker("ui:toolbar.actions")}>
            <span>zone</span>
          </FocusZone>
        </FocusLayer>
      </SpatialFocusProvider>,
    );
    await flushSetup();

    const args = lastRegisterZoneArgs();
    expect(args).toMatchObject({
      moniker: "ui:toolbar.actions",
      parentZone: null,
      overrides: {},
    });
    expect(typeof args.key).toBe("string");
    expect((args.key as string).length).toBeGreaterThan(0);
    expect(args.layerKey).toBeTruthy();
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
          <FocusZone moniker={asMoniker("ui:zone")}>{null}</FocusZone>
        </FocusLayer>
      </SpatialFocusProvider>,
    );
    await flushSetup();

    const registered = lastRegisterZoneArgs();
    mockInvoke.mockClear();
    unmount();

    const unregisters = mockInvoke.mock.calls.filter(
      (c) => c[0] === "spatial_unregister_scope",
    );
    expect(unregisters).toHaveLength(1);
    expect(unregisters[0][1]).toEqual({ key: registered.key });
  });

  it("publishes its key via FocusZoneContext", async () => {
    let observed: SpatialKey | null = null;
    function Capture() {
      observed = useParentZoneKey();
      return null;
    }

    const { unmount } = render(
      <SpatialFocusProvider>
        <FocusLayer name={asLayerName("window")}>
          <FocusZone moniker={asMoniker("ui:zone")}>
            <Capture />
          </FocusZone>
        </FocusLayer>
      </SpatialFocusProvider>,
    );
    await flushSetup();

    const args = lastRegisterZoneArgs();
    expect(observed).toBe(args.key);

    unmount();
  });

  it("forwards parentZone from an outer FocusZone to a child register call", async () => {
    let outerKey: SpatialKey | null = null;
    function CaptureOuter() {
      outerKey = useParentZoneKey();
      return null;
    }

    const { unmount } = render(
      <SpatialFocusProvider>
        <FocusLayer name={asLayerName("window")}>
          <FocusZone moniker={asMoniker("ui:outer")}>
            <CaptureOuter />
            <FocusZone moniker={asMoniker("ui:inner")}>{null}</FocusZone>
          </FocusZone>
        </FocusLayer>
      </SpatialFocusProvider>,
    );
    await flushSetup();

    // React commits child effects before parent effects, so we identify
    // each register call by moniker rather than relying on call order.
    const registers = mockInvoke.mock.calls
      .filter((c) => c[0] === "spatial_register_zone")
      .map((c) => c[1] as Record<string, unknown>);
    expect(registers).toHaveLength(2);
    const outerArgs = registers.find((a) => a.moniker === "ui:outer")!;
    const innerArgs = registers.find((a) => a.moniker === "ui:inner")!;

    expect(outerArgs.parentZone).toBeNull();
    expect(innerArgs.parentZone).toBe(outerKey);

    unmount();
  });

  it("clicks invoke spatial_focus on the zone's key", async () => {
    const { getByText, unmount } = render(
      <SpatialFocusProvider>
        <FocusLayer name={asLayerName("window")}>
          <FocusZone moniker={asMoniker("ui:zone")}>
            <span>zone-content</span>
          </FocusZone>
        </FocusLayer>
      </SpatialFocusProvider>,
    );
    await flushSetup();

    const registered = lastRegisterZoneArgs();
    mockInvoke.mockClear();

    fireEvent.click(getByText("zone-content"));

    const focusCalls = mockInvoke.mock.calls.filter(
      (c) => c[0] === "spatial_focus",
    );
    expect(focusCalls).toHaveLength(1);
    expect(focusCalls[0][1]).toEqual({ key: registered.key });

    unmount();
  });

  it("clicking an inner zone fires spatial_focus exactly once (with the inner key)", async () => {
    // Regression: a click on an inner `<FocusZone>` must not bubble to an
    // enclosing zone and re-fire `spatial_focus` with the outer key.
    // Mirrors the long-standing `<FocusScope>` convention.
    const { getByText, unmount } = render(
      <SpatialFocusProvider>
        <FocusLayer name={asLayerName("window")}>
          <FocusZone moniker={asMoniker("ui:outer")}>
            <FocusZone moniker={asMoniker("ui:inner")}>
              <span>inner-content</span>
            </FocusZone>
          </FocusZone>
        </FocusLayer>
      </SpatialFocusProvider>,
    );
    await flushSetup();

    // Pick the inner zone's register call by moniker (React commits child
    // effects before parent effects, so we don't rely on call order).
    const registers = mockInvoke.mock.calls
      .filter((c) => c[0] === "spatial_register_zone")
      .map((c) => c[1] as Record<string, unknown>);
    const innerArgs = registers.find((a) => a.moniker === "ui:inner")!;

    mockInvoke.mockClear();
    fireEvent.click(getByText("inner-content"));

    const focusCalls = mockInvoke.mock.calls.filter(
      (c) => c[0] === "spatial_focus",
    );
    expect(focusCalls).toHaveLength(1);
    expect(focusCalls[0][1]).toEqual({ key: innerArgs.key });

    unmount();
  });

  it("renders a fallback div when mounted outside any FocusLayer (no spatial registration)", () => {
    // Three-peer architecture: `<FocusZone>` is an entity-aware composite
    // that needs to keep working in unit tests that omit the spatial
    // provider stack — same fallback contract `<FocusScope>` exposes.
    // The plain `<div>` carries `data-moniker` for selector-based test
    // assertions, but no `spatial_register_zone` call fires and there is
    // no `<FocusIndicator>` because there is no Rust-side focus state to
    // follow.
    const { container, unmount } = render(
      <FocusZone moniker={asMoniker("ui:orphan")}>{null}</FocusZone>,
    );
    const node = container.querySelector("[data-moniker='ui:orphan']");
    expect(node).not.toBeNull();
    const registers = mockInvoke.mock.calls.filter(
      (c) => c[0] === "spatial_register_zone",
    );
    expect(registers).toHaveLength(0);
    unmount();
  });

  it("FocusZoneContext default is null when no zone wraps the consumer", () => {
    let observed: SpatialKey | null | undefined;
    function Capture() {
      observed = useParentZoneKey();
      return null;
    }
    render(
      <FocusZoneContext.Provider value={null}>
        <Capture />
      </FocusZoneContext.Provider>,
    );
    expect(observed).toBeNull();
  });

  it("renders data-moniker on the wrapper for CSS targeting / debugging", async () => {
    const { container, unmount } = render(
      <SpatialFocusProvider>
        <FocusLayer name={asLayerName("window")}>
          <FocusZone moniker={asMoniker("ui:zone-attr-test")}>
            <span>content</span>
          </FocusZone>
        </FocusLayer>
      </SpatialFocusProvider>,
    );
    await flushSetup();

    const node = container.querySelector("[data-moniker='ui:zone-attr-test']");
    expect(node).not.toBeNull();

    unmount();
  });

  it("rejects an onClick passthrough at the type level", () => {
    // The primitive owns the click handler; passing onClick must be a
    // compile-time error so a consumer cannot silently disable focus-on-click.
    // This is a type-only assertion — the cast keeps the test running while
    // ts-expect-error checks the type rejection.
    const _check = (
      <FocusZone
        moniker={asMoniker("ui:zone")}
        // @ts-expect-error onClick is omitted from FocusZone's passthrough type.
        onClick={() => {}}
      >
        {null}
      </FocusZone>
    );
    expect(_check).toBeTruthy();
  });

  it("forwards an external RefObject to the primitive's root div", async () => {
    // The optional `ref` prop must point at the same `<div>` that carries
    // `data-moniker` and the click/ResizeObserver wiring — the primitive's
    // internal ref. Pins the merged callback-ref contract so callers can
    // call e.g. `scrollIntoView` on the zone container.
    const externalRef = createRef<HTMLDivElement>();
    const { container, unmount } = render(
      <SpatialFocusProvider>
        <FocusLayer name={asLayerName("window")}>
          <FocusZone moniker={asMoniker("ui:zone")} ref={externalRef}>
            <span>zone</span>
          </FocusZone>
        </FocusLayer>
      </SpatialFocusProvider>,
    );
    await flushSetup();

    const node = container.querySelector("[data-moniker='ui:zone']");
    expect(externalRef.current).toBe(node);
    expect(externalRef.current?.getAttribute("data-moniker")).toBe("ui:zone");

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
    const { container, unmount } = render(
      <SpatialFocusProvider>
        <FocusLayer name={asLayerName("window")}>
          <FocusZone moniker={asMoniker("ui:zone")} ref={externalRef}>
            <span>zone</span>
          </FocusZone>
        </FocusLayer>
      </SpatialFocusProvider>,
    );
    await flushSetup();

    const node = container.querySelector("[data-moniker='ui:zone']");
    expect(calls[0]).toBe(node);
    // Internal ref is still wired: spatial_register_zone ran on mount.
    const registered = lastRegisterZoneArgs();
    expect(registered.moniker).toBe("ui:zone");

    unmount();
    expect(calls[calls.length - 1]).toBeNull();
  });

  it("toggles data-focused via the focus claim registry", async () => {
    const { container, unmount } = render(
      <SpatialFocusProvider>
        <FocusLayer name={asLayerName("window")}>
          <FocusZone moniker={asMoniker("ui:zone")}>
            <span>body</span>
          </FocusZone>
        </FocusLayer>
      </SpatialFocusProvider>,
    );
    await flushSetup();

    const args = lastRegisterZoneArgs();
    const node = container.querySelector(
      "[data-moniker='ui:zone']",
    ) as HTMLElement | null;
    expect(node).not.toBeNull();
    expect(node!.getAttribute("data-focused")).toBeNull();

    act(() => {
      listenHandlers["focus-changed"]?.({
        payload: makePayload({ next_key: args.key as SpatialKey }),
      });
    });
    await waitFor(() =>
      expect(node!.getAttribute("data-focused")).not.toBeNull(),
    );

    act(() => {
      listenHandlers["focus-changed"]?.({
        payload: makePayload({ prev_key: args.key as SpatialKey }),
      });
    });
    await waitFor(() => expect(node!.getAttribute("data-focused")).toBeNull());

    unmount();
  });

  it("renders <FocusIndicator> from React state when the zone is focused", async () => {
    // FocusZone owns its own focus claim, so it shows the same visible
    // decoration as `<FocusScope>` when the Rust kernel marks it as the
    // focused key. State path: Rust event → useFocusClaim → React state →
    // <FocusIndicator>. CSS plays no role — there is no [data-focused]
    // selector to draw the bar.
    const { container, queryByTestId, unmount } = render(
      <SpatialFocusProvider>
        <FocusLayer name={asLayerName("window")}>
          <FocusZone moniker={asMoniker("ui:zone")}>
            <span>body</span>
          </FocusZone>
        </FocusLayer>
      </SpatialFocusProvider>,
    );
    await flushSetup();

    expect(queryByTestId("focus-indicator")).toBeNull();

    const args = lastRegisterZoneArgs();
    act(() => {
      listenHandlers["focus-changed"]?.({
        payload: makePayload({ next_key: args.key as SpatialKey }),
      });
    });

    await waitFor(() =>
      expect(queryByTestId("focus-indicator")).not.toBeNull(),
    );
    const bar = queryByTestId("focus-indicator")!;
    const zone = container.querySelector(
      "[data-moniker='ui:zone']",
    ) as HTMLElement | null;
    expect(zone).not.toBeNull();
    expect(bar.parentElement).toBe(zone);

    act(() => {
      listenHandlers["focus-changed"]?.({
        payload: makePayload({ prev_key: args.key as SpatialKey }),
      });
    });
    await waitFor(() => expect(queryByTestId("focus-indicator")).toBeNull());

    unmount();
  });

  it("showFocusBar={false} suppresses the visible indicator while keeping data-focused", async () => {
    // Container zones (board, grid, perspective, view) use this to claim
    // their slot in the spatial graph without painting a focus bar around
    // their entire body. data-focused still emits — only the visible
    // decoration is suppressed.
    const { container, queryByTestId, unmount } = render(
      <SpatialFocusProvider>
        <FocusLayer name={asLayerName("window")}>
          <FocusZone moniker={asMoniker("ui:board")} showFocusBar={false}>
            <span>body</span>
          </FocusZone>
        </FocusLayer>
      </SpatialFocusProvider>,
    );
    await flushSetup();

    const args = lastRegisterZoneArgs();
    act(() => {
      listenHandlers["focus-changed"]?.({
        payload: makePayload({ next_key: args.key as SpatialKey }),
      });
    });

    const node = container.querySelector(
      "[data-moniker='ui:board']",
    ) as HTMLElement | null;
    expect(node).not.toBeNull();
    await waitFor(() =>
      expect(node!.getAttribute("data-focused")).not.toBeNull(),
    );
    expect(queryByTestId("focus-indicator")).toBeNull();

    unmount();
  });

});
