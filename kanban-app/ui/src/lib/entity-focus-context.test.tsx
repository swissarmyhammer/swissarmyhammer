import { describe, it, expect, vi } from "vitest";
import { useState } from "react";
import { renderHook, render, act } from "@testing-library/react";
import { EntityFocusProvider, useEntityFocus, useFocusedScope, useIsFocused, useRestoreFocus } from "./entity-focus-context";
import type { CommandScope } from "./command-scope";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(() => Promise.resolve()),
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
    act(() => { result.current.setFocus("task:abc"); });
    expect(result.current.focusedMoniker).toBe("task:abc");
  });

  it("setFocus(null) clears focus", () => {
    const { result } = renderHook(() => useEntityFocus(), { wrapper });
    act(() => { result.current.setFocus("task:abc"); });
    act(() => { result.current.setFocus(null); });
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
    const scope: CommandScope = { commands: new Map(), parent: null, moniker: "task:abc" };

    act(() => { result.current.registerScope("task:abc", scope); });
    expect(result.current.getScope("task:abc")).toBe(scope);

    act(() => { result.current.unregisterScope("task:abc"); });
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
    const scope: CommandScope = { commands: new Map(), parent: null, moniker: "task:abc" };

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

    act(() => { result.current.focus.setFocus("task:missing"); });
    expect(result.current.focusedScope).toBeNull();
  });
});

describe("useIsFocused", () => {
  it("returns false when nothing is focused", () => {
    const { result } = renderHook(() => useIsFocused("task:abc"), { wrapper });
    expect(result.current).toBe(false);
  });

  it("returns true for direct match", () => {
    const scope: CommandScope = { commands: new Map(), parent: null, moniker: "task:abc" };
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
    const parentScope: CommandScope = { commands: new Map(), parent: null, moniker: "column:col1" };
    const childScope: CommandScope = { commands: new Map(), parent: parentScope, moniker: "task:abc" };

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
    const scope: CommandScope = { commands: new Map(), parent: null, moniker: "task:abc" };
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
