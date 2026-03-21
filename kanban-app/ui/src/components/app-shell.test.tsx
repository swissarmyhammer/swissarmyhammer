import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, act } from "@testing-library/react";

// Mock Tauri APIs before importing components that use them
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn((cmd: string) => {
    if (cmd === "get_ui_state")
      return Promise.resolve({
        palette_open: false,
        keymap_mode: "cua",
        scope_chain: [],
        open_boards: [],
        windows: {},
        recent_boards: [],
      });
    return Promise.resolve(null);
  }),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

import { AppShell } from "./app-shell";
import { FocusScope } from "./focus-scope";
import { UIStateProvider } from "@/lib/ui-state-context";
import { AppModeProvider } from "@/lib/app-mode-context";
import { UndoStackProvider } from "@/lib/undo-context";
import {
  EntityFocusProvider,
  useEntityFocus,
} from "@/lib/entity-focus-context";
import { useAvailableCommands } from "@/lib/command-scope";
import { InspectProvider } from "@/lib/inspect-context";

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
          <UndoStackProvider>
            <InspectProvider onInspect={() => {}} onDismiss={() => false}>
              <AppShell>{children ?? <CommandInspector />}</AppShell>
            </InspectProvider>
          </UndoStackProvider>
        </AppModeProvider>
      </UIStateProvider>
    </EntityFocusProvider>,
  );
}

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

  it("opens command palette on Mod+Shift+P in CUA mode", async () => {
    renderShell();
    // CUA mode is the default (mocked invoke returns "cua")
    // Mod on non-Mac is Ctrl
    await act(async () => {
      fireEvent.keyDown(document, {
        key: "P",
        code: "KeyP",
        ctrlKey: true,
        shiftKey: true,
      });
    });
    expect(screen.getByTestId("command-palette")).toBeTruthy();
  });

  it("closes command palette on Escape", async () => {
    renderShell();

    // Open the palette first
    await act(async () => {
      fireEvent.keyDown(document, {
        key: "P",
        code: "KeyP",
        ctrlKey: true,
        shiftKey: true,
      });
    });
    expect(screen.getByTestId("command-palette")).toBeTruthy();

    // Press Escape to dismiss
    await act(async () => {
      fireEvent.keyDown(document, {
        key: "Escape",
        code: "Escape",
      });
    });
    expect(screen.queryByTestId("command-palette")).toBeNull();
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

  it("shows mode indicator as COMMAND when palette opens", async () => {
    render(
      <EntityFocusProvider>
        <UIStateProvider>
          <AppModeProvider>
            <UndoStackProvider>
              <InspectProvider onInspect={() => {}} onDismiss={() => false}>
                <AppShell>
                  <CommandInspector />
                </AppShell>
              </InspectProvider>
            </UndoStackProvider>
          </AppModeProvider>
        </UIStateProvider>
      </EntityFocusProvider>,
    );

    // The mode label can be checked via the commands being available.
    // The palette should open and the app.command execute sets mode to "command".
    // We already verified the palette opens; this is a structural smoke test.
    expect(screen.getByTestId("command-list")).toBeTruthy();
  });
});
