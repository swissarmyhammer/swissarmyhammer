import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, fireEvent, waitFor } from "@testing-library/react";
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

/**
 * Shape returned by the backend `list_commands_for_scope`.
 * Used to build mock responses for context menu tests.
 */
interface ResolvedCommand {
  id: string;
  name: string;
  target?: string;
  group: string;
  context_menu: boolean;
  keys?: { vim?: string; cua?: string; emacs?: string };
  available: boolean;
}

/**
 * Helper: configure invoke mock to return the given commands when
 * `list_commands_for_scope` is called, and resolve for everything else.
 */
function mockListCommands(commands: ResolvedCommand[]) {
  (invoke as ReturnType<typeof vi.fn>).mockImplementation(
    (cmd: string, _args?: unknown) => {
      if (cmd === "list_commands_for_scope") return Promise.resolve(commands);
      return Promise.resolve();
    },
  );
}

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

  it("right-click sets entity focus and calls show_context_menu", async () => {
    mockListCommands([
      {
        id: "entity.inspect",
        name: "Inspect",
        group: "entity",
        context_menu: true,
        available: true,
      },
    ]);
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
    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("show_context_menu", {
        items: [
          expect.objectContaining({
            cmd: "entity.inspect",
            name: "Inspect",
            separator: false,
          }),
        ],
      });
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

  it("nested FocusScope: inner right-click stops propagation", async () => {
    // Backend returns both inner and outer commands (scope chain walks up on backend)
    mockListCommands([
      {
        id: "inner.cmd",
        name: "Inner",
        group: "inner",
        context_menu: true,
        available: true,
      },
      {
        id: "outer.cmd",
        name: "Outer",
        group: "outer",
        context_menu: true,
        available: true,
      },
    ]);
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
    await waitFor(() => {
      const ctxCalls = (invoke as ReturnType<typeof vi.fn>).mock.calls.filter(
        (c: unknown[]) => c[0] === "show_context_menu",
      );
      expect(ctxCalls).toHaveLength(1);
      const call = ctxCalls[0];
      // Inner scope should show both inner and outer commands (scope chain walks up on backend)
      const items = call[1].items;
      expect(
        items.find((i: { cmd: string }) => i.cmd === "inner.cmd"),
      ).toBeTruthy();
      expect(
        items.find((i: { cmd: string }) => i.cmd === "outer.cmd"),
      ).toBeTruthy();
    });
  });

  it("nested FocusScope: same command ID without target shadows — inner wins", async () => {
    // Backend handles shadowing: only inner command returned
    mockListCommands([
      {
        id: "entity.inspect",
        name: "Inspect tag",
        group: "entity",
        context_menu: true,
        available: true,
      },
    ]);
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
    await waitFor(() => {
      const ctxCalls = (invoke as ReturnType<typeof vi.fn>).mock.calls.filter(
        (c: unknown[]) => c[0] === "show_context_menu",
      );
      expect(ctxCalls).toHaveLength(1);
      const call = ctxCalls[0];
      const items = call[1].items;
      // No target -> shadow by id alone: inner "Inspect tag" shadows outer "Inspect task"
      expect(items).toHaveLength(1);
      expect(items[0]).toEqual(
        expect.objectContaining({
          cmd: "entity.inspect",
          name: "Inspect tag",
          separator: false,
        }),
      );
    });
  });

  it("nested FocusScope: same command ID with different targets accumulates both", async () => {
    // Backend returns both commands with different targets
    mockListCommands([
      {
        id: "entity.inspect",
        name: "Inspect tag",
        target: "tag:xyz",
        group: "entity",
        context_menu: true,
        available: true,
      },
      {
        id: "entity.inspect",
        name: "Inspect task",
        target: "task:abc",
        group: "entity",
        context_menu: true,
        available: true,
      },
    ]);
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
    await waitFor(() => {
      const ctxCalls = (invoke as ReturnType<typeof vi.fn>).mock.calls.filter(
        (c: unknown[]) => c[0] === "show_context_menu",
      );
      expect(ctxCalls).toHaveLength(1);
      const call = ctxCalls[0];
      const items = call[1].items;
      // Different targets -> both accumulate
      const commandItems = items.filter(
        (i: { separator: boolean }) => !i.separator,
      );
      expect(commandItems).toHaveLength(2);
      expect(
        commandItems.find((i: { name: string }) => i.name === "Inspect tag"),
      ).toBeTruthy();
      expect(
        commandItems.find((i: { name: string }) => i.name === "Inspect task"),
      ).toBeTruthy();
    });
  });

  it("nested FocusScope: same command ID with same target shadows — inner wins", async () => {
    // Backend handles shadowing: only inner command returned
    mockListCommands([
      {
        id: "entity.inspect",
        name: "Inspect task inner",
        target: "task:abc",
        group: "entity",
        context_menu: true,
        available: true,
      },
    ]);
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
    await waitFor(() => {
      const ctxCalls = (invoke as ReturnType<typeof vi.fn>).mock.calls.filter(
        (c: unknown[]) => c[0] === "show_context_menu",
      );
      expect(ctxCalls).toHaveLength(1);
      const call = ctxCalls[0];
      const items = call[1].items;
      // Same target -> shadow: inner wins
      expect(items).toHaveLength(1);
      expect(items[0].name).toBe("Inspect task inner");
    });
  });

  it("nested FocusScope: unavailable inner command blocks same (id, target) from parent", async () => {
    // Backend returns empty list when inner blocks outer
    mockListCommands([]);
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
    // Allow the async list_commands_for_scope call to settle
    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith(
        "list_commands_for_scope",
        expect.anything(),
      );
    });
    // Backend returns empty: no context menu shown.
    expect(invoke).not.toHaveBeenCalledWith(
      "show_context_menu",
      expect.anything(),
    );
  });

  it("double-click executes ui.inspect command", () => {
    const execute = vi.fn();
    const { getByText } = renderWithFocus(
      <FocusScope
        moniker="task:abc"
        commands={[
          { id: "ui.inspect", name: "Inspect", contextMenu: true, execute },
        ]}
      >
        <span>card</span>
      </FocusScope>,
    );
    fireEvent.doubleClick(getByText("card"));
    expect(execute).toHaveBeenCalledTimes(1);
  });

  it("double-click on INPUT does not trigger ui.inspect", () => {
    const execute = vi.fn();
    const { getByRole } = renderWithFocus(
      <FocusScope
        moniker="task:abc"
        commands={[
          { id: "ui.inspect", name: "Inspect", contextMenu: true, execute },
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
            id: "ui.inspect",
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
              id: "ui.inspect",
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
    // Inner scope's ui.inspect fires (resolveCommand finds nearest)
    expect(innerExec).toHaveBeenCalledTimes(1);
    // Outer does NOT fire because stopPropagation prevents the event from reaching it
    expect(outerExec).not.toHaveBeenCalled();
  });

  it("double-click does nothing when no ui.inspect command exists", () => {
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

  it("commands are provided to CommandScopeProvider", async () => {
    mockListCommands([
      {
        id: "entity.inspect",
        name: "Inspect",
        group: "entity",
        context_menu: true,
        available: true,
      },
    ]);
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
    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("show_context_menu", {
        items: [
          expect.objectContaining({
            cmd: "entity.inspect",
            name: "Inspect",
            separator: false,
          }),
        ],
      });
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

  it("showFocusBar=false still fires context menu (handleEvents defaults true)", async () => {
    mockListCommands([
      {
        id: "tag.inspect",
        name: "Inspect tag",
        group: "tag",
        context_menu: true,
        available: true,
      },
    ]);
    const execute = vi.fn();
    const { getByText } = renderWithFocus(
      <FocusScope
        moniker="tag:xyz"
        showFocusBar={false}
        commands={[
          {
            id: "tag.inspect",
            name: "Inspect tag",
            contextMenu: true,
            execute,
          },
        ]}
      >
        <span>tag pill</span>
      </FocusScope>,
    );
    fireEvent.contextMenu(getByText("tag pill"));
    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("show_context_menu", {
        items: [
          expect.objectContaining({
            cmd: "tag.inspect",
            name: "Inspect tag",
          }),
        ],
      });
    });
  });

  it("handleEvents=false suppresses context menu even with showFocusBar=true", async () => {
    mockListCommands([
      {
        id: "tag.inspect",
        name: "Inspect tag",
        group: "tag",
        context_menu: true,
        available: true,
      },
    ]);
    const execute = vi.fn();
    const { getByText } = renderWithFocus(
      <FocusScope
        moniker="tag:xyz"
        showFocusBar={true}
        handleEvents={false}
        commands={[
          {
            id: "tag.inspect",
            name: "Inspect tag",
            contextMenu: true,
            execute,
          },
        ]}
      >
        <span>tag pill</span>
      </FocusScope>,
    );
    fireEvent.contextMenu(getByText("tag pill"));
    // Give time for any async calls to settle
    await new Promise((r) => setTimeout(r, 50));
    expect(invoke).not.toHaveBeenCalledWith(
      "show_context_menu",
      expect.anything(),
    );
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
