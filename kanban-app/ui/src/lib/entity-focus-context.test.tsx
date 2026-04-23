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
vi.mock("@tauri-apps/api/webviewWindow", () => ({
  getCurrentWebviewWindow: vi.fn(() => ({
    label: "main",
    listen: vi.fn(() => Promise.resolve(() => {})),
  })),
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
  <EntityFocusProvider>
    <FocusLayer name="test">{children}</FocusLayer>
  </EntityFocusProvider>
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

/* ---------- spatial key registry + focus-changed store updates ---------- */

describe("spatial key registry and focus-changed store updates", () => {
  it("focus-changed event updates the focused-moniker store to the moniker bound to next_key", async () => {
    const { getCurrentWebviewWindow } =
      await import("@tauri-apps/api/webviewWindow");
    let eventCallback: ((evt: { payload: unknown }) => void) | null = null;
    const webviewListen = vi.fn(
      (_event: string, cb: (evt: { payload: unknown }) => void) => {
        eventCallback = cb;
        return Promise.resolve(() => {});
      },
    );
    (getCurrentWebviewWindow as ReturnType<typeof vi.fn>).mockReturnValue({
      label: "main",
      listen: webviewListen,
    });

    function Harness() {
      const { registerSpatialKey } = useEntityFocus();
      const focused = useFocusedMoniker();
      return (
        <>
          <button
            data-testid="register"
            onClick={() => {
              registerSpatialKey("key-A", "task:01");
              registerSpatialKey("key-B", "task:02");
            }}
          />
          <span data-testid="focused">{focused ?? "null"}</span>
        </>
      );
    }

    const { getByTestId } = render(
      <EntityFocusProvider>
        <Harness />
      </EntityFocusProvider>,
    );
    await flush();

    // Register both keys
    await act(async () => {
      getByTestId("register").click();
    });
    expect(getByTestId("focused").textContent).toBe("null");

    // Fire focus-changed: nothing → key-A; store should reflect task:01
    await act(async () => {
      eventCallback!({ payload: { prev_key: null, next_key: "key-A" } });
    });
    expect(getByTestId("focused").textContent).toBe("task:01");

    // Fire focus-changed: key-A → key-B; store should reflect task:02
    await act(async () => {
      eventCallback!({ payload: { prev_key: "key-A", next_key: "key-B" } });
    });
    expect(getByTestId("focused").textContent).toBe("task:02");
  });

  it("focus-changed with an unknown next_key clears the store (no moniker bound)", async () => {
    const { getCurrentWebviewWindow } =
      await import("@tauri-apps/api/webviewWindow");
    let eventCallback: ((evt: { payload: unknown }) => void) | null = null;
    const webviewListen = vi.fn(
      (_event: string, cb: (evt: { payload: unknown }) => void) => {
        eventCallback = cb;
        return Promise.resolve(() => {});
      },
    );
    (getCurrentWebviewWindow as ReturnType<typeof vi.fn>).mockReturnValue({
      label: "main",
      listen: webviewListen,
    });

    function Harness() {
      const { registerSpatialKey, setFocus } = useEntityFocus();
      const focused = useFocusedMoniker();
      return (
        <>
          <button
            data-testid="seed"
            onClick={() => {
              registerSpatialKey("key-A", "task:01");
              setFocus("task:01");
            }}
          />
          <span data-testid="focused">{focused ?? "null"}</span>
        </>
      );
    }

    const { getByTestId } = render(
      <EntityFocusProvider>
        <Harness />
      </EntityFocusProvider>,
    );
    await flush();

    // Seed a focused moniker.
    await act(async () => {
      getByTestId("seed").click();
    });
    expect(getByTestId("focused").textContent).toBe("task:01");

    // Fire event with an unknown next_key — the listener must not throw
    // and must clear the focused moniker (no moniker bound → null).
    await act(async () => {
      eventCallback!({
        payload: { prev_key: "key-A", next_key: "nonexistent" },
      });
    });
    expect(getByTestId("focused").textContent).toBe("null");
  });

  it("EntityFocusProvider unmount cleans up event listener", async () => {
    const { getCurrentWebviewWindow } =
      await import("@tauri-apps/api/webviewWindow");
    const unsub = vi.fn();
    const webviewListen = vi.fn(() => Promise.resolve(unsub));
    (getCurrentWebviewWindow as ReturnType<typeof vi.fn>).mockReturnValue({
      label: "main",
      listen: webviewListen,
    });

    const { unmount } = render(
      <EntityFocusProvider>
        <div />
      </EntityFocusProvider>,
    );
    await flush();

    // listen should have been called on the current webview window
    // for "focus-changed" so the listener is scoped to this window.
    expect(webviewListen).toHaveBeenCalledWith(
      "focus-changed",
      expect.any(Function),
    );

    unmount();
    await flush();

    // The unsub function should have been called on unmount
    expect(unsub).toHaveBeenCalled();
  });
});
