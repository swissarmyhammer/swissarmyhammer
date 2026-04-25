import { describe, it, expect, vi } from "vitest";
import { render, renderHook, act } from "@testing-library/react";
import { useState, type ReactNode } from "react";
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
  useRestoreFocus,
  type ClaimPredicate,
} from "./entity-focus-context";
import { FocusScope } from "@/components/focus-scope";
import { type CommandScope } from "./command-scope";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(() => Promise.resolve()),
}));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: vi.fn(() => ({ label: "main" })),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
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

/** Flush microtasks and pending effects. */
async function flush() {
  await act(async () => {
    await new Promise((r) => setTimeout(r, 0));
  });
}

/** Reads focusedMoniker from context and renders it as text. */
function FocusMonitor() {
  const { focusedMoniker } = useEntityFocus();
  return <span data-testid="focus-monitor">{focusedMoniker ?? "null"}</span>;
}

// ---------------------------------------------------------------------------
// broadcastNavCommand tests
// ---------------------------------------------------------------------------

describe("broadcastNavCommand", () => {
  it("matching predicate claims focus", async () => {
    const claimPredicates: ClaimPredicate[] = [
      { command: "nav.right", when: (f) => f === "field:tags" },
    ];

    const { getByTestId } = render(
      <EntityFocusProvider>
        <FocusScope moniker="field:tags" commands={[]}>
          <span>tags</span>
        </FocusScope>
        <FocusScope
          moniker="panel:inspector"
          commands={[]}
          claimWhen={claimPredicates}
        >
          <span>inspector</span>
        </FocusScope>
        <FocusMonitor />
        <SetFocusButton moniker="field:tags" />
        <BroadcastButton commandId="nav.right" />
      </EntityFocusProvider>,
    );
    await flush();

    // Set focus to field:tags
    await act(async () => {
      getByTestId("set-focus").click();
      await new Promise((r) => setTimeout(r, 0));
    });
    expect(getByTestId("focus-monitor").textContent).toBe("field:tags");

    // Broadcast nav.right — should claim focus for panel:inspector
    await act(async () => {
      getByTestId("broadcast").click();
      await new Promise((r) => setTimeout(r, 0));
    });
    expect(getByTestId("focus-monitor").textContent).toBe("panel:inspector");
  });

  it("no matching predicate leaves focus unchanged", async () => {
    const claimPredicates: ClaimPredicate[] = [
      { command: "nav.right", when: (f) => f === "field:tags" },
    ];

    const { getByTestId } = render(
      <EntityFocusProvider>
        <FocusScope
          moniker="panel:inspector"
          commands={[]}
          claimWhen={claimPredicates}
        >
          <span>inspector</span>
        </FocusScope>
        <FocusMonitor />
        <SetFocusButton moniker="field:title" />
        <BroadcastButton commandId="nav.right" />
      </EntityFocusProvider>,
    );
    await flush();

    // Set focus to field:title (not field:tags, so predicate won't match)
    await act(async () => {
      getByTestId("set-focus").click();
      await new Promise((r) => setTimeout(r, 0));
    });
    expect(getByTestId("focus-monitor").textContent).toBe("field:title");

    // Broadcast nav.right — predicate checks for field:tags, won't match
    await act(async () => {
      getByTestId("broadcast").click();
      await new Promise((r) => setTimeout(r, 0));
    });
    expect(getByTestId("focus-monitor").textContent).toBe("field:title");
  });

  it("first match wins (short-circuit)", async () => {
    const secondPredWhen = vi.fn(() => true);
    const claimA: ClaimPredicate[] = [
      { command: "nav.right", when: () => true },
    ];
    const claimB: ClaimPredicate[] = [
      { command: "nav.right", when: secondPredWhen },
    ];

    const { getByTestId } = render(
      <EntityFocusProvider>
        <FocusScope moniker="panel:a" commands={[]} claimWhen={claimA}>
          <span>a</span>
        </FocusScope>
        <FocusScope moniker="panel:b" commands={[]} claimWhen={claimB}>
          <span>b</span>
        </FocusScope>
        <FocusMonitor />
        <BroadcastButton commandId="nav.right" />
      </EntityFocusProvider>,
    );
    await flush();

    await act(async () => {
      getByTestId("broadcast").click();
      await new Promise((r) => setTimeout(r, 0));
    });

    // First registered (panel:a) should win
    expect(getByTestId("focus-monitor").textContent).toBe("panel:a");
    // Second predicate should NOT have been called (short-circuit)
    expect(secondPredWhen).not.toHaveBeenCalled();
  });

  it("unmounted scope's predicate is not evaluated", async () => {
    const predWhen = vi.fn(() => true);
    const claimPredicates: ClaimPredicate[] = [
      { command: "nav.right", when: predWhen },
    ];

    function UnmountableScope({ show }: { show: boolean }) {
      return show ? (
        <FocusScope
          moniker="panel:temp"
          commands={[]}
          claimWhen={claimPredicates}
        >
          <span>temp</span>
        </FocusScope>
      ) : null;
    }

    function Harness() {
      const [show, setShow] = useState(true);
      return (
        <>
          <UnmountableScope show={show} />
          <FocusMonitor />
          <button data-testid="toggle" onClick={() => setShow(false)} />
          <BroadcastButton commandId="nav.right" />
        </>
      );
    }

    const { getByTestId } = render(
      <EntityFocusProvider>
        <Harness />
      </EntityFocusProvider>,
    );
    await flush();

    // Unmount the scope
    await act(async () => {
      getByTestId("toggle").click();
      await new Promise((r) => setTimeout(r, 0));
    });

    // Reset the spy to clear any calls from mount phase
    predWhen.mockClear();

    // Broadcast — predicate should NOT be evaluated since scope is unmounted
    await act(async () => {
      getByTestId("broadcast").click();
      await new Promise((r) => setTimeout(r, 0));
    });

    expect(predWhen).not.toHaveBeenCalled();
    expect(getByTestId("focus-monitor").textContent).toBe("null");
  });
});

/** Helper button to set focus imperatively. */
function SetFocusButton({ moniker }: { moniker: string }) {
  const { setFocus } = useEntityFocus();
  return <button data-testid="set-focus" onClick={() => setFocus(moniker)} />;
}

/** Helper button to call broadcastNavCommand. */
function BroadcastButton({ commandId }: { commandId: string }) {
  const { broadcastNavCommand } = useEntityFocus();
  return (
    <button
      data-testid="broadcast"
      onClick={() => broadcastNavCommand(commandId)}
    />
  );
}

describe("useRestoreFocus", () => {
  /**
   * Helper component that conditionally renders a child that calls useRestoreFocus.
   * The parent manages focus state and controls when the child mounts/unmounts.
   */
  function RestoreFocusHarness({
    initialMoniker,
    scope,
  }: {
    initialMoniker: string | null;
    scope?: CommandScope;
  }) {
    const [showChild, setShowChild] = useState(false);
    const focus = useEntityFocus();

    return (
      <>
        <button
          data-testid="setup"
          onClick={() => {
            if (initialMoniker && scope) {
              focus.registerScope(initialMoniker, scope);
            }
            if (initialMoniker) {
              focus.setFocus(initialMoniker);
            }
          }}
        />
        <button data-testid="show" onClick={() => setShowChild(true)} />
        <button data-testid="hide" onClick={() => setShowChild(false)} />
        <button
          data-testid="steal-focus"
          onClick={() => focus.setFocus("task:inspected")}
        />
        <button
          data-testid="unregister"
          onClick={() => {
            if (initialMoniker) focus.unregisterScope(initialMoniker);
          }}
        />
        <span data-testid="focused">{focus.focusedMoniker ?? "null"}</span>
        {showChild && <RestoreChild />}
      </>
    );
  }

  function RestoreChild() {
    useRestoreFocus();
    return <span data-testid="child">child</span>;
  }

  it("restores focus to saved moniker when scope still exists", () => {
    const scope: CommandScope = {
      commands: new Map(),
      parent: null,
      moniker: "task:abc",
    };
    const { getByTestId } = render(
      <EntityFocusProvider>
        <RestoreFocusHarness initialMoniker="task:abc" scope={scope} />
      </EntityFocusProvider>,
    );

    // 1. Set up focus
    act(() => {
      getByTestId("setup").click();
    });
    expect(getByTestId("focused").textContent).toBe("task:abc");

    // 2. Mount the restore hook — captures "task:abc"
    act(() => {
      getByTestId("show").click();
    });
    expect(getByTestId("child")).toBeTruthy();

    // 3. Inspector steals focus
    act(() => {
      getByTestId("steal-focus").click();
    });
    expect(getByTestId("focused").textContent).toBe("task:inspected");

    // 4. Unmount — should restore to "task:abc"
    act(() => {
      getByTestId("hide").click();
    });
    expect(getByTestId("focused").textContent).toBe("task:abc");
  });

  it("clears focus when saved moniker no longer exists in registry", () => {
    const scope: CommandScope = {
      commands: new Map(),
      parent: null,
      moniker: "task:abc",
    };
    const { getByTestId } = render(
      <EntityFocusProvider>
        <RestoreFocusHarness initialMoniker="task:abc" scope={scope} />
      </EntityFocusProvider>,
    );

    act(() => {
      getByTestId("setup").click();
    });
    act(() => {
      getByTestId("show").click();
    });
    act(() => {
      getByTestId("steal-focus").click();
    });

    // Simulate deletion — unregister the scope
    act(() => {
      getByTestId("unregister").click();
    });

    // Unmount — should clear to null since scope no longer exists
    act(() => {
      getByTestId("hide").click();
    });
    expect(getByTestId("focused").textContent).toBe("null");
  });

  it("clears focus when there was no previous focus", () => {
    const { getByTestId } = render(
      <EntityFocusProvider>
        <RestoreFocusHarness initialMoniker={null} />
      </EntityFocusProvider>,
    );

    // Mount with no focus
    act(() => {
      getByTestId("show").click();
    });

    // Inspector steals focus
    act(() => {
      getByTestId("steal-focus").click();
    });
    expect(getByTestId("focused").textContent).toBe("task:inspected");

    // Unmount — should restore to null
    act(() => {
      getByTestId("hide").click();
    });
    expect(getByTestId("focused").textContent).toBe("null");
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
    expect(typeof shim.registerClaimPredicates).toBe("function");
    expect(typeof shim.unregisterClaimPredicates).toBe("function");
    expect(typeof shim.broadcastNavCommand).toBe("function");
  });
});
