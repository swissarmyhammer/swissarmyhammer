import { describe, it, expect, vi } from "vitest";
import { render, renderHook, act } from "@testing-library/react";
import { useState } from "react";
import {
  EntityFocusProvider,
  useEntityFocus,
  useFocusedScope,
  useIsFocused,
  type ClaimPredicate,
} from "./entity-focus-context";
import { FocusClaim, FocusScope } from "@/components/focus-scope";
import {
  CommandScopeProvider,
  type CommandDef,
  type CommandScope,
} from "./command-scope";

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

// ---------------------------------------------------------------------------
// Claim stack tests (FocusClaim component integration)
// ---------------------------------------------------------------------------

/** Flush microtasks and pending layout effects. */
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

/** Reads a scope from the registry and renders its moniker. */
function ScopeMonitor({ moniker }: { moniker: string }) {
  const { getScope } = useEntityFocus();
  const scope = getScope(moniker);
  return (
    <span data-testid="scope-monitor">
      {scope ? scope.moniker ?? "no-moniker" : "null"}
    </span>
  );
}

/** Conditionally renders a FocusClaim for mount/unmount testing. */
function ConditionalClaim({
  show,
  moniker,
}: {
  show: boolean;
  moniker: string;
}) {
  return show ? <FocusClaim moniker={moniker} /> : null;
}

/** Harness with two independently togglable claims. */
function TwoClaimHarness({
  initialA,
  initialB,
  monikerA,
  monikerB,
}: {
  initialA: boolean;
  initialB: boolean;
  monikerA: string;
  monikerB: string;
}) {
  const [showA, setShowA] = useState(initialA);
  const [showB, setShowB] = useState(initialB);
  return (
    <>
      <ConditionalClaim show={showA} moniker={monikerA} />
      <ConditionalClaim show={showB} moniker={monikerB} />
      <FocusMonitor />
      <button data-testid="toggle-a" onClick={() => setShowA((s) => !s)} />
      <button data-testid="toggle-b" onClick={() => setShowB((s) => !s)} />
    </>
  );
}

/** Harness whose single claim's moniker can change dynamically. */
function MutableMonikerHarness({ initial }: { initial: string }) {
  const [moniker, setMoniker] = useState(initial);
  return (
    <>
      <FocusClaim moniker={moniker} />
      <FocusMonitor />
      <button
        data-testid="set-moniker"
        onClick={() => setMoniker("task:updated")}
      />
    </>
  );
}

/** Harness for testing moniker update on the non-active (first) claim. */
function MutableNonActiveMonikerHarness() {
  const [monikerA, setMonikerA] = useState("task:a");
  return (
    <>
      <FocusClaim moniker={monikerA} />
      <FocusClaim moniker="task:b" />
      <FocusMonitor />
      <button
        data-testid="set-moniker-a"
        onClick={() => setMonikerA("task:a-updated")}
      />
    </>
  );
}

const TEST_COMMANDS: CommandDef[] = [{ id: "test.hello", name: "Hello" }];

/**
 * Minimal provider tree: EntityFocusProvider + CommandScopeProvider
 * (FocusClaim reads CommandScopeContext to build its scope).
 */
function ClaimProviders({ children }: { children: React.ReactNode }) {
  return (
    <EntityFocusProvider>
      <CommandScopeProvider commands={TEST_COMMANDS} moniker="root">
        {children}
      </CommandScopeProvider>
    </EntityFocusProvider>
  );
}

describe("claim stack (FocusClaim)", () => {
  it("single claim sets focusedMoniker", async () => {
    const { getByTestId } = render(
      <ClaimProviders>
        <FocusClaim moniker="task:1" />
        <FocusMonitor />
      </ClaimProviders>,
    );
    await flush();
    expect(getByTestId("focus-monitor").textContent).toBe("task:1");
  });

  it("two claims — LIFO: later mount wins", async () => {
    const { getByTestId } = render(
      <ClaimProviders>
        <TwoClaimHarness
          initialA={true}
          initialB={true}
          monikerA="task:a"
          monikerB="task:b"
        />
      </ClaimProviders>,
    );
    await flush();
    // B is mounted second so its claim ID is higher — it wins.
    expect(getByTestId("focus-monitor").textContent).toBe("task:b");
  });

  it("pop active claim restores previous", async () => {
    const { getByTestId } = render(
      <ClaimProviders>
        <TwoClaimHarness
          initialA={true}
          initialB={true}
          monikerA="task:a"
          monikerB="task:b"
        />
      </ClaimProviders>,
    );
    await flush();
    expect(getByTestId("focus-monitor").textContent).toBe("task:b");

    // Unmount B (the active claim) — focus should fall back to A
    await act(async () => {
      getByTestId("toggle-b").click();
      await new Promise((r) => setTimeout(r, 0));
    });
    expect(getByTestId("focus-monitor").textContent).toBe("task:a");
  });

  it("pop non-active claim doesn't change focus", async () => {
    const { getByTestId } = render(
      <ClaimProviders>
        <TwoClaimHarness
          initialA={true}
          initialB={true}
          monikerA="task:a"
          monikerB="task:b"
        />
      </ClaimProviders>,
    );
    await flush();
    expect(getByTestId("focus-monitor").textContent).toBe("task:b");

    // Unmount A (non-active) — focus stays on B
    await act(async () => {
      getByTestId("toggle-a").click();
      await new Promise((r) => setTimeout(r, 0));
    });
    expect(getByTestId("focus-monitor").textContent).toBe("task:b");
  });

  it("moniker update on active claim changes focusedMoniker", async () => {
    const { getByTestId } = render(
      <ClaimProviders>
        <MutableMonikerHarness initial="task:original" />
      </ClaimProviders>,
    );
    await flush();
    expect(getByTestId("focus-monitor").textContent).toBe("task:original");

    await act(async () => {
      getByTestId("set-moniker").click();
      await new Promise((r) => setTimeout(r, 0));
    });
    expect(getByTestId("focus-monitor").textContent).toBe("task:updated");
  });

  it("moniker update on non-active claim doesn't change focus", async () => {
    const { getByTestId } = render(
      <ClaimProviders>
        <MutableNonActiveMonikerHarness />
      </ClaimProviders>,
    );
    await flush();
    // B is active (mounted second, higher claim ID)
    expect(getByTestId("focus-monitor").textContent).toBe("task:b");

    // Change A's moniker — should NOT affect entity focus
    await act(async () => {
      getByTestId("set-moniker-a").click();
      await new Promise((r) => setTimeout(r, 0));
    });
    expect(getByTestId("focus-monitor").textContent).toBe("task:b");
  });

  it("pop all claims: focusedMoniker is null", async () => {
    const { getByTestId } = render(
      <ClaimProviders>
        <TwoClaimHarness
          initialA={true}
          initialB={false}
          monikerA="task:a"
          monikerB="task:b"
        />
      </ClaimProviders>,
    );
    await flush();
    expect(getByTestId("focus-monitor").textContent).toBe("task:a");

    // Unmount A (the only claim)
    await act(async () => {
      getByTestId("toggle-a").click();
      await new Promise((r) => setTimeout(r, 0));
    });
    expect(getByTestId("focus-monitor").textContent).toBe("null");
  });

  it("claim registers scope for its moniker via getScope", async () => {
    const { getByTestId } = render(
      <EntityFocusProvider>
        <CommandScopeProvider commands={TEST_COMMANDS} moniker="task:scoped">
          <FocusClaim moniker="task:scoped" />
          <ScopeMonitor moniker="task:scoped" />
        </CommandScopeProvider>
      </EntityFocusProvider>,
    );
    await flush();
    // The scope registered by FocusClaim should carry the moniker from
    // CommandScopeProvider
    expect(getByTestId("scope-monitor").textContent).toBe("task:scoped");
  });
});

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
