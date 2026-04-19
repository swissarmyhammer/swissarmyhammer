import { describe, it, expect, vi } from "vitest";
import { render, renderHook, act } from "@testing-library/react";
import {
  EntityFocusProvider,
  useEntityFocus,
  useFocusedMoniker,
  useFocusedScope,
  useIsFocused,
} from "./entity-focus-context";
import { FocusLayer } from "@/components/focus-layer";
import { type CommandScope } from "./command-scope";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(() => Promise.resolve()),
  transformCallback: vi.fn(),
}));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: vi.fn(() => ({ label: "main" })),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));
vi.mock("ulid", () => {
  let counter = 0;
  return { ulid: vi.fn(() => "01TEST" + String(++counter).padStart(20, "0")) };
});
vi.mock("@tauri-apps/plugin-log", () => ({
  error: vi.fn(),
  warn: vi.fn(),
  info: vi.fn(),
  debug: vi.fn(),
  trace: vi.fn(),
  attachConsole: vi.fn(() => Promise.resolve()),
}));

const wrapper = ({ children }: { children: React.ReactNode }) => (
  <EntityFocusProvider><FocusLayer name="test">{children}</FocusLayer></EntityFocusProvider>
);

describe("useEntityFocus", () => {
  it("returns null initially", () => {
    const { result } = renderHook(
      () => {
        const focus = useEntityFocus();
        const focused = useFocusedMoniker();
        return { focus, focused };
      },
      { wrapper },
    );
    expect(result.current.focused).toBeNull();
    expect(result.current.focus.getFocusedMoniker()).toBeNull();
  });

  it("setFocus updates the focused moniker", () => {
    const { result } = renderHook(
      () => {
        const focus = useEntityFocus();
        const focused = useFocusedMoniker();
        return { focus, focused };
      },
      { wrapper },
    );
    act(() => {
      result.current.focus.setFocus("task:abc");
    });
    expect(result.current.focused).toBe("task:abc");
    expect(result.current.focus.getFocusedMoniker()).toBe("task:abc");
  });

  it("setFocus(null) clears focus", () => {
    const { result } = renderHook(
      () => {
        const focus = useEntityFocus();
        const focused = useFocusedMoniker();
        return { focus, focused };
      },
      { wrapper },
    );
    act(() => {
      result.current.focus.setFocus("task:abc");
    });
    act(() => {
      result.current.focus.setFocus(null);
    });
    expect(result.current.focused).toBeNull();
    expect(result.current.focus.getFocusedMoniker()).toBeNull();
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

// ---------------------------------------------------------------------------
// broadcastNavCommand tests — verifies spatial_navigate delegation
// ---------------------------------------------------------------------------

describe("broadcastNavCommand", () => {
  it("invokes spatial_navigate for a valid nav command", async () => {
    const { invoke } = await import("@tauri-apps/api/core");
    (invoke as ReturnType<typeof vi.fn>).mockClear();

    const { result } = renderHook(() => useEntityFocus(), { wrapper });

    // Register a claim so the moniker-to-key mapping exists
    act(() => {
      result.current.registerClaim("key-A", "task:01", () => {});
      result.current.setFocus("task:01");
    });
    await flush();
    (invoke as ReturnType<typeof vi.fn>).mockClear();

    // Broadcast nav.right
    act(() => {
      result.current.broadcastNavCommand("nav.right");
    });

    expect(invoke).toHaveBeenCalledWith("spatial_navigate", {
      key: "key-A",
      direction: "Right",
    });
  });

  it("returns false for unknown command id", async () => {
    const { result } = renderHook(() => useEntityFocus(), { wrapper });

    act(() => {
      result.current.registerClaim("key-A", "task:01", () => {});
      result.current.setFocus("task:01");
    });
    await flush();

    let dispatched = false;
    act(() => {
      dispatched = result.current.broadcastNavCommand("unknown.cmd");
    });
    expect(dispatched).toBe(false);
  });

  it("returns false when no moniker is focused", () => {
    const { result } = renderHook(() => useEntityFocus(), { wrapper });

    let dispatched = false;
    act(() => {
      dispatched = result.current.broadcastNavCommand("nav.right");
    });
    expect(dispatched).toBe(false);
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

/* ---------- spatial focus claim registry ---------- */

describe("spatial focus claim registry", () => {
  it("claim registry calls previous callback with false and next with true on focus-changed event", async () => {
    const { listen } = await import("@tauri-apps/api/event");
    let eventCallback: ((evt: { payload: unknown }) => void) | null = null;
    (listen as ReturnType<typeof vi.fn>).mockImplementation(
      (_event: string, cb: (evt: { payload: unknown }) => void) => {
        eventCallback = cb;
        return Promise.resolve(() => {});
      },
    );

    const cbA = vi.fn();
    const cbB = vi.fn();

    function Harness() {
      const { registerClaim, unregisterClaim } = useEntityFocus();
      return (
        <>
          <button
            data-testid="register"
            onClick={() => {
              registerClaim("key-A", "task:01", cbA);
              registerClaim("key-B", "task:02", cbB);
            }}
          />
          <button
            data-testid="unregister"
            onClick={() => {
              unregisterClaim("key-A");
              unregisterClaim("key-B");
            }}
          />
        </>
      );
    }

    const { getByTestId } = render(
      <EntityFocusProvider>
        <Harness />
      </EntityFocusProvider>,
    );
    await flush();

    // Register claims
    await act(async () => {
      getByTestId("register").click();
    });

    // Fire focus-changed: nothing → key-A
    await act(async () => {
      eventCallback!({ payload: { prev_key: null, next_key: "key-A" } });
    });
    expect(cbA).toHaveBeenCalledWith(true);
    expect(cbB).not.toHaveBeenCalled();

    cbA.mockClear();
    cbB.mockClear();

    // Fire focus-changed: key-A → key-B
    await act(async () => {
      eventCallback!({ payload: { prev_key: "key-A", next_key: "key-B" } });
    });
    expect(cbA).toHaveBeenCalledWith(false);
    expect(cbB).toHaveBeenCalledWith(true);
  });

  it("unregistered key in focus-changed event is a no-op", async () => {
    const { listen } = await import("@tauri-apps/api/event");
    let eventCallback: ((evt: { payload: unknown }) => void) | null = null;
    (listen as ReturnType<typeof vi.fn>).mockImplementation(
      (_event: string, cb: (evt: { payload: unknown }) => void) => {
        eventCallback = cb;
        return Promise.resolve(() => {});
      },
    );

    const cbA = vi.fn();

    function Harness() {
      const { registerClaim } = useEntityFocus();
      return (
        <button
          data-testid="register"
          onClick={() => registerClaim("key-A", "task:01", cbA)}
        />
      );
    }

    const { getByTestId } = render(
      <EntityFocusProvider>
        <Harness />
      </EntityFocusProvider>,
    );
    await flush();

    await act(async () => {
      getByTestId("register").click();
    });

    // Fire event referencing a nonexistent prev_key — should not throw
    await act(async () => {
      eventCallback!({
        payload: { prev_key: "nonexistent", next_key: "key-A" },
      });
    });
    expect(cbA).toHaveBeenCalledWith(true);
  });

  it("EntityFocusProvider unmount cleans up event listener", async () => {
    const { listen } = await import("@tauri-apps/api/event");
    const unsub = vi.fn();
    (listen as ReturnType<typeof vi.fn>).mockImplementation(() =>
      Promise.resolve(unsub),
    );

    const { unmount } = render(
      <EntityFocusProvider>
        <div />
      </EntityFocusProvider>,
    );
    await flush();

    // listen should have been called for "focus-changed"
    expect(listen).toHaveBeenCalledWith("focus-changed", expect.any(Function));

    unmount();
    await flush();

    // The unsub function should have been called on unmount
    expect(unsub).toHaveBeenCalled();
  });
});
