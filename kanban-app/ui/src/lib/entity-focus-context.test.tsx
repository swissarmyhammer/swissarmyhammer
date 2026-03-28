import { describe, it, expect, vi } from "vitest";
import { render, renderHook, act } from "@testing-library/react";
import { useState } from "react";
import {
  EntityFocusProvider,
  useEntityFocus,
  useFocusedScope,
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
        <FocusScope moniker="panel:temp" commands={[]} claimWhen={claimPredicates}>
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
  return (
    <button data-testid="set-focus" onClick={() => setFocus(moniker)} />
  );
}

/** Helper button to call broadcastNavCommand. */
function BroadcastButton({ commandId }: { commandId: string }) {
  const { broadcastNavCommand } = useEntityFocus();
  return (
    <button data-testid="broadcast" onClick={() => broadcastNavCommand(commandId)} />
  );
}

describe("useRestoreFocus", () => {
  /**
   * Helper component that conditionally renders a child that calls useRestoreFocus.
   * The parent manages focus state and controls when the child mounts/unmounts.
   */
  function RestoreFocusHarness({ initialMoniker, scope }: { initialMoniker: string | null; scope?: CommandScope }) {
    const [showChild, setShowChild] = useState(false);
    const focus = useEntityFocus();

    return (
      <>
        <button data-testid="setup" onClick={() => {
          if (initialMoniker && scope) {
            focus.registerScope(initialMoniker, scope);
          }
          if (initialMoniker) {
            focus.setFocus(initialMoniker);
          }
        }} />
        <button data-testid="show" onClick={() => setShowChild(true)} />
        <button data-testid="hide" onClick={() => setShowChild(false)} />
        <button data-testid="steal-focus" onClick={() => focus.setFocus("task:inspected")} />
        <button data-testid="unregister" onClick={() => {
          if (initialMoniker) focus.unregisterScope(initialMoniker);
        }} />
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
    const scope: CommandScope = { commands: new Map(), parent: null, moniker: "task:abc" };
    const { getByTestId } = render(
      <EntityFocusProvider>
        <RestoreFocusHarness initialMoniker="task:abc" scope={scope} />
      </EntityFocusProvider>,
    );

    // 1. Set up focus
    act(() => { getByTestId("setup").click(); });
    expect(getByTestId("focused").textContent).toBe("task:abc");

    // 2. Mount the restore hook — captures "task:abc"
    act(() => { getByTestId("show").click(); });
    expect(getByTestId("child")).toBeTruthy();

    // 3. Inspector steals focus
    act(() => { getByTestId("steal-focus").click(); });
    expect(getByTestId("focused").textContent).toBe("task:inspected");

    // 4. Unmount — should restore to "task:abc"
    act(() => { getByTestId("hide").click(); });
    expect(getByTestId("focused").textContent).toBe("task:abc");
  });

  it("clears focus when saved moniker no longer exists in registry", () => {
    const scope: CommandScope = { commands: new Map(), parent: null, moniker: "task:abc" };
    const { getByTestId } = render(
      <EntityFocusProvider>
        <RestoreFocusHarness initialMoniker="task:abc" scope={scope} />
      </EntityFocusProvider>,
    );

    act(() => { getByTestId("setup").click(); });
    act(() => { getByTestId("show").click(); });
    act(() => { getByTestId("steal-focus").click(); });

    // Simulate deletion — unregister the scope
    act(() => { getByTestId("unregister").click(); });

    // Unmount — should clear to null since scope no longer exists
    act(() => { getByTestId("hide").click(); });
    expect(getByTestId("focused").textContent).toBe("null");
  });

  it("clears focus when there was no previous focus", () => {
    const { getByTestId } = render(
      <EntityFocusProvider>
        <RestoreFocusHarness initialMoniker={null} />
      </EntityFocusProvider>,
    );

    // Mount with no focus
    act(() => { getByTestId("show").click(); });

    // Inspector steals focus
    act(() => { getByTestId("steal-focus").click(); });
    expect(getByTestId("focused").textContent).toBe("task:inspected");

    // Unmount — should restore to null
    act(() => { getByTestId("hide").click(); });
    expect(getByTestId("focused").textContent).toBe("null");
  });
});
