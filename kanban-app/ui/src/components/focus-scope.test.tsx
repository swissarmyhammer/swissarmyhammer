import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, fireEvent } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import {
  EntityFocusProvider,
  useEntityFocus,
  useIsFocused,
} from "@/lib/entity-focus-context";
import { FocusScope, useParentFocusScope } from "./focus-scope";
import { CommandScopeProvider } from "@/lib/command-scope";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(() => Promise.resolve()),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ label: "main" }),
}));

/** Helper to read focus state from inside the provider. */
function FocusReader() {
  const { focusedMoniker } = useEntityFocus();
  return <div data-testid="focus-reader">{focusedMoniker ?? "null"}</div>;
}

function renderWithFocus(ui: React.ReactElement) {
  return render(
    <EntityFocusProvider>
      <FocusReader />
      {ui}
    </EntityFocusProvider>,
  );
}

describe("FocusScope", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    (invoke as ReturnType<typeof vi.fn>).mockResolvedValue(undefined);
  });

  it("click sets entity focus to moniker", () => {
    const { getByTestId, getByText } = renderWithFocus(
      <FocusScope moniker="task:abc" commands={[]}>
        <span>card</span>
      </FocusScope>,
    );
    fireEvent.click(getByText("card"));
    expect(getByTestId("focus-reader").textContent).toBe("task:abc");
  });

  it("right-click sets entity focus and calls show_context_menu", () => {
    const execute = vi.fn();
    const { getByTestId, getByText } = renderWithFocus(
      <FocusScope
        moniker="task:abc"
        commands={[
          { id: "entity.inspect", name: "Inspect", contextMenu: true, execute },
        ]}
      >
        <span>card</span>
      </FocusScope>,
    );
    fireEvent.contextMenu(getByText("card"));
    expect(getByTestId("focus-reader").textContent).toBe("task:abc");
    expect(invoke).toHaveBeenCalledWith("show_context_menu", {
      items: [{ id: "entity.inspect", name: "Inspect" }],
    });
  });

  it("clicking input inside does not change entity focus", () => {
    const { getByTestId, getByRole } = renderWithFocus(
      <FocusScope moniker="task:abc" commands={[]}>
        <input type="text" />
      </FocusScope>,
    );
    fireEvent.click(getByRole("textbox"));
    expect(getByTestId("focus-reader").textContent).toBe("null");
  });

  it("nested FocusScope: inner click sets inner moniker", () => {
    const { getByTestId, getByText } = renderWithFocus(
      <FocusScope moniker="task:abc" commands={[]}>
        <span>card</span>
        <FocusScope moniker="tag:xyz" commands={[]}>
          <span>tag</span>
        </FocusScope>
      </FocusScope>,
    );
    fireEvent.click(getByText("tag"));
    expect(getByTestId("focus-reader").textContent).toBe("tag:xyz");
  });

  it("nested FocusScope: inner right-click stops propagation", () => {
    const outerExec = vi.fn();
    const innerExec = vi.fn();
    const { getByText } = renderWithFocus(
      <FocusScope
        moniker="task:abc"
        commands={[
          {
            id: "outer.cmd",
            name: "Outer",
            contextMenu: true,
            execute: outerExec,
          },
        ]}
      >
        <span>card</span>
        <FocusScope
          moniker="tag:xyz"
          commands={[
            {
              id: "inner.cmd",
              name: "Inner",
              contextMenu: true,
              execute: innerExec,
            },
          ]}
        >
          <span>tag</span>
        </FocusScope>
      </FocusScope>,
    );
    fireEvent.contextMenu(getByText("tag"));
    // show_context_menu should be called exactly once (inner scope handles it, stopPropagation prevents outer)
    const ctxCalls = (invoke as ReturnType<typeof vi.fn>).mock.calls.filter(
      (c: unknown[]) => c[0] === "show_context_menu",
    );
    expect(ctxCalls).toHaveLength(1);
    const call = ctxCalls[0];
    // Inner scope should show both inner and outer commands (scope chain walks up)
    const items = call[1].items;
    expect(
      items.find((i: { id: string }) => i.id === "inner.cmd"),
    ).toBeTruthy();
    expect(
      items.find((i: { id: string }) => i.id === "outer.cmd"),
    ).toBeTruthy();
  });

  it("nested FocusScope: same command ID without target shadows — inner wins", () => {
    const outerExec = vi.fn();
    const innerExec = vi.fn();
    const { getByText } = renderWithFocus(
      <FocusScope
        moniker="task:abc"
        commands={[
          {
            id: "entity.inspect",
            name: "Inspect task",
            contextMenu: true,
            execute: outerExec,
          },
        ]}
      >
        <span>card</span>
        <FocusScope
          moniker="tag:xyz"
          commands={[
            {
              id: "entity.inspect",
              name: "Inspect tag",
              contextMenu: true,
              execute: innerExec,
            },
          ]}
        >
          <span>tag</span>
        </FocusScope>
      </FocusScope>,
    );
    fireEvent.contextMenu(getByText("tag"));
    const ctxCalls = (invoke as ReturnType<typeof vi.fn>).mock.calls.filter(
      (c: unknown[]) => c[0] === "show_context_menu",
    );
    expect(ctxCalls).toHaveLength(1);
    const call = ctxCalls[0];
    const items = call[1].items;
    // No target → shadow by id alone: inner "Inspect tag" shadows outer "Inspect task"
    expect(items).toHaveLength(1);
    expect(items[0]).toEqual({ id: "entity.inspect", name: "Inspect tag" });
  });

  it("nested FocusScope: same command ID with different targets accumulates both", () => {
    const outerExec = vi.fn();
    const innerExec = vi.fn();
    const { getByText } = renderWithFocus(
      <FocusScope
        moniker="task:abc"
        commands={[
          {
            id: "entity.inspect",
            name: "Inspect task",
            target: "task:abc",
            contextMenu: true,
            execute: outerExec,
          },
        ]}
      >
        <span>card</span>
        <FocusScope
          moniker="tag:xyz"
          commands={[
            {
              id: "entity.inspect",
              name: "Inspect tag",
              target: "tag:xyz",
              contextMenu: true,
              execute: innerExec,
            },
          ]}
        >
          <span>tag</span>
        </FocusScope>
      </FocusScope>,
    );
    fireEvent.contextMenu(getByText("tag"));
    const ctxCalls = (invoke as ReturnType<typeof vi.fn>).mock.calls.filter(
      (c: unknown[]) => c[0] === "show_context_menu",
    );
    expect(ctxCalls).toHaveLength(1);
    const call = ctxCalls[0];
    const items = call[1].items;
    // Different targets → both accumulate (separator inserted between depth groups)
    const commandItems = items.filter(
      (i: { id: string }) => i.id !== "__separator__",
    );
    expect(commandItems).toHaveLength(2);
    expect(
      commandItems.find((i: { name: string }) => i.name === "Inspect tag"),
    ).toBeTruthy();
    expect(
      commandItems.find((i: { name: string }) => i.name === "Inspect task"),
    ).toBeTruthy();
  });

  it("nested FocusScope: same command ID with same target shadows — inner wins", () => {
    const outerExec = vi.fn();
    const innerExec = vi.fn();
    const { getByText } = renderWithFocus(
      <FocusScope
        moniker="task:abc"
        commands={[
          {
            id: "entity.inspect",
            name: "Inspect task outer",
            target: "task:abc",
            contextMenu: true,
            execute: outerExec,
          },
        ]}
      >
        <span>card</span>
        <FocusScope
          moniker="tag:xyz"
          commands={[
            {
              id: "entity.inspect",
              name: "Inspect task inner",
              target: "task:abc",
              contextMenu: true,
              execute: innerExec,
            },
          ]}
        >
          <span>tag</span>
        </FocusScope>
      </FocusScope>,
    );
    fireEvent.contextMenu(getByText("tag"));
    const ctxCalls = (invoke as ReturnType<typeof vi.fn>).mock.calls.filter(
      (c: unknown[]) => c[0] === "show_context_menu",
    );
    expect(ctxCalls).toHaveLength(1);
    const call = ctxCalls[0];
    const items = call[1].items;
    // Same target → shadow: inner wins
    expect(items).toHaveLength(1);
    expect(items[0].name).toBe("Inspect task inner");
  });

  it("nested FocusScope: unavailable inner command blocks same (id, target) from parent", () => {
    const outerExec = vi.fn();
    const { getByText } = renderWithFocus(
      <FocusScope
        moniker="task:abc"
        commands={[
          {
            id: "entity.inspect",
            name: "Inspect task",
            contextMenu: true,
            execute: outerExec,
          },
        ]}
      >
        <span>card</span>
        <FocusScope
          moniker="tag:xyz"
          commands={[
            {
              id: "entity.inspect",
              name: "Inspect tag",
              contextMenu: true,
              available: false,
            },
          ]}
        >
          <span>tag</span>
        </FocusScope>
      </FocusScope>,
    );
    fireEvent.contextMenu(getByText("tag"));
    // Both have no target → same shadow key. Unavailable inner blocks outer.
    expect(invoke).not.toHaveBeenCalledWith(
      "show_context_menu",
      expect.anything(),
    );
  });

  it("double-click executes entity.inspect command", () => {
    const execute = vi.fn();
    const { getByText } = renderWithFocus(
      <FocusScope
        moniker="task:abc"
        commands={[
          { id: "entity.inspect", name: "Inspect", contextMenu: true, execute },
        ]}
      >
        <span>card</span>
      </FocusScope>,
    );
    fireEvent.doubleClick(getByText("card"));
    expect(execute).toHaveBeenCalledTimes(1);
  });

  it("double-click on INPUT does not trigger entity.inspect", () => {
    const execute = vi.fn();
    const { getByRole } = renderWithFocus(
      <FocusScope
        moniker="task:abc"
        commands={[
          { id: "entity.inspect", name: "Inspect", contextMenu: true, execute },
        ]}
      >
        <input type="text" />
      </FocusScope>,
    );
    fireEvent.doubleClick(getByRole("textbox"));
    expect(execute).not.toHaveBeenCalled();
  });

  it("double-click propagation stops at innermost FocusScope", () => {
    const outerExec = vi.fn();
    const innerExec = vi.fn();
    const { getByText } = renderWithFocus(
      <FocusScope
        moniker="task:abc"
        commands={[
          {
            id: "entity.inspect",
            name: "Inspect task",
            contextMenu: true,
            execute: outerExec,
          },
        ]}
      >
        <span>card</span>
        <FocusScope
          moniker="tag:xyz"
          commands={[
            {
              id: "entity.inspect",
              name: "Inspect tag",
              contextMenu: true,
              execute: innerExec,
            },
          ]}
        >
          <span>tag</span>
        </FocusScope>
      </FocusScope>,
    );
    fireEvent.doubleClick(getByText("tag"));
    // Inner scope's entity.inspect fires (resolveCommand finds nearest)
    expect(innerExec).toHaveBeenCalledTimes(1);
    // Outer does NOT fire because stopPropagation prevents the event from reaching it
    expect(outerExec).not.toHaveBeenCalled();
  });

  it("double-click does nothing when no entity.inspect command exists", () => {
    const execute = vi.fn();
    const { getByText } = renderWithFocus(
      <FocusScope
        moniker="task:abc"
        commands={[
          { id: "other.command", name: "Other", contextMenu: true, execute },
        ]}
      >
        <span>card</span>
      </FocusScope>,
    );
    // Should not throw
    fireEvent.doubleClick(getByText("card"));
    expect(execute).not.toHaveBeenCalled();
  });

  it("data-focused attribute set when focused", () => {
    const { container, getByText } = renderWithFocus(
      <FocusScope moniker="task:abc" commands={[]}>
        <span>card</span>
      </FocusScope>,
    );
    fireEvent.click(getByText("card"));
    const scopeDiv = container.querySelector("[data-moniker='task:abc']");
    expect(scopeDiv?.hasAttribute("data-focused")).toBe(true);
  });

  it("data-focused attribute absent when not focused", () => {
    const { container } = renderWithFocus(
      <FocusScope moniker="task:abc" commands={[]}>
        <span>card</span>
      </FocusScope>,
    );
    const scopeDiv = container.querySelector("[data-moniker='task:abc']");
    expect(scopeDiv?.hasAttribute("data-focused")).toBe(false);
  });

  it("data-moniker attribute always set", () => {
    const { container } = renderWithFocus(
      <FocusScope moniker="task:abc" commands={[]}>
        <span>card</span>
      </FocusScope>,
    );
    const scopeDiv = container.querySelector("[data-moniker='task:abc']");
    expect(scopeDiv).not.toBeNull();
    expect(scopeDiv?.getAttribute("data-moniker")).toBe("task:abc");
  });

  it("commands are provided to CommandScopeProvider", () => {
    const execute = vi.fn();
    const { getByText } = renderWithFocus(
      <FocusScope
        moniker="task:abc"
        commands={[
          { id: "entity.inspect", name: "Inspect", contextMenu: true, execute },
        ]}
      >
        <span>card</span>
      </FocusScope>,
    );
    fireEvent.contextMenu(getByText("card"));
    expect(invoke).toHaveBeenCalledWith("show_context_menu", {
      items: [{ id: "entity.inspect", name: "Inspect" }],
    });
  });

  it("scope is registered on mount and deregistered on unmount", () => {
    // The registry is a ref, so we use a probe that reads getScope imperatively.
    let probeGetScope: ((m: string) => unknown) | null = null;
    function ScopeProbe() {
      const { getScope } = useEntityFocus();
      probeGetScope = getScope;
      return null;
    }

    const { unmount } = render(
      <EntityFocusProvider>
        <ScopeProbe />
        <FocusScope moniker="task:abc" commands={[]}>
          <span>card</span>
        </FocusScope>
      </EntityFocusProvider>,
    );

    // After mount + effects, scope should be registered in the ref
    expect(probeGetScope!("task:abc")).not.toBeNull();
    unmount();
    // After unmount, the cleanup should have deregistered
    expect(probeGetScope!("task:abc")).toBeNull();
  });

  describe("useParentFocusScope", () => {
    /** Helper that reads useParentFocusScope and renders the value. */
    function ParentScopeReader() {
      const parentMoniker = useParentFocusScope();
      return <span data-testid="parent-scope">{parentMoniker ?? "null"}</span>;
    }

    it("returns parent FocusScope moniker", () => {
      const { getByTestId } = render(
        <EntityFocusProvider>
          <FocusScope moniker="column:col1" commands={[]}>
            <ParentScopeReader />
          </FocusScope>
        </EntityFocusProvider>,
      );
      expect(getByTestId("parent-scope").textContent).toBe("column:col1");
    });

    it("skips CommandScopeProvider, returns grandparent FocusScope moniker", () => {
      const { getByTestId } = render(
        <EntityFocusProvider>
          <FocusScope moniker="column:col1" commands={[]}>
            <CommandScopeProvider commands={[]} moniker="inner-cmd">
              <ParentScopeReader />
            </CommandScopeProvider>
          </FocusScope>
        </EntityFocusProvider>,
      );
      // CommandScopeProvider is NOT a FocusScope, so context still shows the FocusScope ancestor
      expect(getByTestId("parent-scope").textContent).toBe("column:col1");
    });

    it("returns null at root", () => {
      const { getByTestId } = render(
        <EntityFocusProvider>
          <ParentScopeReader />
        </EntityFocusProvider>,
      );
      expect(getByTestId("parent-scope").textContent).toBe("null");
    });
  });

  it("useIsFocused ancestor: column gets data-focused when card inside is focused", () => {
    /** Column component that reads useIsFocused. */
    function ColumnWithFocus({
      moniker,
      children,
    }: {
      moniker: string;
      children: React.ReactNode;
    }) {
      const focused = useIsFocused(moniker);
      return (
        <div data-testid={`col-${moniker}`} data-focused={focused || undefined}>
          {children}
        </div>
      );
    }

    const { getByTestId, getByText } = render(
      <EntityFocusProvider>
        <FocusScope moniker="column:col1" commands={[]}>
          <ColumnWithFocus moniker="column:col1">
            <FocusScope moniker="task:abc" commands={[]}>
              <span>card</span>
            </FocusScope>
          </ColumnWithFocus>
        </FocusScope>
      </EntityFocusProvider>,
    );

    // Click the card to focus task:abc
    fireEvent.click(getByText("card"));

    // The column should also show as focused via ancestor walk
    expect(getByTestId("col-column:col1").hasAttribute("data-focused")).toBe(
      true,
    );
  });
});
