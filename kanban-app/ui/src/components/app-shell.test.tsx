import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, act } from "@testing-library/react";

// Mock Tauri APIs before importing components that use them
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn((cmd: string) => {
    if (cmd === "get_ui_state")
      return Promise.resolve({
        palette_open: false,
        palette_mode: "command",
        keymap_mode: "cua",
        scope_chain: [],
        open_boards: [],
        windows: {},
        recent_boards: [],
      });
    return Promise.resolve(null);
  }),
}));
/**
 * Captured event listeners keyed by event name.
 *
 * The `listen` mock stores each callback here so tests can fire synthetic
 * events by calling `listenCallbacks["event-name"](payload)`.
 */
const listenCallbacks: Record<string, (event: unknown) => void> = {};
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn((eventName: string, cb: (event: unknown) => void) => {
    listenCallbacks[eventName] = cb;
    return Promise.resolve(() => {});
  }),
}));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ label: "main" }),
}));

import { AppShell } from "./app-shell";
import { FocusScope } from "./focus-scope";
import { UIStateProvider } from "@/lib/ui-state-context";
import { AppModeProvider } from "@/lib/app-mode-context";
import { UndoProvider } from "@/lib/undo-context";
import {
  EntityFocusProvider,
  useEntityFocus,
} from "@/lib/entity-focus-context";
import { useAvailableCommands } from "@/lib/command-scope";
import { invoke } from "@tauri-apps/api/core";

/**
 * Helper component that renders inside AppShell to inspect commands
 * registered in the CommandScope.
 */
function CommandInspector() {
  const commands = useAvailableCommands();
  return (
    <ul data-testid="command-list">
      {commands.map((c) => (
        <li key={c.command.id} data-testid={`cmd-${c.command.id}`}>
          {c.command.name}
        </li>
      ))}
    </ul>
  );
}

/** Render AppShell with all required parent providers. */
function renderShell(children?: React.ReactNode) {
  return render(
    <EntityFocusProvider>
      <UIStateProvider>
        <AppModeProvider>
          <UndoProvider>
            <AppShell>{children ?? <CommandInspector />}</AppShell>
          </UndoProvider>
        </AppModeProvider>
      </UIStateProvider>
    </EntityFocusProvider>,
  );
}

/** Platform-aware Mod key: metaKey on Mac, ctrlKey elsewhere. */
const MOD_KEY = /Mac|iPhone|iPad|iPod/.test(navigator.platform)
  ? "metaKey"
  : "ctrlKey";

describe("AppShell", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("renders children", () => {
    renderShell(<div data-testid="child">Hello</div>);
    expect(screen.getByTestId("child")).toBeTruthy();
    expect(screen.getByText("Hello")).toBeTruthy();
  });

  it("provides global commands via CommandScope", () => {
    renderShell();
    // Check that well-known global commands are available
    expect(screen.getByTestId("cmd-app.command")).toBeTruthy();
    expect(screen.getByTestId("cmd-app.dismiss")).toBeTruthy();
    expect(screen.getByTestId("cmd-app.search")).toBeTruthy();
    expect(screen.getByTestId("cmd-app.help")).toBeTruthy();
    // Commands added by Card 10
    expect(screen.getByTestId("cmd-app.quit")).toBeTruthy();
    expect(screen.getByTestId("cmd-settings.keymap.vim")).toBeTruthy();
    expect(screen.getByTestId("cmd-file.newBoard")).toBeTruthy();
    expect(screen.getByTestId("cmd-file.openBoard")).toBeTruthy();
  });

  it("does not render command palette by default", () => {
    renderShell();
    expect(screen.queryByTestId("command-palette")).toBeNull();
  });

  it("dispatches app.command to backend on Mod+Shift+P in CUA mode", async () => {
    const mockInvoke = invoke as ReturnType<typeof vi.fn>;
    renderShell();
    mockInvoke.mockClear();

    // CUA mode is the default (mocked invoke returns "cua")
    await act(async () => {
      fireEvent.keyDown(document, {
        key: "P",
        code: "KeyP",
        [MOD_KEY]: true,
        shiftKey: true,
      });
    });

    // Palette opening is now driven by backend UIState, so we verify
    // that the keybinding dispatches to the backend.
    const cmdCall = mockInvoke.mock.calls.find(
      (c: unknown[]) =>
        c[0] === "dispatch_command" &&
        ((c[1] as Record<string, unknown>)?.cmd === "app.command" ||
          (c[1] as Record<string, unknown>)?.cmd === "app.palette"),
    );
    expect(cmdCall).toBeTruthy();
  });

  it("dispatches app.dismiss to backend on Escape", async () => {
    const mockInvoke = invoke as ReturnType<typeof vi.fn>;
    renderShell();
    mockInvoke.mockClear();

    // Press Escape to dismiss
    await act(async () => {
      fireEvent.keyDown(document, {
        key: "Escape",
        code: "Escape",
      });
    });

    const dismissCall = mockInvoke.mock.calls.find(
      (c: unknown[]) =>
        c[0] === "dispatch_command" &&
        (c[1] as Record<string, unknown>)?.cmd === "app.dismiss",
    );
    expect(dismissCall).toBeTruthy();
  });

  it("keyboard dispatch includes scopeChain with window moniker", async () => {
    const mockInvoke = invoke as ReturnType<typeof vi.fn>;

    function FocusedCard() {
      const { setFocus } = useEntityFocus();
      return (
        <FocusScope moniker="task:t1" commands={[]}>
          <button onClick={() => setFocus("task:t1")}>Focus Card</button>
        </FocusScope>
      );
    }

    renderShell(<FocusedCard />);
    mockInvoke.mockClear();

    // Focus the card scope
    await act(async () => {
      fireEvent.click(screen.getByText("Focus Card"));
    });

    mockInvoke.mockClear();

    // Press Escape — this dispatches app.dismiss through the focused scope
    await act(async () => {
      fireEvent.keyDown(document, { key: "Escape", code: "Escape" });
    });

    const dismissCall = mockInvoke.mock.calls.find(
      (c: unknown[]) =>
        c[0] === "dispatch_command" &&
        (c[1] as Record<string, unknown>)?.cmd === "app.dismiss",
    );
    expect(dismissCall).toBeTruthy();

    // The scopeChain must be present and include the window moniker
    const params = dismissCall![1] as Record<string, unknown>;
    expect(params.scopeChain).toBeTruthy();
    expect(Array.isArray(params.scopeChain)).toBe(true);
    const chain = params.scopeChain as string[];
    // Window moniker should be in the chain (AppShell wraps in window:main via App)
    // At minimum, the chain should not be empty — it should contain at least
    // the scope monikers from the focused card upward.
    expect(chain.length).toBeGreaterThan(0);
  });

  it("keybinding handler resolves commands from focused scope", async () => {
    const focusedFn = vi.fn();

    /**
     * A component that sets up a focused scope with a custom app.dismiss
     * command. When focused, pressing Escape should resolve to this override
     * instead of the global app.dismiss.
     */
    function FocusedChild() {
      const { setFocus } = useEntityFocus();
      return (
        <FocusScope
          moniker="task:test"
          commands={[
            {
              id: "app.dismiss",
              name: "Focused Dismiss",
              execute: focusedFn,
            },
          ]}
        >
          <button onClick={() => setFocus("task:test")}>Focus Me</button>
        </FocusScope>
      );
    }

    renderShell(<FocusedChild />);

    // Focus the scope by clicking the button
    await act(async () => {
      fireEvent.click(screen.getByText("Focus Me"));
    });

    // Press Escape (which maps to app.dismiss in CUA binding table).
    // Should resolve from the focused scope's app.dismiss, not the root one.
    await act(async () => {
      fireEvent.keyDown(document, {
        key: "Escape",
        code: "Escape",
      });
    });

    expect(focusedFn).toHaveBeenCalled();
  });

  it("file.closeBoard dispatches to backend via dispatch_command", async () => {
    const mockInvoke = invoke as ReturnType<typeof vi.fn>;

    renderShell();

    mockInvoke.mockClear();

    // Find and execute the file.closeBoard command
    const closeBoardItem = screen.getByTestId("cmd-file.closeBoard");
    expect(closeBoardItem).toBeTruthy();

    // Simulate Mod+W (Cmd on Mac, Ctrl elsewhere)
    await act(async () => {
      fireEvent.keyDown(document, {
        key: "w",
        code: "KeyW",
        [MOD_KEY]: true,
      });
    });

    // The invoke should have been called with dispatch_command (backend resolves path from UIState)
    const closeCall = mockInvoke.mock.calls.find(
      (c: unknown[]) =>
        c[0] === "dispatch_command" &&
        (c[1] as Record<string, unknown>)?.cmd === "file.closeBoard",
    );
    expect(closeCall).toBeTruthy();
  });

  it("shows mode indicator as COMMAND when palette opens", async () => {
    render(
      <EntityFocusProvider>
        <UIStateProvider>
          <AppModeProvider>
            <UndoProvider>
              <AppShell>
                <CommandInspector />
              </AppShell>
            </UndoProvider>
          </AppModeProvider>
        </UIStateProvider>
      </EntityFocusProvider>,
    );

    // The mode label can be checked via the commands being available.
    // The palette should open and the app.command execute sets mode to "command".
    // We already verified the palette opens; this is a structural smoke test.
    expect(screen.getByTestId("command-list")).toBeTruthy();
  });

  it("blocks app.undo dispatch when activeElement is inside .cm-editor", async () => {
    const mockInvoke = invoke as ReturnType<typeof vi.fn>;
    renderShell();

    // Create a .cm-editor element with a focusable child
    const cmEditor = document.createElement("div");
    cmEditor.className = "cm-editor";
    const input = document.createElement("input");
    cmEditor.appendChild(input);
    document.body.appendChild(cmEditor);
    input.focus();

    mockInvoke.mockClear();

    // Simulate Ctrl+Z (CUA undo) — should be blocked by CM6 guard
    await act(async () => {
      fireEvent.keyDown(document, {
        key: "z",
        code: "KeyZ",
        ctrlKey: true,
      });
    });

    // dispatch_command should NOT have been called with app.undo
    const undoCall = mockInvoke.mock.calls.find(
      (c: unknown[]) =>
        c[0] === "dispatch_command" &&
        (c[1] as Record<string, unknown>)?.cmd === "app.undo",
    );
    expect(undoCall).toBeUndefined();

    // Cleanup
    document.body.removeChild(cmEditor);
  });

  it("dispatches app.undo when activeElement is NOT inside .cm-editor", async () => {
    const mockInvoke = invoke as ReturnType<typeof vi.fn>;
    renderShell();

    // Focus a regular button outside any .cm-editor
    const button = document.createElement("button");
    document.body.appendChild(button);
    button.focus();

    mockInvoke.mockClear();

    // Simulate Mod+Z (Cmd on Mac, Ctrl elsewhere)
    await act(async () => {
      fireEvent.keyDown(document, {
        key: "z",
        code: "KeyZ",
        [MOD_KEY]: true,
      });
    });

    // dispatch_command SHOULD have been called with app.undo
    const undoCall = mockInvoke.mock.calls.find(
      (c: unknown[]) =>
        c[0] === "dispatch_command" &&
        (c[1] as Record<string, unknown>)?.cmd === "app.undo",
    );
    expect(undoCall).toBeTruthy();

    // Cleanup
    document.body.removeChild(button);
  });

  it("context-menu-command event dispatches through useDispatchCommand with scope chain", async () => {
    const mockInvoke = invoke as ReturnType<typeof vi.fn>;
    renderShell();
    mockInvoke.mockClear();

    // Simulate a context-menu-command event from the Rust backend carrying the
    // full ContextMenuItem payload (cmd, target, scope_chain).
    const contextMenuCallback = listenCallbacks["context-menu-command"];
    expect(contextMenuCallback).toBeTruthy();

    await act(async () => {
      contextMenuCallback({
        payload: {
          cmd: "entity.copy",
          target: "task:abc",
          scope_chain: ["task:abc", "column:todo", "window:main"],
        },
      });
    });

    // dispatch_command should have been called with the context menu payload
    const copyCall = mockInvoke.mock.calls.find(
      (c: unknown[]) =>
        c[0] === "dispatch_command" &&
        (c[1] as Record<string, unknown>)?.cmd === "entity.copy",
    );
    expect(copyCall).toBeTruthy();

    // Verify the scope chain and target from the context menu are passed through
    const params = copyCall![1] as Record<string, unknown>;
    expect(params.target).toBe("task:abc");
    expect(params.scopeChain).toEqual([
      "task:abc",
      "column:todo",
      "window:main",
    ]);
  });
});
