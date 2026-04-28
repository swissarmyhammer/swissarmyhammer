import { describe, it, expect, vi } from "vitest";
import { render, renderHook, act } from "@testing-library/react";
import { type ReactNode } from "react";
import {
  EntityFocusProvider,
  FocusStore,
  useEntityFocus,
  useFocusActions,
  useFocusStore,
  useFocusedMoniker,
  useFocusedMonikerRef,
  useFocusedScope,
  useIsDirectFocus,
  useIsFocused,
} from "./entity-focus-context";
import { SpatialFocusProvider } from "./spatial-focus-context";
import { type CommandScope } from "./command-scope";
import {
  asMoniker,
  asSpatialKey,
  asWindowLabel,
  type FocusChangedPayload,
} from "@/types/spatial";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(() => Promise.resolve()),
}));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: vi.fn(() => ({ label: "main" })),
}));

/**
 * Captured `focus-changed` handler so bridge tests can fire synthetic
 * payloads through the `SpatialFocusProvider`'s single global listener.
 * The non-bridge tests rely on the noop unsubscribe behavior — they
 * neither register a handler nor inspect this slot.
 */
const listenCallbacks: Record<string, (event: { payload: unknown }) => void> =
  {};
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(
    (eventName: string, cb: (event: { payload: unknown }) => void) => {
      listenCallbacks[eventName] = cb;
      return Promise.resolve(() => {
        delete listenCallbacks[eventName];
      });
    },
  ),
}));
vi.mock("@tauri-apps/plugin-log", () => ({
  error: vi.fn(),
  warn: vi.fn(),
  info: vi.fn(),
  debug: vi.fn(),
  trace: vi.fn(),
  attachConsole: vi.fn(() => Promise.resolve()),
}));

const wrapper = ({ children }: { children: React.ReactNode }) => (
  <EntityFocusProvider>{children}</EntityFocusProvider>
);

describe("useEntityFocus", () => {
  it("returns null initially", () => {
    const { result } = renderHook(() => useEntityFocus(), { wrapper });
    expect(result.current.focusedMoniker).toBeNull();
  });

  it("setFocus updates focusedMoniker", () => {
    const { result } = renderHook(() => useEntityFocus(), { wrapper });
    act(() => {
      result.current.setFocus("task:abc");
    });
    expect(result.current.focusedMoniker).toBe("task:abc");
  });

  it("setFocus(null) clears focus", () => {
    const { result } = renderHook(() => useEntityFocus(), { wrapper });
    act(() => {
      result.current.setFocus("task:abc");
    });
    act(() => {
      result.current.setFocus(null);
    });
    expect(result.current.focusedMoniker).toBeNull();
  });

  it("throws outside provider", () => {
    expect(() => {
      renderHook(() => useEntityFocus());
    }).toThrow("useEntityFocus must be used within an EntityFocusProvider");
  });
});

describe("scope registry", () => {
  it("registerScope/unregisterScope lifecycle", () => {
    const { result } = renderHook(() => useEntityFocus(), { wrapper });
    const scope: CommandScope = {
      commands: new Map(),
      parent: null,
      moniker: "task:abc",
    };

    act(() => {
      result.current.registerScope("task:abc", scope);
    });
    expect(result.current.getScope("task:abc")).toBe(scope);

    act(() => {
      result.current.unregisterScope("task:abc");
    });
    expect(result.current.getScope("task:abc")).toBeNull();
  });

  it("getScope returns null for unknown moniker", () => {
    const { result } = renderHook(() => useEntityFocus(), { wrapper });
    expect(result.current.getScope("task:unknown")).toBeNull();
  });
});

describe("useFocusedScope", () => {
  it("returns null when nothing is focused", () => {
    const { result } = renderHook(() => useFocusedScope(), { wrapper });
    expect(result.current).toBeNull();
  });

  it("returns the scope when focused", () => {
    const scope: CommandScope = {
      commands: new Map(),
      parent: null,
      moniker: "task:abc",
    };

    const { result } = renderHook(
      () => {
        const focus = useEntityFocus();
        const focusedScope = useFocusedScope();
        return { focus, focusedScope };
      },
      { wrapper },
    );

    act(() => {
      result.current.focus.registerScope("task:abc", scope);
      result.current.focus.setFocus("task:abc");
    });

    expect(result.current.focusedScope).toBe(scope);
  });

  it("returns null when focused moniker has no registered scope", () => {
    const { result } = renderHook(
      () => {
        const focus = useEntityFocus();
        const focusedScope = useFocusedScope();
        return { focus, focusedScope };
      },
      { wrapper },
    );

    act(() => {
      result.current.focus.setFocus("task:missing");
    });
    expect(result.current.focusedScope).toBeNull();
  });
});

describe("useIsFocused", () => {
  it("returns false when nothing is focused", () => {
    const { result } = renderHook(() => useIsFocused("task:abc"), { wrapper });
    expect(result.current).toBe(false);
  });

  it("returns true for direct match", () => {
    const scope: CommandScope = {
      commands: new Map(),
      parent: null,
      moniker: "task:abc",
    };
    const { result } = renderHook(
      () => {
        const focus = useEntityFocus();
        const isFocused = useIsFocused("task:abc");
        return { focus, isFocused };
      },
      { wrapper },
    );

    act(() => {
      result.current.focus.registerScope("task:abc", scope);
      result.current.focus.setFocus("task:abc");
    });
    expect(result.current.isFocused).toBe(true);
  });

  it("returns true for ancestor match", () => {
    const parentScope: CommandScope = {
      commands: new Map(),
      parent: null,
      moniker: "column:col1",
    };
    const childScope: CommandScope = {
      commands: new Map(),
      parent: parentScope,
      moniker: "task:abc",
    };

    const { result } = renderHook(
      () => {
        const focus = useEntityFocus();
        const isFocused = useIsFocused("column:col1");
        return { focus, isFocused };
      },
      { wrapper },
    );

    act(() => {
      result.current.focus.registerScope("column:col1", parentScope);
      result.current.focus.registerScope("task:abc", childScope);
      result.current.focus.setFocus("task:abc");
    });
    // column:col1 is an ancestor of task:abc, so it should be focused
    expect(result.current.isFocused).toBe(true);
  });

  it("returns false for unrelated moniker", () => {
    const scope: CommandScope = {
      commands: new Map(),
      parent: null,
      moniker: "task:abc",
    };
    const { result } = renderHook(
      () => {
        const focus = useEntityFocus();
        const isFocused = useIsFocused("task:other");
        return { focus, isFocused };
      },
      { wrapper },
    );

    act(() => {
      result.current.focus.registerScope("task:abc", scope);
      result.current.focus.setFocus("task:abc");
    });
    expect(result.current.isFocused).toBe(false);
  });
});

/* ---------- window moniker in scope chain ---------- */

describe("window moniker in scope chain", () => {
  it("scope chain built from a focused entity includes window:main at the root", async () => {
    const { invoke } = await import("@tauri-apps/api/core");
    (invoke as ReturnType<typeof vi.fn>).mockClear();

    // Build a scope chain: task:abc → column:col1 → window:main
    const windowScope: CommandScope = {
      commands: new Map(),
      parent: null,
      moniker: "window:main",
    };
    const columnScope: CommandScope = {
      commands: new Map(),
      parent: windowScope,
      moniker: "column:col1",
    };
    const taskScope: CommandScope = {
      commands: new Map(),
      parent: columnScope,
      moniker: "task:abc",
    };

    const { result } = renderHook(() => useEntityFocus(), { wrapper });

    act(() => {
      result.current.registerScope("task:abc", taskScope);
      result.current.setFocus("task:abc");
    });

    // Wait for the async dispatch to flush
    await act(async () => {
      await new Promise((r) => setTimeout(r, 0));
    });

    // invokeFocusChange should have been called with a scope chain
    // that includes window:main as the last (root) element.
    // backendDispatch calls invoke("dispatch_command", ...) internally.
    expect(invoke).toHaveBeenCalledWith(
      "dispatch_command",
      expect.objectContaining({
        cmd: "ui.setFocus",
        args: { scope_chain: ["task:abc", "column:col1", "window:main"] },
      }),
    );
  });
});

// ---------------------------------------------------------------------------
// FocusStore — standalone (no provider needed)
// ---------------------------------------------------------------------------

describe("FocusStore", () => {
  it("subscribe notifies only the matching moniker on set", () => {
    const store = new FocusStore();
    const cbA = vi.fn();
    const cbB = vi.fn();
    const cbC = vi.fn();
    store.subscribe("A", cbA);
    store.subscribe("B", cbB);
    store.subscribe("C", cbC);

    store.set("A");
    expect(cbA).toHaveBeenCalledTimes(1);
    expect(cbB).not.toHaveBeenCalled();
    expect(cbC).not.toHaveBeenCalled();

    // Reset and move A -> B: both A and B slots should fire; C untouched.
    cbA.mockClear();
    cbB.mockClear();
    cbC.mockClear();
    store.set("B");
    expect(cbA).toHaveBeenCalledTimes(1);
    expect(cbB).toHaveBeenCalledTimes(1);
    expect(cbC).not.toHaveBeenCalled();
  });

  it("set is a no-op when the value does not change", () => {
    const store = new FocusStore();
    const cbA = vi.fn();
    store.subscribe("A", cbA);
    store.set("A");
    cbA.mockClear();

    store.set("A");
    expect(cbA).not.toHaveBeenCalled();
    expect(store.getSnapshot()).toBe("A");
  });

  it("subscribeAll fires on every change", () => {
    const store = new FocusStore();
    const broad = vi.fn();
    store.subscribeAll(broad);

    store.set("A");
    store.set("B");
    store.set(null);
    expect(broad).toHaveBeenCalledTimes(3);
  });

  it("unsubscribe stops further notifications and prunes empty slots", () => {
    const store = new FocusStore();
    const cb = vi.fn();
    const unsubscribe = store.subscribe("A", cb);
    store.set("A");
    expect(cb).toHaveBeenCalledTimes(1);

    unsubscribe();
    cb.mockClear();
    store.set(null);
    store.set("A");
    expect(cb).not.toHaveBeenCalled();
  });

  it("getSnapshot reflects the current value", () => {
    const store = new FocusStore();
    expect(store.getSnapshot()).toBeNull();
    store.set("A");
    expect(store.getSnapshot()).toBe("A");
    store.set(null);
    expect(store.getSnapshot()).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// useFocusActions / useFocusStore / useFocusedMoniker / useFocusedMonikerRef
// ---------------------------------------------------------------------------

describe("useFocusActions", () => {
  it("value identity is stable across focus moves", () => {
    const { result } = renderHook(
      () => {
        const actions = useFocusActions();
        return actions;
      },
      { wrapper },
    );
    const first = result.current;

    act(() => {
      first.setFocus("A");
    });
    act(() => {
      first.setFocus("B");
    });
    act(() => {
      first.setFocus(null);
    });

    // Must be the *same* object across every focus move.
    expect(result.current).toBe(first);
  });

  it("throws outside provider", () => {
    expect(() => {
      renderHook(() => useFocusActions());
    }).toThrow("useFocusActions must be used within an EntityFocusProvider");
  });
});

describe("useFocusStore", () => {
  it("returns the same store across renders", () => {
    const { result, rerender } = renderHook(() => useFocusStore(), {
      wrapper,
    });
    const first = result.current;
    rerender();
    expect(result.current).toBe(first);
    expect(first).toBeInstanceOf(FocusStore);
  });

  it("throws outside provider", () => {
    expect(() => {
      renderHook(() => useFocusStore());
    }).toThrow("useFocusStore must be used within an EntityFocusProvider");
  });
});

describe("useFocusedMoniker", () => {
  it("returns null initially and tracks changes", () => {
    const { result } = renderHook(
      () => {
        const actions = useFocusActions();
        const moniker = useFocusedMoniker();
        return { actions, moniker };
      },
      { wrapper },
    );
    expect(result.current.moniker).toBeNull();

    act(() => {
      result.current.actions.setFocus("task:abc");
    });
    expect(result.current.moniker).toBe("task:abc");

    act(() => {
      result.current.actions.setFocus(null);
    });
    expect(result.current.moniker).toBeNull();
  });
});

describe("useFocusedMonikerRef", () => {
  it("updates ref without re-rendering its caller", () => {
    let renderCount = 0;

    function Probe({
      onActions,
      onRef,
    }: {
      onActions: (a: ReturnType<typeof useFocusActions>) => void;
      onRef: (ref: React.MutableRefObject<string | null>) => void;
    }) {
      renderCount += 1;
      const actions = useFocusActions();
      const ref = useFocusedMonikerRef();
      onActions(actions);
      onRef(ref);
      return null;
    }

    let actions: ReturnType<typeof useFocusActions> | null = null;
    let capturedRef: React.MutableRefObject<string | null> | null = null;

    render(
      <EntityFocusProvider>
        <Probe
          onActions={(a) => {
            actions = a;
          }}
          onRef={(r) => {
            capturedRef = r;
          }}
        />
      </EntityFocusProvider>,
    );

    const beforeMoves = renderCount;
    const capturedActions = actions!;
    const ref = capturedRef!;

    act(() => {
      capturedActions.setFocus("A");
    });
    act(() => {
      capturedActions.setFocus("B");
    });
    act(() => {
      capturedActions.setFocus(null);
    });

    // Probe must NOT have re-rendered — focus-ref consumers are render-free.
    expect(renderCount).toBe(beforeMoves);
    // Ref tracks the latest value.
    expect(ref.current).toBeNull();

    act(() => {
      capturedActions.setFocus("final");
    });
    expect(renderCount).toBe(beforeMoves);
    expect(ref.current).toBe("final");
  });
});

// ---------------------------------------------------------------------------
// useIsDirectFocus — selective subscription
// ---------------------------------------------------------------------------

describe("useIsDirectFocus", () => {
  /**
   * Render N probes, each subscribed to its own moniker via useIsDirectFocus.
   * Record per-probe render counts so the test can assert how many probes
   * were woken by a given focus move.
   */
  function renderProbes(monikers: string[]) {
    const counts: Record<string, number> = Object.fromEntries(
      monikers.map((m) => [m, 0]),
    );

    function Probe({ moniker }: { moniker: string }) {
      counts[moniker] += 1;
      const focused = useIsDirectFocus(moniker);
      return (
        <span data-testid={`probe-${moniker}`}>{focused ? "yes" : "no"}</span>
      );
    }

    function Harness({ children }: { children: ReactNode }) {
      return <>{children}</>;
    }

    let actions: ReturnType<typeof useFocusActions> | null = null;
    function ActionsProbe() {
      actions = useFocusActions();
      return null;
    }

    const utils = render(
      <EntityFocusProvider>
        <Harness>
          <ActionsProbe />
          {monikers.map((m) => (
            <Probe key={m} moniker={m} />
          ))}
        </Harness>
      </EntityFocusProvider>,
    );

    return { utils, counts, getActions: () => actions! };
  }

  it("only notifies the two affected monikers on focus change", () => {
    const { counts, getActions, utils } = renderProbes(["A", "B", "C"]);
    const actions = getActions();

    // Record mount-time counts, then zero them so assertions measure moves only.
    const base = { ...counts };

    act(() => {
      actions.setFocus("A");
    });
    // A must have re-rendered; B and C must NOT have.
    expect(counts.A).toBe(base.A + 1);
    expect(counts.B).toBe(base.B);
    expect(counts.C).toBe(base.C);
    expect(utils.getByTestId("probe-A").textContent).toBe("yes");

    act(() => {
      actions.setFocus("B");
    });
    // A lost focus → re-rendered. B gained focus → re-rendered. C untouched.
    expect(counts.A).toBe(base.A + 2);
    expect(counts.B).toBe(base.B + 1);
    expect(counts.C).toBe(base.C);
    expect(utils.getByTestId("probe-A").textContent).toBe("no");
    expect(utils.getByTestId("probe-B").textContent).toBe("yes");
  });

  it("returns false for probe whose moniker is not focused", () => {
    const { getActions, utils } = renderProbes(["A", "B"]);

    act(() => {
      getActions().setFocus("A");
    });
    expect(utils.getByTestId("probe-A").textContent).toBe("yes");
    expect(utils.getByTestId("probe-B").textContent).toBe("no");
  });

  it("clearing focus notifies the previously focused slot only", () => {
    const { counts, getActions } = renderProbes(["A", "B", "C"]);
    const actions = getActions();
    act(() => {
      actions.setFocus("A");
    });
    const afterSet = { ...counts };

    act(() => {
      actions.setFocus(null);
    });
    // Only A's slot should fire when clearing focus.
    expect(counts.A).toBe(afterSet.A + 1);
    expect(counts.B).toBe(afterSet.B);
    expect(counts.C).toBe(afterSet.C);
  });
});

// ---------------------------------------------------------------------------
// useEntityFocus — compat shim still re-renders on focus
// ---------------------------------------------------------------------------

describe("useEntityFocus (compat shim)", () => {
  it("exposes the combined shape and still re-renders on focus", () => {
    let renderCount = 0;
    function Probe({
      onActions,
    }: {
      onActions: (a: ReturnType<typeof useFocusActions>) => void;
    }) {
      renderCount += 1;
      const focus = useEntityFocus();
      // Shape check — every action plus the moniker must be present.
      onActions(focus);
      return (
        <span data-testid="moniker">{focus.focusedMoniker ?? "null"}</span>
      );
    }

    let actions: ReturnType<typeof useFocusActions> | null = null;
    const utils = render(
      <EntityFocusProvider>
        <Probe
          onActions={(a) => {
            actions = a;
          }}
        />
      </EntityFocusProvider>,
    );
    const base = renderCount;

    act(() => {
      actions!.setFocus("A");
    });
    expect(utils.getByTestId("moniker").textContent).toBe("A");
    expect(renderCount).toBe(base + 1);

    act(() => {
      actions!.setFocus("B");
    });
    expect(utils.getByTestId("moniker").textContent).toBe("B");
    expect(renderCount).toBe(base + 2);

    // Shape: every action from FocusActions must be present on the shim.
    const shim = actions!;
    expect(typeof shim.setFocus).toBe("function");
    expect(typeof shim.registerScope).toBe("function");
    expect(typeof shim.unregisterScope).toBe("function");
    expect(typeof shim.getScope).toBe("function");
    expect(typeof shim.broadcastNavCommand).toBe("function");
  });
});

// ---------------------------------------------------------------------------
// broadcastNavCommand — predicate registry has been removed
// ---------------------------------------------------------------------------

describe("broadcastNavCommand", () => {
  it("is a no-op stub that always returns false", () => {
    // The pull-based predicate registry has been replaced by the Rust
    // spatial-nav kernel's `overrides` map. The callable remains in the
    // actions bag for source compatibility but has no behavior — it
    // never claims focus and always returns `false`.
    const { result } = renderHook(() => useFocusActions(), { wrapper });
    expect(result.current.broadcastNavCommand("nav.right")).toBe(false);
    expect(result.current.broadcastNavCommand("nav.up")).toBe(false);
    expect(result.current.broadcastNavCommand("any.command")).toBe(false);
  });

  it("does not mutate focus state", () => {
    // Even when there's a focused entity, broadcastNavCommand must not
    // touch the store — all real navigation lives in the Rust kernel.
    const { result } = renderHook(() => useEntityFocus(), { wrapper });
    act(() => {
      result.current.setFocus("task:abc");
    });
    expect(result.current.focusedMoniker).toBe("task:abc");

    act(() => {
      result.current.broadcastNavCommand("nav.right");
    });
    expect(result.current.focusedMoniker).toBe("task:abc");
  });
});

// ---------------------------------------------------------------------------
// Spatial → entity-focus bridge
//
// The spatial-nav kernel emits `focus-changed` events whenever a
// `<FocusScope>` click or arrow-key navigation moves focus, and
// `EntityFocusProvider` is responsible for mirroring `payload.next_moniker`
// into its own moniker-keyed `FocusStore` so downstream consumers — most
// importantly the `focusedMonikerRef` API and the `useFocusedScope`
// chain that drives `extractScopeBindings` — stay in sync without each
// click handler having to double-write entity-focus and spatial-focus.
//
// The bridge subscribes to `SpatialFocusActions.subscribeFocusChanged`
// from inside the entity-focus provider, so the integration only fires
// when both providers are mounted (production always mounts both; isolated
// unit-test harnesses that skip `<SpatialFocusProvider>` get the legacy
// behavior unchanged).
// ---------------------------------------------------------------------------

/** Wait one microtask for `SpatialFocusProvider`'s `listen()` setup. */
async function flushListenSetup() {
  await act(async () => {
    await Promise.resolve();
  });
}

/**
 * Push a synthetic `focus-changed` payload through the captured listener
 * so consumers of `subscribeFocusChanged` (notably the entity-focus
 * bridge) observe the wire-shape event Tauri would deliver from a
 * `spatial_focus` / `spatial_navigate` invocation.
 */
function emitFocusChanged(payload: FocusChangedPayload): void {
  const cb = listenCallbacks["focus-changed"];
  expect(cb).toBeTruthy();
  cb({ payload });
}

function bridgeWrapper({ children }: { children: ReactNode }) {
  return (
    <SpatialFocusProvider>
      <EntityFocusProvider>{children}</EntityFocusProvider>
    </SpatialFocusProvider>
  );
}

describe("EntityFocusProvider — spatial focus bridge", () => {
  it("mirrors focus-changed.next_moniker into the entity-focus store", async () => {
    const { result } = renderHook(() => useEntityFocus(), {
      wrapper: bridgeWrapper,
    });
    await flushListenSetup();

    expect(result.current.focusedMoniker).toBeNull();

    await act(async () => {
      emitFocusChanged({
        window_label: asWindowLabel("main"),
        prev_key: null,
        next_key: asSpatialKey("k1"),
        next_moniker: asMoniker("task:01ABC"),
      });
    });

    expect(result.current.focusedMoniker).toBe("task:01ABC");
  });

  it("clears focus when next_key is null", async () => {
    const { result } = renderHook(() => useEntityFocus(), {
      wrapper: bridgeWrapper,
    });
    await flushListenSetup();

    await act(async () => {
      emitFocusChanged({
        window_label: asWindowLabel("main"),
        prev_key: null,
        next_key: asSpatialKey("k1"),
        next_moniker: asMoniker("task:01ABC"),
      });
    });
    expect(result.current.focusedMoniker).toBe("task:01ABC");

    await act(async () => {
      emitFocusChanged({
        window_label: asWindowLabel("main"),
        prev_key: asSpatialKey("k1"),
        next_key: null,
        next_moniker: null,
      });
    });

    expect(result.current.focusedMoniker).toBeNull();
  });

  it("keeps focusedMonikerRef in sync with successive spatial moves", async () => {
    let capturedRef: React.MutableRefObject<string | null> | null = null;
    let capturedActions: ReturnType<typeof useEntityFocus> | null = null;

    function Probe() {
      capturedActions = useEntityFocus();
      capturedRef = useFocusedMonikerRef();
      return null;
    }

    render(
      <SpatialFocusProvider>
        <EntityFocusProvider>
          <Probe />
        </EntityFocusProvider>
      </SpatialFocusProvider>,
    );
    await flushListenSetup();
    // Touch the actions bag so eslint/TS see the variable as used and
    // future test refactors that read it find a populated handle.
    expect(capturedActions).not.toBeNull();
    expect(capturedRef!.current).toBeNull();

    await act(async () => {
      emitFocusChanged({
        window_label: asWindowLabel("main"),
        prev_key: null,
        next_key: asSpatialKey("ka"),
        next_moniker: asMoniker("column:todo"),
      });
    });
    expect(capturedRef!.current).toBe("column:todo");

    await act(async () => {
      emitFocusChanged({
        window_label: asWindowLabel("main"),
        prev_key: asSpatialKey("ka"),
        next_key: asSpatialKey("kb"),
        next_moniker: asMoniker("task:01"),
      });
    });
    expect(capturedRef!.current).toBe("task:01");
  });

  it("is a no-op when SpatialFocusProvider is absent", () => {
    // The legacy `<EntityFocusProvider>`-only contract still has to work for
    // unit-test harnesses that don't wrap in `<SpatialFocusProvider>`. The
    // bridge degrades silently — `setFocus` from React still drives the
    // store as before.
    const { result } = renderHook(() => useEntityFocus(), { wrapper });
    act(() => {
      result.current.setFocus("task:abc");
    });
    expect(result.current.focusedMoniker).toBe("task:abc");
  });
});
