/**
 * Tests for the React-driven `spatial_focus_lost` IPC.
 *
 * Step 8 of the spatial-nav redesign: when the focused scope unmounts,
 * `LayerScopeRegistry.delete(fq)` notifies the spatial focus provider,
 * which builds a snapshot whose `scopes` set excludes the lost FQM and
 * dispatches `spatial_focus_lost` to the kernel. These tests pin three
 * properties:
 *
 * - The IPC fires only when the deleted FQM is the currently focused FQM
 *   in the window.
 * - The snapshot built inside the deletion handler does NOT contain the
 *   lost FQM (the registry deletion runs first).
 * - Errors from the IPC are caught and logged — never propagated through
 *   the cleanup path.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, act } from "@testing-library/react";
import { type RefObject } from "react";

/* -------------------------------------------------------------------------- */
/* Tauri mocks                                                                */
/* -------------------------------------------------------------------------- */

const mockInvoke = vi.fn(
  (..._args: unknown[]): Promise<unknown> => Promise.resolve(),
);
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
  type FocusChangedPayload,
  type FullyQualifiedMoniker,
  type Rect,
} from "@/types/spatial";

/* -------------------------------------------------------------------------- */
/* Helpers                                                                    */
/* -------------------------------------------------------------------------- */

async function flushSetup() {
  await act(async () => {
    await Promise.resolve();
  });
}

const ZERO_RECT: Rect = {
  x: asPixels(0),
  y: asPixels(0),
  width: asPixels(0),
  height: asPixels(0),
};

/** Build a `ScopeEntry` whose `ref.current` is a real DOM node and whose
 *  `lastKnownRect` is seeded with `rect` (defaults to a zero rect — happy-
 *  dom returns zeros for `getBoundingClientRect()` without explicit
 *  layout, so this matches the rect that would actually be cached).
 */
function makeEntry(
  parentZone: FullyQualifiedMoniker | null = null,
  rect: Rect | null = ZERO_RECT,
): { entry: ScopeEntry; node: HTMLDivElement } {
  const node = document.createElement("div");
  const ref: RefObject<HTMLElement | null> = { current: node };
  return {
    entry: {
      ref,
      parentZone,
      segment: asSegment("scope"),
      lastKnownRect: rect,
    },
    node,
  };
}

/**
 * Capture the registry the layer publishes. Mounts inside the layer so
 * we observe the layer's own registry, not a parent's.
 */
function CaptureRegistry({
  out,
}: {
  out: { current: LayerScopeRegistry | null };
}) {
  out.current = useOptionalLayerScopeRegistry();
  return null;
}

/** Render a `<SpatialFocusProvider>` + `<FocusLayer>` and capture the
 *  layer's scope registry. Returns the JSX so callers can `render(...)`. */
function captureLayerRegistryFor(
  layerName: string,
  out: { current: LayerScopeRegistry | null },
) {
  return (
    <SpatialFocusProvider>
      <FocusLayer name={asSegment(layerName)}>
        <CaptureRegistry out={out} />
      </FocusLayer>
    </SpatialFocusProvider>
  );
}

beforeEach(() => {
  mockInvoke.mockClear();
  listenHandlers = {};
});

/* -------------------------------------------------------------------------- */
/* Tests                                                                      */
/* -------------------------------------------------------------------------- */

describe("spatial_focus_lost IPC", () => {
  it("fires when the deleted FQM is the currently focused FQM", async () => {
    const captured: { current: LayerScopeRegistry | null } = { current: null };
    render(captureLayerRegistryFor("window", captured));
    await flushSetup();

    const layerFq = fqRoot(asSegment("window"));
    const fq = composeFq(layerFq, asSegment("focused"));

    const registry = captured.current!;
    const { entry } = makeEntry(layerFq);
    registry.add(fq, entry);

    // Simulate the kernel announcing focus on `fq`. The provider's
    // `focus-changed` listener mirrors `next_fq` into its internal
    // `focusedFqRef`, which is what the deletion handler reads.
    const payload: FocusChangedPayload = {
      window_label: "main" as FocusChangedPayload["window_label"],
      prev_fq: null,
      next_fq: fq,
      next_segment: asSegment("focused"),
    };
    act(() => {
      listenHandlers["focus-changed"]?.({ payload });
    });

    mockInvoke.mockClear();
    registry.delete(fq);

    expect(mockInvoke).toHaveBeenCalledWith(
      "spatial_focus_lost",
      expect.objectContaining({
        focusedFq: fq,
        lostParentZone: layerFq,
        lostLayerFq: layerFq,
        lostRect: expect.objectContaining({
          x: asPixels(0),
          y: asPixels(0),
        }),
        snapshot: expect.objectContaining({
          layer_fq: layerFq,
        }),
      }),
    );
  });

  it("does NOT fire when an unfocused scope is unmounted", async () => {
    const captured: { current: LayerScopeRegistry | null } = { current: null };
    render(captureLayerRegistryFor("window", captured));
    await flushSetup();

    const layerFq = fqRoot(asSegment("window"));
    const focusedFq = composeFq(layerFq, asSegment("focused"));
    const otherFq = composeFq(layerFq, asSegment("other"));

    const registry = captured.current!;
    const { entry: focusedEntry } = makeEntry(layerFq);
    const { entry: otherEntry } = makeEntry(layerFq);
    registry.add(focusedFq, focusedEntry);
    registry.add(otherFq, otherEntry);

    // Set focus to focusedFq.
    act(() => {
      listenHandlers["focus-changed"]?.({
        payload: {
          window_label: "main" as FocusChangedPayload["window_label"],
          prev_fq: null,
          next_fq: focusedFq,
          next_segment: asSegment("focused"),
        },
      });
    });

    mockInvoke.mockClear();
    // Delete the unfocused scope.
    registry.delete(otherFq);

    // The IPC must NOT have been called for the unfocused scope.
    const focusLostCalls = mockInvoke.mock.calls.filter(
      (call) => call[0] === "spatial_focus_lost",
    );
    expect(focusLostCalls).toHaveLength(0);
  });

  it("snapshot built in the delete handler excludes the lost FQM", async () => {
    const captured: { current: LayerScopeRegistry | null } = { current: null };
    render(captureLayerRegistryFor("window", captured));
    await flushSetup();

    const layerFq = fqRoot(asSegment("window"));
    const focusedFq = composeFq(layerFq, asSegment("focused"));
    const sibFq = composeFq(layerFq, asSegment("sib"));

    const registry = captured.current!;
    const { entry: focusedEntry } = makeEntry(layerFq);
    const { entry: sibEntry } = makeEntry(layerFq);
    registry.add(focusedFq, focusedEntry);
    registry.add(sibFq, sibEntry);

    act(() => {
      listenHandlers["focus-changed"]?.({
        payload: {
          window_label: "main" as FocusChangedPayload["window_label"],
          prev_fq: null,
          next_fq: focusedFq,
          next_segment: asSegment("focused"),
        },
      });
    });

    mockInvoke.mockClear();
    registry.delete(focusedFq);

    const focusLostCall = mockInvoke.mock.calls.find(
      (call) => call[0] === "spatial_focus_lost",
    );
    expect(focusLostCall).toBeDefined();
    const args = focusLostCall![1] as {
      snapshot: { scopes: { fq: FullyQualifiedMoniker }[] };
    };
    const snapshotFqs = args.snapshot.scopes.map((s) => s.fq);
    expect(snapshotFqs).not.toContain(focusedFq);
    expect(snapshotFqs).toContain(sibFq);
  });

  it("IPC errors are caught and never propagate from the cleanup path", async () => {
    const captured: { current: LayerScopeRegistry | null } = { current: null };
    render(captureLayerRegistryFor("window", captured));
    await flushSetup();

    const layerFq = fqRoot(asSegment("window"));
    const fq = composeFq(layerFq, asSegment("focused"));

    const registry = captured.current!;
    const { entry } = makeEntry(layerFq);
    registry.add(fq, entry);
    act(() => {
      listenHandlers["focus-changed"]?.({
        payload: {
          window_label: "main" as FocusChangedPayload["window_label"],
          prev_fq: null,
          next_fq: fq,
          next_segment: asSegment("focused"),
        },
      });
    });

    // Make ONLY the spatial_focus_lost IPC reject so the catch handler
    // runs. Other commands (spatial_push_layer, etc.) must continue to
    // resolve so the layer mount machinery doesn't trip a different
    // error path.
    mockInvoke.mockImplementation((command: unknown) => {
      if (command === "spatial_focus_lost") {
        return Promise.reject(new Error("ipc boom"));
      }
      return Promise.resolve();
    });

    const consoleErrorSpy = vi
      .spyOn(console, "error")
      .mockImplementation(() => {});

    // The delete must not throw even though the IPC rejected — the
    // cleanup path can never propagate an error or React's unmount
    // would stall.
    expect(() => registry.delete(fq)).not.toThrow();

    // Drain the rejected promise so the error reaches console.error.
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(consoleErrorSpy).toHaveBeenCalledWith(
      "[spatial_focus_lost] failed",
      expect.any(Error),
    );
    consoleErrorSpy.mockRestore();
    mockInvoke.mockReset();
    mockInvoke.mockImplementation(
      (..._args: unknown[]): Promise<unknown> => Promise.resolve(),
    );
  });
});

/* -------------------------------------------------------------------------- */
/* Real React unmount lifecycle — regression test                             */
/* -------------------------------------------------------------------------- */

/**
 * Pin the production-unmount path that earlier-iteration code
 * silently skipped. Mounting a real `<FocusScope>` inside a
 * `<FocusLayer>` and toggling the scope away (parent layer stays
 * mounted — the production scenario) walks the same commit phase
 * that nullifies the bound `setRef(null)` callback BEFORE the
 * `useEffect` cleanup runs `LayerScopeRegistry.delete(fq)`. With the
 * cached-rect contract on `ScopeEntry.lastKnownRect`, the deletion
 * listener still has live geometry to dispatch with — the IPC fires
 * even though `entry.ref.current` is `null` at delete time.
 *
 * A whole-tree `unmount()` is NOT the right shape here: when the
 * `<FocusLayer>` unmounts in the same commit, its
 * `registerLayerRegistry` cleanup unsubscribes the deletion listener
 * before the child scope's cleanup runs `delete()`, so the listener
 * (correctly) does not fire. Production unmounts almost always keep
 * the enclosing layer alive — a card column shrinking, an inspector
 * row vanishing, etc. — and that is the case this regression test
 * exercises.
 */
describe("spatial_focus_lost real unmount lifecycle", () => {
  it("fires the IPC when a real <FocusScope> with focus is removed by a re-render (parent layer stays mounted)", async () => {
    // Stub getBoundingClientRect on every newly-created div so the
    // mount-time / ResizeObserver rect samples produce a non-zero
    // rect — the cached rect on `ScopeEntry.lastKnownRect` is what the
    // deletion listener reads, so this is the value the test will
    // assert against.
    const STUB_RECT = { x: 10, y: 20, width: 30, height: 40 };
    const originalGetRect = HTMLDivElement.prototype.getBoundingClientRect;
    HTMLDivElement.prototype.getBoundingClientRect = function () {
      return {
        x: STUB_RECT.x,
        y: STUB_RECT.y,
        width: STUB_RECT.width,
        height: STUB_RECT.height,
        top: STUB_RECT.y,
        left: STUB_RECT.x,
        right: STUB_RECT.x + STUB_RECT.width,
        bottom: STUB_RECT.y + STUB_RECT.height,
        toJSON: () => STUB_RECT,
      } as DOMRect;
    };

    try {
      const layerFq = fqRoot(asSegment("window"));
      const fq = composeFq(layerFq, asSegment("focused"));

      function Tree({ show }: { show: boolean }) {
        return (
          <SpatialFocusProvider>
            <FocusLayer name={asSegment("window")}>
              {show ? (
                <FocusScope moniker={asSegment("focused")} commands={[]}>
                  <span>focused</span>
                </FocusScope>
              ) : null}
            </FocusLayer>
          </SpatialFocusProvider>
        );
      }

      const { rerender, unmount } = render(<Tree show={true} />);
      // Two microtask flushes: the provider's `listen()` setup
      // resolves on the next tick, and the layer's effect that
      // subscribes the deletion listener also has to run.
      await flushSetup();
      await flushSetup();

      // The focus-changed handler must be wired up by now — guard the
      // test against a silent miss where the listener fires into the
      // void.
      expect(listenHandlers["focus-changed"]).toBeDefined();

      // Tell the provider the focused FQM so the deletion listener
      // recognises this scope as the focused one.
      act(() => {
        listenHandlers["focus-changed"]?.({
          payload: {
            window_label: "main" as FocusChangedPayload["window_label"],
            prev_fq: null,
            next_fq: fq,
            next_segment: asSegment("focused"),
          },
        });
      });

      mockInvoke.mockClear();

      // Re-render WITHOUT the FocusScope. The FocusLayer stays mounted,
      // its `registerLayerRegistry` effect's deletion listener stays
      // subscribed, and the FocusScope's commit-phase `setRef(null)`
      // runs BEFORE the layer-registry `useEffect` cleanup that calls
      // `delete()`. The cached `lastKnownRect` is the only live geometry
      // available — that is what the listener must dispatch.
      rerender(<Tree show={false} />);
      await act(async () => {
        await Promise.resolve();
      });

      const focusLostCall = mockInvoke.mock.calls.find(
        (call) => call[0] === "spatial_focus_lost",
      );
      expect(focusLostCall).toBeDefined();
      const args = focusLostCall![1] as {
        focusedFq: FullyQualifiedMoniker;
        lostLayerFq: FullyQualifiedMoniker;
        lostRect: Rect;
      };
      expect(args.focusedFq).toBe(fq);
      expect(args.lostLayerFq).toBe(layerFq);
      expect(args.lostRect).toEqual({
        x: asPixels(STUB_RECT.x),
        y: asPixels(STUB_RECT.y),
        width: asPixels(STUB_RECT.width),
        height: asPixels(STUB_RECT.height),
      });

      unmount();
    } finally {
      HTMLDivElement.prototype.getBoundingClientRect = originalGetRect;
    }
  });
});
