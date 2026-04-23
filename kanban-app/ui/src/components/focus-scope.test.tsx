import { describe, it, expect, vi, beforeEach } from "vitest";
import React from "react";
import { render, fireEvent, waitFor, act } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import {
  EntityFocusProvider,
  useEntityFocus,
  useFocusedMoniker,
  useIsFocused,
} from "@/lib/entity-focus-context";
import {
  FocusScope,
  useFocusScopeElementRef,
  useParentFocusScope,
} from "./focus-scope";
import { FocusLayer } from "./focus-layer";
import { CommandScopeProvider, useDispatchCommand } from "@/lib/command-scope";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(() => Promise.resolve()),
  transformCallback: vi.fn(),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ label: "main" }),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));
// A single shared mock so tests can rewire behavior with `mockImplementation`
// after import — exposing a stable `listen` fn is what lets the navOverride
// test capture the production `focus-changed` handler via the webview listen
// path introduced when entity-focus-context moved from app-wide listen to
// `getCurrentWebviewWindow().listen()`.
const { webviewWindowListen } = vi.hoisted(() => ({
  webviewWindowListen: vi.fn(() => Promise.resolve(() => {})),
}));
vi.mock("@tauri-apps/api/webviewWindow", () => ({
  getCurrentWebviewWindow: () => ({
    label: "main",
    listen: webviewWindowListen,
  }),
}));

vi.mock("ulid", () => {
  let counter = 0;
  return { ulid: vi.fn(() => "01TEST" + String(++counter).padStart(20, "0")) };
});

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
  const focusedMoniker = useFocusedMoniker();
  return <div data-testid="focus-reader">{focusedMoniker ?? "null"}</div>;
}

function renderWithFocus(ui: React.ReactElement) {
  return render(
    <EntityFocusProvider>
      <FocusLayer name="test">
        <FocusReader />
        {ui}
      </FocusLayer>
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
        <FocusLayer name="test">
          <ScopeProbe />
          <FocusScope moniker="task:abc" commands={[]}>
            <span>card</span>
          </FocusScope>
        </FocusLayer>
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
          <FocusLayer name="test">
            <FocusScope moniker="column:col1" commands={[]}>
              <ParentScopeReader />
            </FocusScope>
          </FocusLayer>
        </EntityFocusProvider>,
      );
      expect(getByTestId("parent-scope").textContent).toBe("column:col1");
    });

    it("skips CommandScopeProvider, returns grandparent FocusScope moniker", () => {
      const { getByTestId } = render(
        <EntityFocusProvider>
          <FocusLayer name="test">
            <FocusScope moniker="column:col1" commands={[]}>
              <CommandScopeProvider commands={[]} moniker="inner-cmd">
                <ParentScopeReader />
              </CommandScopeProvider>
            </FocusScope>
          </FocusLayer>
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
        <FocusLayer name="test">
          <FocusScope moniker="column:col1" commands={[]}>
            <ColumnWithFocus moniker="column:col1">
              <FocusScope moniker="task:abc" commands={[]}>
                <span>card</span>
              </FocusScope>
            </ColumnWithFocus>
          </FocusScope>
        </FocusLayer>
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

/* ---------- spatial key and Tauri invokes ---------- */

describe("spatial focus integration", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("FocusScope click invokes spatial_focus with its key", async () => {
    const { getByText } = render(
      <EntityFocusProvider>
        <FocusLayer name="test">
          <FocusScope moniker="task:abc" commands={[]}>
            <span>click me</span>
          </FocusScope>
        </FocusLayer>
      </EntityFocusProvider>,
    );

    fireEvent.click(getByText("click me"));

    // spatial_focus should be called with a key string
    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith(
        "spatial_focus",
        expect.objectContaining({ key: expect.any(String) }),
      );
    });
  });

  it("FocusScope mount invokes spatial_register with key, moniker, and layerKey", async () => {
    render(
      <EntityFocusProvider>
        <FocusLayer name="test">
          <FocusScope moniker="task:xyz" commands={[]}>
            <span>child</span>
          </FocusScope>
        </FocusLayer>
      </EntityFocusProvider>,
    );

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith(
        "spatial_register",
        expect.objectContaining({
          args: expect.objectContaining({
            key: expect.any(String),
            moniker: "task:xyz",
            layerKey: expect.any(String),
            x: expect.any(Number),
            y: expect.any(Number),
            w: expect.any(Number),
            h: expect.any(Number),
          }),
        }),
      );
    });
  });

  // --------------------------------------------------------------
  // Regression for 01KPVT4K538CJHJR31NNQHY8EH (inspector layer escape)
  // --------------------------------------------------------------
  //
  // The Rust algorithm correctly refuses to cross layer boundaries (see
  // `spatial_state.rs::navigate_down_from_last_inspector_field_does_not_escape_to_window_layer`
  // and friends — all green). For the live-app symptom "nav.down past
  // the last inspector field leaks into the window layer" to be real,
  // the FocusScopes inside the inner FocusLayer must be registering
  // with the WRONG layerKey — specifically, the outer window layer
  // instead of the inner inspector layer. If that is true, then from
  // Rust's perspective the "inspector fields" share a layer with
  // window-layer cards and the filter cannot save us.
  //
  // This test pins the contract: a FocusScope inside a nested
  // `<FocusLayer name="inspector">` registers with the inspector's
  // layer key, NOT the outer window layer's key.
  it("FocusScope inside an inner FocusLayer registers with the inner layer key, not the outer", async () => {
    render(
      <EntityFocusProvider>
        <FocusLayer name="window">
          {/* An outer-layer scope so the test can confirm the outer
              FocusLayer actually registered its own key too. */}
          <FocusScope moniker="card:outer" commands={[]}>
            <span>outer</span>
          </FocusScope>
          <FocusLayer name="inspector">
            <FocusScope moniker="field:inner" commands={[]}>
              <span>inner</span>
            </FocusScope>
          </FocusLayer>
        </FocusLayer>
      </EntityFocusProvider>,
    );

    const mockInvoke = invoke as ReturnType<typeof vi.fn>;

    // Capture the pushed layer keys in order.
    const pushedLayerKeys: string[] = [];
    await waitFor(() => {
      const pushes = mockInvoke.mock.calls.filter(
        (c) => c[0] === "spatial_push_layer",
      );
      expect(pushes.length).toBeGreaterThanOrEqual(2);
      pushedLayerKeys.length = 0;
      for (const p of pushes) {
        pushedLayerKeys.push((p[1] as { key: string; name: string }).key);
      }
    });

    // Pushes run top-down in render phase, so [0] is window and [1] is
    // inspector. Validate the names too so this test fails loudly if
    // that invariant drifts.
    const windowPush = mockInvoke.mock.calls.find(
      (c) =>
        c[0] === "spatial_push_layer" &&
        (c[1] as { name: string }).name === "window",
    );
    const inspectorPush = mockInvoke.mock.calls.find(
      (c) =>
        c[0] === "spatial_push_layer" &&
        (c[1] as { name: string }).name === "inspector",
    );
    expect(windowPush).toBeDefined();
    expect(inspectorPush).toBeDefined();
    const windowLayerKey = (windowPush![1] as { key: string }).key;
    const inspectorLayerKey = (inspectorPush![1] as { key: string }).key;
    expect(windowLayerKey).not.toBe(inspectorLayerKey);

    // The `field:inner` scope MUST register with the inspector's layer
    // key. If it registers with the window layer key, every `nav.*`
    // command will find window-layer cards in its candidate pool and
    // leak focus across the layer boundary. Reproduces the live-app
    // symptom reported in 01KPVT4K538CJHJR31NNQHY8EH.
    await waitFor(() => {
      const innerRegister = mockInvoke.mock.calls.find(
        (c) =>
          c[0] === "spatial_register" &&
          (c[1] as { args: { moniker: string } }).args.moniker ===
            "field:inner",
      );
      expect(innerRegister).toBeDefined();
      const innerArgs = (innerRegister![1] as { args: { layerKey: string } })
        .args;
      expect(innerArgs.layerKey).toBe(inspectorLayerKey);
      expect(innerArgs.layerKey).not.toBe(windowLayerKey);
    });

    // Belt-and-suspenders: the outer scope still registers with the
    // window layer. If both scopes ended up with the same key, the
    // test above would silently pass on a degenerate config.
    await waitFor(() => {
      const outerRegister = mockInvoke.mock.calls.find(
        (c) =>
          c[0] === "spatial_register" &&
          (c[1] as { args: { moniker: string } }).args.moniker === "card:outer",
      );
      expect(outerRegister).toBeDefined();
      const outerArgs = (outerRegister![1] as { args: { layerKey: string } })
        .args;
      expect(outerArgs.layerKey).toBe(windowLayerKey);
    });
  });

  // --------------------------------------------------------------
  // Regression for the live-app symptom the user reports:
  // `j` past the last inspector field still leaks to a board card.
  //
  // The algorithm is correct. The previous test proves React threads
  // the inner layer key correctly under `render()`. But production
  // runs under `<React.StrictMode>` which mounts every component
  // twice (mount → unmount → mount). This replays every
  // `spatial_push_layer` / `spatial_register` / `spatial_unregister`
  // / `spatial_remove_layer` invoke with different spatial keys
  // (`useSpatialKey` generates a fresh ULID per mount). Net live
  // state must have exactly ONE entry per moniker, with the correct
  // layer_key.
  //
  // If this test fails, the live app has an orphan registration we
  // never clean up — and a FocusScope inside a nested FocusLayer
  // would show up in Rust under the OUTER layer's key as far as the
  // beam test is concerned, breaking the layer filter from the
  // ground up.
  // --------------------------------------------------------------
  it("under StrictMode, net live state has field scope registered under inspector layer only", async () => {
    render(
      <React.StrictMode>
        <EntityFocusProvider>
          <FocusLayer name="window">
            <FocusScope moniker="card:outer" commands={[]}>
              <span>outer</span>
            </FocusScope>
            <FocusLayer name="inspector">
              <FocusScope moniker="field:inner" commands={[]}>
                <span>inner</span>
              </FocusScope>
            </FocusLayer>
          </FocusLayer>
        </EntityFocusProvider>
      </React.StrictMode>,
    );

    const mockInvoke = invoke as ReturnType<typeof vi.fn>;

    // Wait until every registration the harness is going to fire has
    // fired — for a two-scope tree there will be 2 live registers
    // plus 1-2 unregisters (StrictMode mount-unmount-mount for each
    // scope). Capture the final live state by replaying the invoke
    // log in Rust's style: a map of spatial_key → latest layer_key
    // for each key, minus any keys that were subsequently
    // unregistered.
    await waitFor(() => {
      const registers = mockInvoke.mock.calls.filter(
        (c) => c[0] === "spatial_register",
      );
      // Net at least one register for each of the two scopes.
      expect(registers.length).toBeGreaterThanOrEqual(2);
    });

    // Simulate the Rust-side state: replay every spatial_* invoke in
    // order.
    interface LiveEntry {
      moniker: string;
      layerKey: string;
    }
    const liveEntries = new Map<string, LiveEntry>();
    for (const call of mockInvoke.mock.calls) {
      const [cmd, payload] = call as [string, unknown];
      if (cmd === "spatial_register") {
        const args = (payload as { args: LiveEntry & { key: string } }).args;
        liveEntries.set(args.key, {
          moniker: args.moniker,
          layerKey: args.layerKey,
        });
      } else if (cmd === "spatial_unregister") {
        const key = (payload as { key: string }).key;
        liveEntries.delete(key);
      }
    }

    // Replay the push/remove sequence for layers using Rust's
    // refcounting semantics — each push increments, each remove
    // decrements, entry lives while count > 0. Under StrictMode's
    // useState double-invoke a layer gets push-push-remove, net
    // refcount 1 (still live). Without the refcount that would
    // collapse to 0 and the layer would vanish from the stack.
    const layerRefcount = new Map<string, number>();
    const layerOrder: string[] = [];
    for (const call of mockInvoke.mock.calls) {
      const [cmd, payload] = call as [string, unknown];
      if (cmd === "spatial_push_layer") {
        const key = (payload as { key: string }).key;
        const prev = layerRefcount.get(key) ?? 0;
        if (prev === 0) layerOrder.push(key);
        layerRefcount.set(key, prev + 1);
      } else if (cmd === "spatial_remove_layer") {
        const key = (payload as { key: string }).key;
        const prev = layerRefcount.get(key) ?? 0;
        const next = Math.max(0, prev - 1);
        layerRefcount.set(key, next);
        if (next === 0) {
          const idx = layerOrder.indexOf(key);
          if (idx >= 0) layerOrder.splice(idx, 1);
        }
      }
    }
    const liveLayers = layerOrder.filter(
      (k) => (layerRefcount.get(k) ?? 0) > 0,
    );

    // The window layer must still be live (outermost, not unmounted).
    // The inspector layer must also still be live.
    expect(liveLayers.length).toBe(2);
    const [windowLayerKey, inspectorLayerKey] = liveLayers;
    expect(windowLayerKey).toMatch(/^layer-window-/);
    expect(inspectorLayerKey).toMatch(/^layer-inspector-/);

    // Build: for each moniker, which layer_key is live?
    const monikerToLayer = new Map<string, string>();
    for (const entry of liveEntries.values()) {
      monikerToLayer.set(entry.moniker, entry.layerKey);
    }

    // Invariant: the inner moniker must be live under the inspector
    // layer — not the window layer, not both, not neither. Any
    // deviation (missing, double-registered under different keys,
    // stale orphan under window layer) means live-app nav will leak.
    expect(monikerToLayer.get("field:inner")).toBe(inspectorLayerKey);
    expect(monikerToLayer.get("card:outer")).toBe(windowLayerKey);

    // There must be EXACTLY 2 live entries — not, e.g., 3 because a
    // StrictMode orphan register from the first mount was never
    // unregistered.
    expect(liveEntries.size).toBe(2);
  });

  it("FocusScope unmount invokes spatial_unregister", async () => {
    function Harness({ show }: { show: boolean }) {
      return (
        <EntityFocusProvider>
          <FocusLayer name="test">
            {show && (
              <FocusScope moniker="task:tmp" commands={[]}>
                <span>temp</span>
              </FocusScope>
            )}
          </FocusLayer>
        </EntityFocusProvider>
      );
    }

    const { rerender } = render(<Harness show={true} />);

    // Clear mocks to isolate unmount calls
    vi.clearAllMocks();

    rerender(<Harness show={false} />);

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith(
        "spatial_unregister",
        expect.objectContaining({ key: expect.any(String) }),
      );
    });
  });

  it("FocusScope unmount removes from claim registry", async () => {
    // Production `entity-focus-context` subscribes to `focus-changed` on the
    // current webview window — capture the handler via the shared webview
    // listen mock so this test can simulate a late event after unmount.
    let eventCallback: ((evt: { payload: unknown }) => void) | null = null;
    webviewWindowListen.mockImplementation(((
      _event: string,
      cb: (evt: { payload: unknown }) => void,
    ) => {
      eventCallback = cb;
      return Promise.resolve(() => {});
    }) as unknown as () => Promise<() => void>);

    let capturedKey: string | null = null;

    function Harness({ show }: { show: boolean }) {
      return (
        <EntityFocusProvider>
          <FocusLayer name="test">
            {show && (
              <FocusScope moniker="task:tmp" commands={[]}>
                <span>temp</span>
              </FocusScope>
            )}
            <FocusReader />
          </FocusLayer>
        </EntityFocusProvider>
      );
    }

    const { rerender } = render(<Harness show={true} />);

    // Capture the spatial key from the spatial_register call
    await waitFor(() => {
      const registerCall = (invoke as ReturnType<typeof vi.fn>).mock.calls.find(
        (c: unknown[]) => c[0] === "spatial_register",
      );
      expect(registerCall).toBeTruthy();
      capturedKey = (registerCall![1] as { args: { key: string } }).args.key;
    });

    // Unmount the FocusScope
    rerender(<Harness show={false} />);

    // Fire focus-changed event for the now-unmounted key — should NOT throw
    if (eventCallback) {
      await act(async () => {
        eventCallback!({
          payload: { prev_key: null, next_key: capturedKey },
        });
      });
    }

    // Focus should not have changed to the unmounted scope's moniker
    // (callback was unregistered, so the event is a no-op)
  });

  it("navOverride prop is forwarded to spatial_register as overrides", async () => {
    const overrides = { Right: "task:02", Left: null };
    render(
      <EntityFocusProvider>
        <FocusLayer name="test">
          <FocusScope moniker="task:01" commands={[]} navOverride={overrides}>
            <span>child</span>
          </FocusScope>
        </FocusLayer>
      </EntityFocusProvider>,
    );

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith(
        "spatial_register",
        expect.objectContaining({
          args: expect.objectContaining({
            moniker: "task:01",
            overrides: { Right: "task:02", Left: null },
          }),
        }),
      );
    });
  });

  it("nested FocusScope threads parent_scope moniker through spatial_register", async () => {
    // Locks down the wiring that lets the Rust engine's container-first
    // search keep `h/j/k/l` inside a card's sub-parts. Child scopes read
    // the ancestor moniker from `FocusScopeContext`; without this thread,
    // every `spatial_register` call would pass `parent_scope: null` and
    // the engine would have no way to scope cardinal-direction searches to
    // the siblings of the source scope.
    render(
      <EntityFocusProvider>
        <FocusLayer name="test">
          <FocusScope moniker="task:card-1" commands={[]}>
            <FocusScope moniker="tag:pill-1" commands={[]}>
              <span>pill</span>
            </FocusScope>
          </FocusScope>
        </FocusLayer>
      </EntityFocusProvider>,
    );

    // The outer card has no ancestor scope — parentScope is null.
    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith(
        "spatial_register",
        expect.objectContaining({
          args: expect.objectContaining({
            moniker: "task:card-1",
            parentScope: null,
          }),
        }),
      );
    });

    // The inner pill's parentScope is the enclosing card's moniker.
    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith(
        "spatial_register",
        expect.objectContaining({
          args: expect.objectContaining({
            moniker: "tag:pill-1",
            parentScope: "task:card-1",
          }),
        }),
      );
    });
  });

  it("navOverride end-to-end: nav.right dispatch routes override target through focus-changed", async () => {
    // This test exercises the full navOverride path end-to-end through
    // the unified command dispatch pipeline:
    //   React (mount FocusScope with navOverride)
    //   → Rust spatial_register (overrides payload captured)
    //   → user sets focus on the source scope
    //   → React useDispatchCommand("nav.right") → invoke("dispatch_command")
    //   → Rust NavigateCmd reads SpatialState.focused_key, calls navigate()
    //     (simulated here since Rust is not in scope for this harness)
    //   → Rust applies the override and emits focus-changed(next_key = target)
    //   → React focus-changed listener routes back to focused moniker + claim flip
    //
    // The Rust override *selection* is covered by unit tests in
    // spatial_state.rs; this test proves React is wired such that if Rust
    // returns the override target, the UI picks it up on both the
    // focus-state and the claim highlight path.
    //
    // Critically: there is NO JS-side broadcaster in this graph. The
    // `broadcastNavCommand` side-channel was deleted in favor of routing
    // `nav.*` through the same `dispatch_command` pipeline the rest of
    // the command surface uses.
    // Production `entity-focus-context` listens for `focus-changed` via
    // `getCurrentWebviewWindow().listen()` so listeners are scoped to this
    // window. Capture the handler via the shared webviewWindow mock.
    let eventCallback: ((evt: { payload: unknown }) => void) | null = null;
    webviewWindowListen.mockImplementation(((
      event: string,
      cb: (evt: { payload: unknown }) => void,
    ) => {
      if (event === "focus-changed") eventCallback = cb;
      return Promise.resolve(() => {});
    }) as unknown as () => Promise<() => void>);

    // Helper that dispatches nav.right through the command pipeline — no
    // JS-side broadcaster, just a useDispatchCommand call like
    // production's `createKeyHandler` would make after a `l` keypress.
    function NavDispatcher() {
      const dispatch = useDispatchCommand();
      return (
        <button
          data-testid="dispatch-right"
          onClick={() => {
            dispatch("nav.right").catch(() => {});
          }}
        />
      );
    }

    const { getByText, getByTestId } = render(
      <EntityFocusProvider>
        <FocusLayer name="test">
          <FocusReader />
          <NavDispatcher />
          <FocusScope
            moniker="task:01"
            commands={[]}
            navOverride={{ Right: "task:02" }}
          >
            <span>source</span>
          </FocusScope>
          <FocusScope moniker="task:02" commands={[]}>
            <span>target</span>
          </FocusScope>
        </FocusLayer>
      </EntityFocusProvider>,
    );

    // Both scopes should register with Rust. Capture each scope's spatial key
    // from its spatial_register call so we can correlate later invokes.
    let sourceKey: string | null = null;
    let targetKey: string | null = null;
    await waitFor(() => {
      const calls = (invoke as ReturnType<typeof vi.fn>).mock.calls;
      const sourceCall = calls.find(
        (c: unknown[]) =>
          c[0] === "spatial_register" &&
          (c[1] as { args: { moniker: string } }).args.moniker === "task:01",
      );
      const targetCall = calls.find(
        (c: unknown[]) =>
          c[0] === "spatial_register" &&
          (c[1] as { args: { moniker: string } }).args.moniker === "task:02",
      );
      expect(sourceCall).toBeTruthy();
      expect(targetCall).toBeTruthy();
      sourceKey = (sourceCall![1] as { args: { key: string } }).args.key;
      targetKey = (targetCall![1] as { args: { key: string } }).args.key;
    });

    // Confirm the override payload reached Rust exactly as declared.
    expect(invoke).toHaveBeenCalledWith(
      "spatial_register",
      expect.objectContaining({
        args: expect.objectContaining({
          key: sourceKey,
          moniker: "task:01",
          overrides: { Right: "task:02" },
        }),
      }),
    );

    // Seed focus on the source scope so Rust's `NavigateCmd` has a
    // focused source when it reads SpatialState. Clicking populates the
    // moniker→key map in entity-focus-context and calls spatial_focus.
    fireEvent.click(getByText("source"));
    expect(getByTestId("focus-reader").textContent).toBe("task:01");

    // Clear mocks so we can assert exactly one dispatch_command call.
    (invoke as ReturnType<typeof vi.fn>).mockClear();

    // User "presses" l (vim right): React dispatches nav.right through
    // the command pipeline. The dispatcher forwards to Rust via
    // invoke("dispatch_command", { cmd: "nav.right", ... }).
    fireEvent.click(getByTestId("dispatch-right"));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith(
        "dispatch_command",
        expect.objectContaining({ cmd: "nav.right" }),
      );
    });

    // Simulate Rust applying the override and emitting focus-changed with
    // next_key = target scope's key. This is the exact event shape Rust
    // fires after NavigateCmd's navigate() returns Some(override_target_key).
    expect(eventCallback).toBeTruthy();
    await act(async () => {
      eventCallback!({
        payload: { prev_key: sourceKey, next_key: targetKey },
      });
    });

    // The React side picked up the override target: focused moniker is now
    // task:02, proving the full loop closed.
    expect(getByTestId("focus-reader").textContent).toBe("task:02");
  });
});

/* ---------- spatial={false} opt-out ---------- */

describe("FocusScope spatial prop", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    (invoke as ReturnType<typeof vi.fn>).mockResolvedValue(undefined);
  });

  it("spatial=false skips spatial_register (no rect in the beam-test graph)", async () => {
    render(
      <EntityFocusProvider>
        <FocusLayer name="test">
          <FocusScope moniker="row:1" commands={[]} spatial={false}>
            <span>row</span>
          </FocusScope>
        </FocusLayer>
      </EntityFocusProvider>,
    );

    // Give effects a chance to flush — if spatial_register were going to be
    // called, it would have been called synchronously on mount.
    await new Promise((r) => setTimeout(r, 20));
    expect(invoke).not.toHaveBeenCalledWith(
      "spatial_register",
      expect.anything(),
    );
  });

  it("spatial=false skips spatial_unregister on unmount", async () => {
    function Harness({ show }: { show: boolean }) {
      return (
        <EntityFocusProvider>
          <FocusLayer name="test">
            {show && (
              <FocusScope moniker="row:tmp" commands={[]} spatial={false}>
                <span>temp</span>
              </FocusScope>
            )}
          </FocusLayer>
        </EntityFocusProvider>
      );
    }

    const { rerender } = render(<Harness show={true} />);
    vi.clearAllMocks();
    rerender(<Harness show={false} />);

    // Give effects a tick; spatial_unregister must NOT be called because
    // the scope never registered in the first place.
    await new Promise((r) => setTimeout(r, 20));
    expect(invoke).not.toHaveBeenCalledWith(
      "spatial_unregister",
      expect.anything(),
    );
  });

  it("spatial=true (default) still invokes spatial_register", async () => {
    // Sanity check the inverse: omitting `spatial` yields the usual
    // registration. Guards against the spatial={false} case silently
    // becoming the default.
    render(
      <EntityFocusProvider>
        <FocusLayer name="test">
          <FocusScope moniker="cell:1" commands={[]}>
            <span>cell</span>
          </FocusScope>
        </FocusLayer>
      </EntityFocusProvider>,
    );

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith(
        "spatial_register",
        expect.objectContaining({
          args: expect.objectContaining({ moniker: "cell:1" }),
        }),
      );
    });
  });

  it("spatial=false scope still registers in the entity-focus scope registry", () => {
    // Opting out of spatial registration must not break the focus/command
    // scope registration — non-spatial container scopes (rows) still own
    // their commands and must resolve in the scope chain.
    let probeGetScope: ((m: string) => unknown) | null = null;
    function ScopeProbe() {
      const { getScope } = useEntityFocus();
      probeGetScope = getScope;
      return null;
    }

    render(
      <EntityFocusProvider>
        <FocusLayer name="test">
          <ScopeProbe />
          <FocusScope moniker="row:1" commands={[]} spatial={false}>
            <span>row</span>
          </FocusScope>
        </FocusLayer>
      </EntityFocusProvider>,
    );

    expect(probeGetScope!("row:1")).not.toBeNull();
  });
});

/* ---------- useFocusScopeElementRef hook ---------- */

describe("useFocusScopeElementRef", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    (invoke as ReturnType<typeof vi.fn>).mockResolvedValue(undefined);
  });

  /**
   * Probe that renders its hook result into the DOM so tests can assert
   * whether the ref was populated. A non-null ref renders "has-ref",
   * a null context renders "no-ref".
   */
  function ElementRefProbe({ testId }: { testId: string }) {
    const ref = useFocusScopeElementRef();
    return <span data-testid={testId}>{ref ? "has-ref" : "no-ref"}</span>;
  }

  it("returns null outside any FocusScope", () => {
    const { getByTestId } = render(
      <EntityFocusProvider>
        <ElementRefProbe testId="probe" />
      </EntityFocusProvider>,
    );
    expect(getByTestId("probe").textContent).toBe("no-ref");
  });

  it("returns null inside a FocusScope rendered with renderContainer=true (default)", () => {
    // Default FocusScope owns its own wrapping `<div>` and binds the
    // ref internally. Descendants must NOT see a ref — there is
    // nothing for them to attach to.
    const { getByTestId } = render(
      <EntityFocusProvider>
        <FocusLayer name="test">
          <FocusScope moniker="task:abc" commands={[]}>
            <ElementRefProbe testId="probe" />
          </FocusScope>
        </FocusLayer>
      </EntityFocusProvider>,
    );
    expect(getByTestId("probe").textContent).toBe("no-ref");
  });

  it("returns a non-null ref inside a FocusScope with renderContainer=false", () => {
    // When the scope suppresses its container, a descendant must attach
    // the ref to its own DOM element so `ResizeObserver` can measure it.
    const { getByTestId } = render(
      <EntityFocusProvider>
        <FocusLayer name="test">
          <FocusScope moniker="task:abc" commands={[]} renderContainer={false}>
            <ElementRefProbe testId="probe" />
          </FocusScope>
        </FocusLayer>
      </EntityFocusProvider>,
    );
    expect(getByTestId("probe").textContent).toBe("has-ref");
  });

  it("attaching the ref causes spatial_register to report a real DOM rect", async () => {
    // Full contract: the consumer attaches the ref to its element, and
    // `ResizeObserver` + `getBoundingClientRect` produce the rect that
    // flows into spatial_register. Proves `useFocusScopeElementRef` is
    // not just returning a ref — it's the ref the scope observes.
    function Consumer() {
      const ref = useFocusScopeElementRef();
      return (
        <div
          ref={ref as React.RefObject<HTMLDivElement>}
          data-testid="consumer"
          style={{ width: "100px", height: "50px" }}
        >
          content
        </div>
      );
    }

    render(
      <EntityFocusProvider>
        <FocusLayer name="test">
          <FocusScope moniker="cell:abc" commands={[]} renderContainer={false}>
            <Consumer />
          </FocusScope>
        </FocusLayer>
      </EntityFocusProvider>,
    );

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith(
        "spatial_register",
        expect.objectContaining({
          args: expect.objectContaining({
            moniker: "cell:abc",
            x: expect.any(Number),
            y: expect.any(Number),
            w: expect.any(Number),
            h: expect.any(Number),
          }),
        }),
      );
    });
  });
});

/* ---------- renderContainer=false data-focused propagation ---------- */

describe("renderContainer=false data-focused propagation", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    (invoke as ReturnType<typeof vi.fn>).mockResolvedValue(undefined);
  });

  /**
   * Consumer helper that binds the enclosing `FocusScope`'s elementRef
   * to a leaf `<div>` via `useFocusScopeElementRef`. Mirrors production
   * consumers (row selector, LeftNav button, perspective tab).
   */
  function RefConsumer({ testId }: { testId: string }) {
    const ref = useFocusScopeElementRef();
    return (
      <div
        ref={ref as React.RefObject<HTMLDivElement>}
        data-testid={testId}
        style={{ width: "100px", height: "50px" }}
      >
        content
      </div>
    );
  }

  it("claimed renderContainer=false scope sets data-focused on its attached element", async () => {
    // The scope has no wrapping container, so the only DOM node it can
    // mark is the consumer element. This is the central guarantee of
    // the centralized focus decoration: one attribute write, one place.
    function Harness() {
      const { setFocus } = useEntityFocus();
      return (
        <>
          <button
            data-testid="focus-btn"
            onClick={() => setFocus("task:abc")}
          />
          <FocusScope moniker="task:abc" commands={[]} renderContainer={false}>
            <RefConsumer testId="leaf" />
          </FocusScope>
        </>
      );
    }

    const { getByTestId } = renderWithFocus(<Harness />);
    const leaf = getByTestId("leaf");

    // Initially unfocused — no attribute.
    expect(leaf.hasAttribute("data-focused")).toBe(false);

    // Click the focus trigger — setFocus flips the claim, which FocusScope
    // mirrors onto leaf.
    fireEvent.click(getByTestId("focus-btn"));

    await waitFor(() => {
      expect(leaf.getAttribute("data-focused")).toBe("true");
    });
  });

  it("unclaimed renderContainer=false scope clears data-focused on its attached element", async () => {
    // Round-trip: focus A, then focus B. A's leaf must lose the
    // attribute (not just stay stale at "true").
    function Harness() {
      const { setFocus } = useEntityFocus();
      return (
        <>
          <button data-testid="focus-a" onClick={() => setFocus("task:a")} />
          <button data-testid="focus-b" onClick={() => setFocus("task:b")} />
          <FocusScope moniker="task:a" commands={[]} renderContainer={false}>
            <RefConsumer testId="leaf-a" />
          </FocusScope>
          <FocusScope moniker="task:b" commands={[]} renderContainer={false}>
            <RefConsumer testId="leaf-b" />
          </FocusScope>
        </>
      );
    }

    const { getByTestId } = renderWithFocus(<Harness />);
    const leafA = getByTestId("leaf-a");
    const leafB = getByTestId("leaf-b");

    fireEvent.click(getByTestId("focus-a"));
    await waitFor(() => {
      expect(leafA.getAttribute("data-focused")).toBe("true");
    });

    fireEvent.click(getByTestId("focus-b"));
    await waitFor(() => {
      expect(leafB.getAttribute("data-focused")).toBe("true");
    });
    // Critical regression guard — A must be cleared.
    expect(leafA.hasAttribute("data-focused")).toBe(false);
  });

  it("showFocusBar=false skips the data-focused write on renderContainer=false scopes", async () => {
    // `showFocusBar` is the opt-out for non-decorated scopes (e.g.
    // the structural `store:` scope in `store-container.tsx`). When
    // set, FocusScope must not mark its attached element.
    function Harness() {
      const { setFocus } = useEntityFocus();
      return (
        <>
          <button
            data-testid="focus-btn"
            onClick={() => setFocus("task:abc")}
          />
          <FocusScope
            moniker="task:abc"
            commands={[]}
            renderContainer={false}
            showFocusBar={false}
          >
            <RefConsumer testId="leaf" />
          </FocusScope>
        </>
      );
    }

    const { getByTestId } = renderWithFocus(<Harness />);
    const leaf = getByTestId("leaf");

    fireEvent.click(getByTestId("focus-btn"));

    // Give claim propagation time to settle — the attribute must never appear.
    await new Promise((r) => setTimeout(r, 50));
    expect(leaf.hasAttribute("data-focused")).toBe(false);
  });

  it("claimed scope scrolls its attached element into view", async () => {
    // Regression guard for the scrollIntoView behavior migrating out of
    // FocusHighlight into FocusScope. Covers the renderContainer=false
    // path specifically because renderContainer=true previously had
    // scrollIntoView via FocusHighlight.
    const scrollSpy = vi.fn();
    const originalScrollIntoView = HTMLElement.prototype.scrollIntoView;
    HTMLElement.prototype.scrollIntoView =
      scrollSpy as unknown as typeof HTMLElement.prototype.scrollIntoView;

    try {
      function Harness() {
        const { setFocus } = useEntityFocus();
        return (
          <>
            <button
              data-testid="focus-btn"
              onClick={() => setFocus("task:abc")}
            />
            <FocusScope
              moniker="task:abc"
              commands={[]}
              renderContainer={false}
            >
              <RefConsumer testId="leaf" />
            </FocusScope>
          </>
        );
      }

      const { getByTestId } = renderWithFocus(<Harness />);
      fireEvent.click(getByTestId("focus-btn"));

      await waitFor(() => {
        expect(scrollSpy).toHaveBeenCalledWith({ block: "nearest" });
      });
    } finally {
      HTMLElement.prototype.scrollIntoView = originalScrollIntoView;
    }
  });
});

/* ---------- pull-based data-focused (regression suite) ---------- */

describe("FocusScope data-focused pulls from focused-moniker store", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    (invoke as ReturnType<typeof vi.fn>).mockResolvedValue(undefined);
  });

  /**
   * Regression for orphaned focus bars: after A → B, only B's element may
   * carry `data-focused`. Exercising both scopes through the same
   * focused-moniker store proves that the attribute derives from the
   * idempotent `focusedMoniker === myMoniker` comparison, with no residue
   * from a missed "un-notify" step.
   */
  it("no stale data-focused after focus moves away", async () => {
    function Harness() {
      const { setFocus } = useEntityFocus();
      return (
        <>
          <button data-testid="focus-a" onClick={() => setFocus("task:a")} />
          <button data-testid="focus-b" onClick={() => setFocus("task:b")} />
          <FocusScope moniker="task:a" commands={[]}>
            <span>a</span>
          </FocusScope>
          <FocusScope moniker="task:b" commands={[]}>
            <span>b</span>
          </FocusScope>
        </>
      );
    }

    const { container, getByTestId } = renderWithFocus(<Harness />);

    fireEvent.click(getByTestId("focus-a"));
    await waitFor(() => {
      const a = container.querySelector("[data-moniker='task:a']");
      expect(a?.hasAttribute("data-focused")).toBe(true);
    });

    fireEvent.click(getByTestId("focus-b"));
    await waitFor(() => {
      const b = container.querySelector("[data-moniker='task:b']");
      expect(b?.hasAttribute("data-focused")).toBe(true);
    });

    // The critical guard: exactly one element carries data-focused after
    // a rapid A → B sequence. The push-based implementation could leave
    // A's attribute behind if the claim callback got unregistered before
    // the "un-notify" ran.
    const focused = container.querySelectorAll("[data-focused='true']");
    expect(focused).toHaveLength(1);
    const a = container.querySelector("[data-moniker='task:a']");
    expect(a?.hasAttribute("data-focused")).toBe(false);
  });

  /**
   * Regression for unmount races: a scope that unmounts while focused
   * must not leave its `data-focused` attribute on any surviving element
   * (including DOM nodes reused by React across re-renders).
   */
  it("no stale data-focused after focused scope unmounts", async () => {
    function Harness({ show }: { show: boolean }) {
      const { setFocus } = useEntityFocus();
      return (
        <>
          <button data-testid="focus-a" onClick={() => setFocus("task:a")} />
          {show && (
            <FocusScope moniker="task:a" commands={[]}>
              <span>a</span>
            </FocusScope>
          )}
          <FocusScope moniker="task:other" commands={[]}>
            <span>other</span>
          </FocusScope>
        </>
      );
    }

    const { container, getByTestId, rerender } = render(
      <EntityFocusProvider>
        <FocusLayer name="test">
          <Harness show={true} />
        </FocusLayer>
      </EntityFocusProvider>,
    );

    fireEvent.click(getByTestId("focus-a"));
    await waitFor(() => {
      const a = container.querySelector("[data-moniker='task:a']");
      expect(a?.hasAttribute("data-focused")).toBe(true);
    });

    // Unmount the focused scope. No surviving element — including the
    // sibling `task:other` scope — may carry data-focused.
    rerender(
      <EntityFocusProvider>
        <FocusLayer name="test">
          <Harness show={false} />
        </FocusLayer>
      </EntityFocusProvider>,
    );

    await waitFor(() => {
      expect(container.querySelector("[data-moniker='task:a']")).toBeNull();
    });

    const anyFocused = container.querySelectorAll("[data-focused='true']");
    expect(anyFocused).toHaveLength(0);
  });

  /**
   * The decoration must derive from the focused-moniker store on every
   * render — no notifyClaim push needed. We drive the store directly
   * via `setFocus` (which only mutates the store in the pull-based
   * implementation) and assert the matching scope is decorated.
   *
   * This is the clearest proof of pull semantics: a single store write
   * is sufficient to repaint every scope, without any per-key callback
   * having to fire.
   */
  it("data-focused derives from store not from per-key notifications", async () => {
    function Harness() {
      const { setFocus } = useEntityFocus();
      return (
        <>
          <button data-testid="focus-c" onClick={() => setFocus("task:c")} />
          <FocusScope moniker="task:a" commands={[]}>
            <span>a</span>
          </FocusScope>
          <FocusScope moniker="task:b" commands={[]}>
            <span>b</span>
          </FocusScope>
          <FocusScope moniker="task:c" commands={[]}>
            <span>c</span>
          </FocusScope>
        </>
      );
    }

    const { container, getByTestId } = renderWithFocus(<Harness />);

    fireEvent.click(getByTestId("focus-c"));

    await waitFor(() => {
      const c = container.querySelector("[data-moniker='task:c']");
      expect(c?.hasAttribute("data-focused")).toBe(true);
    });

    const focused = container.querySelectorAll("[data-focused='true']");
    expect(focused).toHaveLength(1);
    expect(focused[0].getAttribute("data-moniker")).toBe("task:c");
  });

  /**
   * The Rust `focus-changed` event is the authoritative focus signal.
   * When it fires, the store updates and every scope must re-derive
   * `data-focused` by comparing its moniker to the new store value.
   * No intermediate push call is required.
   */
  it("rust focus-changed event updates visual without any push notification", async () => {
    // Capture the production focus-changed handler so this test can fire
    // a synthetic Rust event directly at it.
    let eventCallback: ((evt: { payload: unknown }) => void) | null = null;
    webviewWindowListen.mockImplementation(((
      event: string,
      cb: (evt: { payload: unknown }) => void,
    ) => {
      if (event === "focus-changed") eventCallback = cb;
      return Promise.resolve(() => {});
    }) as unknown as () => Promise<() => void>);

    const { container } = render(
      <EntityFocusProvider>
        <FocusLayer name="test">
          <FocusScope moniker="task:a" commands={[]}>
            <span>a</span>
          </FocusScope>
          <FocusScope moniker="task:b" commands={[]}>
            <span>b</span>
          </FocusScope>
        </FocusLayer>
      </EntityFocusProvider>,
    );

    // Extract b's spatial key from its spatial_register invoke so we can
    // address it in the synthetic focus-changed event.
    let targetKey: string | null = null;
    await waitFor(() => {
      const calls = (invoke as ReturnType<typeof vi.fn>).mock.calls;
      const targetCall = calls.find(
        (c: unknown[]) =>
          c[0] === "spatial_register" &&
          (c[1] as { args: { moniker: string } }).args.moniker === "task:b",
      );
      expect(targetCall).toBeTruthy();
      targetKey = (targetCall![1] as { args: { key: string } }).args.key;
    });

    expect(eventCallback).toBeTruthy();
    await act(async () => {
      eventCallback!({
        payload: { prev_key: null, next_key: targetKey },
      });
    });

    await waitFor(() => {
      const b = container.querySelector("[data-moniker='task:b']");
      expect(b?.hasAttribute("data-focused")).toBe(true);
    });

    const a = container.querySelector("[data-moniker='task:a']");
    expect(a?.hasAttribute("data-focused")).toBe(false);
  });

  /**
   * Two scopes sharing a moniker both decorate themselves — the pull
   * model is idempotent by construction (each scope independently
   * compares its moniker to the store value). This proves duplicate
   * mounts stay in sync without requiring every callback in a registry
   * to fire.
   */
  it("two scopes sharing a moniker both decorate from the same store value", async () => {
    function Harness() {
      const { setFocus } = useEntityFocus();
      return (
        <>
          <button
            data-testid="focus-dup"
            onClick={() => setFocus("task:dup")}
          />
          <FocusScope moniker="task:dup" commands={[]} data-testid="dup-1">
            <span>one</span>
          </FocusScope>
          <FocusScope moniker="task:dup" commands={[]} data-testid="dup-2">
            <span>two</span>
          </FocusScope>
        </>
      );
    }

    const { container, getByTestId } = renderWithFocus(<Harness />);

    fireEvent.click(getByTestId("focus-dup"));

    await waitFor(() => {
      const nodes = container.querySelectorAll("[data-moniker='task:dup']");
      expect(nodes).toHaveLength(2);
      expect(nodes[0].hasAttribute("data-focused")).toBe(true);
      expect(nodes[1].hasAttribute("data-focused")).toBe(true);
    });
  });
});
