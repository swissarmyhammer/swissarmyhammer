/**
 * Architecture-invariant tests pinning that the React entity-focus store
 * is a **pure projection** of the Rust kernel's focus state.
 *
 * Source of truth for card `01KQD0WK54G0FRD7SZVZASA9ST`. The user
 * direction:
 * > "I expect the state for the focus to be in the Rust kernel and the
 * > UI to just render it. That was kinda the whole point to avoid two
 * > sets of state."
 *
 * After this card lands:
 *
 * - `setFocus(moniker)` becomes a write-through to the kernel: it
 *   dispatches `spatial_focus_by_moniker` (or equivalent), waits for
 *   the kernel's `focus-changed` event to flow back through the
 *   `subscribeFocusChanged` bridge, and only then updates the
 *   moniker-keyed store.
 * - The store NEVER mutates synchronously from `setFocus`. The only
 *   path that writes the store is the kernel `focus-changed`
 *   subscription.
 * - If the kernel rejects the moniker (unknown to the registry), the
 *   store stays at its previous value; the kernel logs a
 *   `tracing::error!` and the React side surfaces a `console.error`.
 *
 * These tests use the kernel simulator (`installKernelSimulator`) so the
 * IPC trace is fully observable and the timing contract is exact.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { type ReactNode } from "react";

// ---------------------------------------------------------------------------
// Hoisted Tauri-API spy triple — same shape as
// `inspector.kernel-focus-advance.browser.test.tsx`.
// ---------------------------------------------------------------------------

type ListenCallback = (event: { payload: unknown }) => void;

const { mockInvoke, mockListen, listeners } = vi.hoisted(() => {
  const listeners = new Map<string, ListenCallback[]>();
  const mockInvoke = vi.fn(
    async (_cmd: string, _args?: unknown): Promise<unknown> => undefined,
  );
  const mockListen = vi.fn(
    (eventName: string, cb: ListenCallback): Promise<() => void> => {
      const cbs = listeners.get(eventName) ?? [];
      cbs.push(cb);
      listeners.set(eventName, cbs);
      return Promise.resolve(() => {
        const arr = listeners.get(eventName);
        if (arr) {
          const idx = arr.indexOf(cb);
          if (idx >= 0) arr.splice(idx, 1);
        }
      });
    },
  );
  return { mockInvoke, mockListen, listeners };
});

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...a: unknown[]) => mockInvoke(...(a as [string, unknown?])),
}));

vi.mock("@tauri-apps/api/event", () => ({
  emit: vi.fn(() => Promise.resolve()),
  listen: (...a: Parameters<typeof mockListen>) => mockListen(...a),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ label: "main" }),
}));

vi.mock("@tauri-apps/plugin-log", () => ({
  error: vi.fn(),
  warn: vi.fn(),
  info: vi.fn(),
  debug: vi.fn(),
  trace: vi.fn(),
  attachConsole: vi.fn(() => Promise.resolve()),
}));

// ---------------------------------------------------------------------------
// Imports come after the mocks.
// ---------------------------------------------------------------------------

import {
  EntityFocusProvider,
  useEntityFocus,
  useFocusedMonikerRef,
  useFocusedScope,
  useFocusActions,
} from "./entity-focus-context";
import { SpatialFocusProvider } from "./spatial-focus-context";
import { type CommandScope } from "./command-scope";
import {
  asLayerKey,
  asMoniker,
  asSpatialKey,
  asWindowLabel,
  type Pixels,
  type Rect,
} from "@/types/spatial";
import { installKernelSimulator } from "@/test-helpers/kernel-simulator";

const ZERO_RECT: Rect = {
  x: 0 as Pixels,
  y: 0 as Pixels,
  width: 0 as Pixels,
  height: 0 as Pixels,
};

const wrapper = ({ children }: { children: ReactNode }) => (
  <SpatialFocusProvider>
    <EntityFocusProvider>{children}</EntityFocusProvider>
  </SpatialFocusProvider>
);

/** Wait one microtask for `SpatialFocusProvider`'s `listen()` setup. */
async function flushListenSetup() {
  await act(async () => {
    await Promise.resolve();
  });
}

/**
 * Pre-register a moniker → key mapping in the kernel simulator so
 * `spatial_focus_by_moniker` can resolve it. Registers a single
 * `<FocusScope>`-shaped entry with the given key/moniker; the layer/zone
 * tree details don't matter for these projection tests.
 */
async function seedRegistration(
  layerKey: string,
  scopeKey: string,
  moniker: string,
): Promise<void> {
  await mockInvoke("spatial_register_scope", {
    key: asSpatialKey(scopeKey),
    moniker: asMoniker(moniker),
    rect: ZERO_RECT,
    layerKey: asLayerKey(layerKey),
    parentZone: null,
    overrides: {},
  });
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("EntityFocusProvider — kernel-projection invariant", () => {
  beforeEach(() => {
    mockInvoke.mockClear();
    mockListen.mockClear();
    listeners.clear();
  });

  it("setFocus(moniker) dispatches a kernel command exactly once", async () => {
    installKernelSimulator(mockInvoke, listeners);
    await seedRegistration("L", "k1", "task:01ABC");
    mockInvoke.mockClear();

    const { result } = renderHook(() => useEntityFocus(), { wrapper });
    await flushListenSetup();

    await act(async () => {
      result.current.setFocus("task:01ABC");
      await new Promise((r) => setTimeout(r, 0));
    });

    // Count every IPC that targets the kernel's focus-by-moniker entry
    // point. The signature must accept a moniker; the simulator
    // resolves it to a SpatialKey internally. The exact command name
    // depends on the implementation choice (Step 3) — accept either
    // `spatial_focus_by_moniker` or a `spatial_focus(key)` call where
    // the key was looked up React-side via `findByMoniker`.
    const focusByMonikerCalls = mockInvoke.mock.calls.filter(
      (c) => c[0] === "spatial_focus_by_moniker",
    );
    const focusByKeyCalls = mockInvoke.mock.calls.filter(
      (c) => c[0] === "spatial_focus",
    );
    const totalFocusCalls = focusByMonikerCalls.length + focusByKeyCalls.length;
    expect(
      totalFocusCalls,
      "setFocus(moniker) must dispatch exactly one kernel focus command",
    ).toBe(1);
  });

  it("setFocus(moniker) does NOT update the entity-focus store synchronously", async () => {
    const sim = installKernelSimulator(
      mockInvoke,
      listeners,
      // Hold the focus-changed emit until we explicitly release it. The
      // simulator normally emits via `queueMicrotask`; we want a clean
      // "before the kernel responds" snapshot.
      async () => undefined,
    );
    await seedRegistration("L", "k1", "task:01ABC");

    const { result } = renderHook(() => useEntityFocus(), { wrapper });
    await flushListenSetup();

    expect(result.current.focusedMoniker).toBeNull();

    // Fire setFocus *without* awaiting microtasks. The kernel simulator
    // will queue its emit; before the queue drains, the React store
    // must still be null — proving setFocus does not write the store
    // directly.
    act(() => {
      result.current.setFocus("task:01ABC");
    });

    // Read synchronously, before any microtask flush.
    expect(
      result.current.focusedMoniker,
      "store must NOT update synchronously from setFocus — only the kernel event drives it",
    ).toBeNull();

    // Now drain the microtask queue and re-check: the focus-changed
    // event from the simulator should have flowed through the bridge.
    await act(async () => {
      await new Promise((r) => setTimeout(r, 0));
    });
    expect(result.current.focusedMoniker).toBe("task:01ABC");
    expect(sim.currentFocus.key).toBe("k1");
  });

  it("setFocus(moniker) for an unknown moniker leaves the store untouched and logs an error", async () => {
    installKernelSimulator(mockInvoke, listeners);

    // Seed an initial focus so we can prove the store DOESN'T regress.
    await seedRegistration("L", "k_known", "task:known");

    const { result } = renderHook(() => useEntityFocus(), { wrapper });
    await flushListenSetup();

    await act(async () => {
      result.current.setFocus("task:known");
      await new Promise((r) => setTimeout(r, 0));
    });
    expect(result.current.focusedMoniker).toBe("task:known");

    const errorSpy = vi.spyOn(console, "error").mockImplementation(() => {});
    try {
      // Ask the kernel to focus a moniker that has no registered scope.
      // The kernel emits `tracing::error!`; the simulator is
      // architected to mirror that as a no-op (no focus-changed
      // emission). The React-side dispatch awaits an `Err` from
      // `spatial_focus_by_moniker` and surfaces it as console.error.
      await act(async () => {
        result.current.setFocus("task:does-not-exist");
        await new Promise((r) => setTimeout(r, 0));
      });

      expect(
        result.current.focusedMoniker,
        "store must remain at its previous value when the kernel rejects the moniker",
      ).toBe("task:known");

      // We surface a console.error in the React adapter so dev-mode
      // sees the rejection. The kernel-side `tracing::error!` is
      // covered by Rust unit tests on the kernel side.
      expect(errorSpy).toHaveBeenCalled();
    } finally {
      errorSpy.mockRestore();
    }
  });

  it("useFocusedScope() reflects the kernel's reported moniker after focus-changed", async () => {
    installKernelSimulator(mockInvoke, listeners);
    await seedRegistration("L", "k1", "task:01ABC");

    const taskScope: CommandScope = {
      commands: new Map(),
      parent: null,
      moniker: "task:01ABC",
    };

    const { result } = renderHook(
      () => {
        const actions = useFocusActions();
        const focused = useFocusedScope();
        return { actions, focused };
      },
      { wrapper },
    );
    await flushListenSetup();

    act(() => {
      result.current.actions.registerScope("task:01ABC", taskScope);
    });
    expect(result.current.focused).toBeNull();

    // Drive the kernel; wait for the bridge.
    await act(async () => {
      result.current.actions.setFocus("task:01ABC");
      await new Promise((r) => setTimeout(r, 0));
    });

    expect(
      result.current.focused,
      "useFocusedScope() must reflect the kernel's reported moniker, not whatever React state was prior",
    ).toBe(taskScope);
  });

  it("setFocus(null) dispatches spatial_clear_focus and waits for the kernel emit", async () => {
    installKernelSimulator(mockInvoke, listeners);
    await seedRegistration("L", "k1", "task:01ABC");

    const { result } = renderHook(() => useEntityFocus(), { wrapper });
    await flushListenSetup();

    // Drive focus to a known moniker first so we can prove the store
    // does NOT regress synchronously on `setFocus(null)`.
    await act(async () => {
      result.current.setFocus("task:01ABC");
      await new Promise((r) => setTimeout(r, 0));
    });
    expect(result.current.focusedMoniker).toBe("task:01ABC");

    mockInvoke.mockClear();

    // Synchronous null write would be the bug we're guarding against.
    // Fire `setFocus(null)` and immediately read the store BEFORE
    // draining microtasks: it must still report the previous moniker
    // because the kernel has not yet emitted `focus-changed`.
    act(() => {
      result.current.setFocus(null);
    });

    expect(
      result.current.focusedMoniker,
      "setFocus(null) must NOT mutate the store synchronously — the store stays at its previous value until the kernel emits a clear event",
    ).toBe("task:01ABC");

    // Confirm the kernel command was dispatched. The exact command
    // name is `spatial_clear_focus`; we accept either that or a
    // moniker-shaped clear (`spatial_focus_by_moniker` with a null
    // moniker) for resilience to the implementation choice.
    const clearCalls = mockInvoke.mock.calls.filter(
      (c) =>
        c[0] === "spatial_clear_focus" ||
        (c[0] === "spatial_focus_by_moniker" &&
          (c[1] as { moniker?: unknown })?.moniker === null),
    );
    expect(
      clearCalls.length,
      "setFocus(null) must dispatch exactly one kernel clear-focus command",
    ).toBe(1);

    // Drain the microtask queue and verify the bridge eventually
    // wrote the store back to null in response to the kernel emit.
    await act(async () => {
      await new Promise((r) => setTimeout(r, 0));
    });
    expect(result.current.focusedMoniker).toBeNull();
  });

  it("focusedMonikerRef.current survives a single kernel cycle without blinking through null", async () => {
    installKernelSimulator(mockInvoke, listeners);
    await seedRegistration("L", "k1", "task:01ABC");

    let captured: React.MutableRefObject<string | null> | null = null;
    let capturedActions: ReturnType<typeof useFocusActions> | null = null;

    function Probe() {
      capturedActions = useFocusActions();
      captured = useFocusedMonikerRef();
      return null;
    }

    const { rerender } = renderHook(() => null, {
      wrapper: ({ children }: { children: ReactNode }) => (
        <SpatialFocusProvider>
          <EntityFocusProvider>
            <Probe />
            {children}
          </EntityFocusProvider>
        </SpatialFocusProvider>
      ),
    });
    await flushListenSetup();

    expect(captured!.current).toBeNull();

    await act(async () => {
      capturedActions!.setFocus("task:01ABC");
      await new Promise((r) => setTimeout(r, 0));
    });

    expect(captured!.current).toBe("task:01ABC");

    // Re-render — value must not blink through null between renders.
    rerender();
    expect(captured!.current).toBe("task:01ABC");
  });
});

/** Suppress an unused-import warning for the hoisted `asWindowLabel`. */
void asWindowLabel;
