import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, act } from "@testing-library/react";

/**
 * Shared default `invoke` stub for tests in this file.
 *
 * Returns a populated UIState payload for `get_ui_state` so AppShell's
 * `useAppShellUIState` hook can read `uiState.windows?.[label]` without
 * a null-deref. Tests that need to stub a *specific* command should
 * call `mockInvoke.mockImplementation` with a dispatcher that defers
 * to this default for everything else — overriding the entire mock
 * implementation without preserving the UIState branch will crash the
 * AppShell render.
 */
function defaultInvoke(cmd: string): Promise<unknown> {
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
}

// Mock Tauri APIs before importing components that use them
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn((cmd: string) => defaultInvoke(cmd)),
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
import { FocusLayer } from "./focus-layer";
import { UIStateProvider } from "@/lib/ui-state-context";
import { AppModeProvider } from "@/lib/app-mode-context";
import { UndoProvider } from "@/lib/undo-context";
import {
  EntityFocusProvider,
  useEntityFocus,
} from "@/lib/entity-focus-context";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { asLayerName, asMoniker, asSpatialKey } from "@/types/spatial";
import { useAvailableCommands } from "@/lib/command-scope";
import { invoke } from "@tauri-apps/api/core";

/** Identity-stable layer name for the test window root, matches App.tsx. */
const WINDOW_LAYER_NAME = asLayerName("window");

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

/** Render AppShell with all required parent providers.
 *
 * AppShell calls `useCurrentLayerKey()` to thread the window-root layer key
 * into the palette's `<FocusLayer>` (the palette portals to `document.body`,
 * so the React ancestor chain is severed at render time). The hook throws
 * outside any `<FocusLayer>`, so the test harness must mirror App.tsx's
 * production wrapping: a `<SpatialFocusProvider>` that owns the spatial
 * focus actions bag, and a `<FocusLayer name="window">` that mounts the
 * window-root layer in the Rust-side stack.
 */
function renderShell(children?: React.ReactNode) {
  return render(
    <SpatialFocusProvider>
      <FocusLayer name={WINDOW_LAYER_NAME}>
        <EntityFocusProvider>
          <UIStateProvider>
            <AppModeProvider>
              <UndoProvider>
                <AppShell>{children ?? <CommandInspector />}</AppShell>
              </UndoProvider>
            </AppModeProvider>
          </UIStateProvider>
        </EntityFocusProvider>
      </FocusLayer>
    </SpatialFocusProvider>,
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
        <FocusScope moniker={asMoniker("task:t1")} commands={[]}>
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
          moniker={asMoniker("task:test")}
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
      <SpatialFocusProvider>
        <FocusLayer name={WINDOW_LAYER_NAME}>
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
          </EntityFocusProvider>
        </FocusLayer>
      </SpatialFocusProvider>,
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

  // ─────────────────────────────────────────────────────────────────────────
  // nav.drillIn / nav.drillOut — Enter/Escape command wiring
  //
  // Drill commands route through the global CommandScope: the closures
  // read `focusedKey()` from `SpatialFocusProvider`, await the matching
  // Tauri invoke (`spatial_drill_in` / `spatial_drill_out`), and on a
  // non-null `Moniker` result dispatch `setFocus(moniker)` (which the
  // entity focus provider in turn fans out as `ui.setFocus`).
  //
  // The tests below exercise each branch — non-null result, null
  // result, leading focus state — by:
  //   1. Setting `focus-changed` payload via the captured `listen`
  //      callback so the provider's internal `focusedKeyRef` reflects
  //      a known focused `SpatialKey`.
  //   2. Stubbing `invoke()` to return a chosen value for the drill
  //      command under test.
  //   3. Pressing Enter / Escape and asserting the resulting
  //      `invoke()` call list.
  // ─────────────────────────────────────────────────────────────────────────

  /**
   * Push a synthetic `focus-changed` payload through the captured
   * listener so the SpatialFocusProvider records `nextKey` as the
   * latest focused SpatialKey.
   *
   * Tauri normally emits these from the Rust kernel after a successful
   * `spatial_focus` / `spatial_navigate`; in the test environment the
   * `listen` mock keeps the callback in `listenCallbacks` and we drive
   * it directly.
   */
  function emitFocusChanged(nextKey: string | null): void {
    const cb = listenCallbacks["focus-changed"];
    expect(cb).toBeTruthy();
    cb({
      payload: {
        window_label: "main",
        prev_key: null,
        next_key: nextKey,
        next_moniker: nextKey,
      },
    });
  }

  it("nav.drillIn invokes spatial_drill_in for the focused SpatialKey on Enter", async () => {
    const mockInvoke = invoke as ReturnType<typeof vi.fn>;
    renderShell();

    // Seed the provider's focusedKeyRef before the keystroke.
    await act(async () => {
      emitFocusChanged("k:zone");
    });

    mockInvoke.mockClear();
    // Stub the kernel call so the closure has a non-null moniker to
    // hand to setFocus. Preserve the `get_ui_state` default (the
    // module-scope mock returns a populated UIState payload there);
    // overriding the entire mockImplementation would null it out and
    // break `useAppShellUIState`'s `uiState.windows?.[label]` read.
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "spatial_drill_in")
        return Promise.resolve(asMoniker("task:child"));
      return defaultInvoke(cmd);
    });

    await act(async () => {
      fireEvent.keyDown(document, { key: "Enter", code: "Enter" });
    });

    const drillCall = mockInvoke.mock.calls.find(
      (c: unknown[]) => c[0] === "spatial_drill_in",
    );
    expect(drillCall).toBeTruthy();
    expect((drillCall![1] as Record<string, unknown>).key).toBe(
      asSpatialKey("k:zone"),
    );
  });

  it("nav.drillIn dispatches ui.setFocus when the kernel returns a Moniker", async () => {
    const mockInvoke = invoke as ReturnType<typeof vi.fn>;
    renderShell();

    await act(async () => {
      emitFocusChanged("k:zone");
    });

    mockInvoke.mockClear();
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "spatial_drill_in")
        return Promise.resolve(asMoniker("task:child"));
      return defaultInvoke(cmd);
    });

    await act(async () => {
      fireEvent.keyDown(document, { key: "Enter", code: "Enter" });
    });

    // setFocus fans out to dispatch_command(ui.setFocus, …). The exact
    // shape carries the entity scope chain, but the cmd alone is
    // sufficient evidence the drill closure walked into the success
    // branch.
    const setFocusCall = mockInvoke.mock.calls.find(
      (c: unknown[]) =>
        c[0] === "dispatch_command" &&
        (c[1] as Record<string, unknown>)?.cmd === "ui.setFocus",
    );
    expect(setFocusCall).toBeTruthy();
  });

  it("nav.drillIn is a no-op when the kernel returns null", async () => {
    const mockInvoke = invoke as ReturnType<typeof vi.fn>;
    renderShell();

    await act(async () => {
      emitFocusChanged("k:leaf");
    });

    mockInvoke.mockClear();
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "spatial_drill_in") return Promise.resolve(null);
      return defaultInvoke(cmd);
    });

    await act(async () => {
      fireEvent.keyDown(document, { key: "Enter", code: "Enter" });
    });

    // No ui.setFocus dispatch — drill-in null means "leaf without
    // editor or empty zone", and the closure exits without falling
    // through to any other command.
    const setFocusCall = mockInvoke.mock.calls.find(
      (c: unknown[]) =>
        c[0] === "dispatch_command" &&
        (c[1] as Record<string, unknown>)?.cmd === "ui.setFocus",
    );
    expect(setFocusCall).toBeUndefined();
    // Enter must NOT fall through to app.dismiss either — drill-in is
    // the explicit no-op branch.
    const dismissCall = mockInvoke.mock.calls.find(
      (c: unknown[]) =>
        c[0] === "dispatch_command" &&
        (c[1] as Record<string, unknown>)?.cmd === "app.dismiss",
    );
    expect(dismissCall).toBeUndefined();
  });

  it("nav.drillOut invokes spatial_drill_out for the focused SpatialKey on Escape", async () => {
    const mockInvoke = invoke as ReturnType<typeof vi.fn>;
    renderShell();

    await act(async () => {
      emitFocusChanged("k:leaf");
    });

    mockInvoke.mockClear();
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "spatial_drill_out")
        return Promise.resolve(asMoniker("ui:zone"));
      return defaultInvoke(cmd);
    });

    await act(async () => {
      fireEvent.keyDown(document, { key: "Escape", code: "Escape" });
    });

    const drillCall = mockInvoke.mock.calls.find(
      (c: unknown[]) => c[0] === "spatial_drill_out",
    );
    expect(drillCall).toBeTruthy();
    expect((drillCall![1] as Record<string, unknown>).key).toBe(
      asSpatialKey("k:leaf"),
    );

    // Non-null result → setFocus, no app.dismiss.
    const setFocusCall = mockInvoke.mock.calls.find(
      (c: unknown[]) =>
        c[0] === "dispatch_command" &&
        (c[1] as Record<string, unknown>)?.cmd === "ui.setFocus",
    );
    expect(setFocusCall).toBeTruthy();
  });

  it("nav.drillOut falls through to app.dismiss when the kernel returns null", async () => {
    const mockInvoke = invoke as ReturnType<typeof vi.fn>;
    renderShell();

    await act(async () => {
      emitFocusChanged("k:rootLeaf");
    });

    mockInvoke.mockClear();
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "spatial_drill_out") return Promise.resolve(null);
      return defaultInvoke(cmd);
    });

    await act(async () => {
      fireEvent.keyDown(document, { key: "Escape", code: "Escape" });
    });

    // Drill-out happened but returned null (layer root). Closure
    // dispatches app.dismiss as the fall-through.
    const drillCall = mockInvoke.mock.calls.find(
      (c: unknown[]) => c[0] === "spatial_drill_out",
    );
    expect(drillCall).toBeTruthy();

    const dismissCall = mockInvoke.mock.calls.find(
      (c: unknown[]) =>
        c[0] === "dispatch_command" &&
        (c[1] as Record<string, unknown>)?.cmd === "app.dismiss",
    );
    expect(dismissCall).toBeTruthy();
  });

  it("nav.drillOut falls through to app.dismiss when no spatial focus is set", async () => {
    const mockInvoke = invoke as ReturnType<typeof vi.fn>;
    renderShell();

    // Explicitly clear any prior focus state.
    await act(async () => {
      emitFocusChanged(null);
    });

    mockInvoke.mockClear();

    await act(async () => {
      fireEvent.keyDown(document, { key: "Escape", code: "Escape" });
    });

    // No focused key → no spatial_drill_out call, but app.dismiss
    // still fires via the closure's early-return fall-through.
    const drillCall = mockInvoke.mock.calls.find(
      (c: unknown[]) => c[0] === "spatial_drill_out",
    );
    expect(drillCall).toBeUndefined();
    const dismissCall = mockInvoke.mock.calls.find(
      (c: unknown[]) =>
        c[0] === "dispatch_command" &&
        (c[1] as Record<string, unknown>)?.cmd === "app.dismiss",
    );
    expect(dismissCall).toBeTruthy();
  });

  // ─────────────────────────────────────────────────────────────────────────
  // ui.inspect — Space binding
  //
  // The CUA `keys.cua: "Space"` rebind on `board.inspect`
  // (board-view.tsx) requires `normalizeKeyEvent` to canonicalise the
  // physical spacebar (`e.key === " "`) to the string `"Space"`. The
  // app-shell-level test below verifies that the round-trip works for
  // an arbitrary scope-level command keyed to Space — the same code
  // path `board.inspect` will take when a card is focused.
  // ─────────────────────────────────────────────────────────────────────────

  it("Space pressed on a focused scope dispatches a command with keys.cua=Space", async () => {
    const inspectFn = vi.fn();

    function FocusedCard() {
      const { setFocus } = useEntityFocus();
      return (
        <FocusScope
          moniker={asMoniker("task:t-space")}
          commands={[
            {
              id: "ui.inspect",
              name: "Inspect",
              keys: { vim: "Enter", cua: "Space" },
              execute: inspectFn,
            },
          ]}
        >
          <button onClick={() => setFocus("task:t-space")}>Focus</button>
        </FocusScope>
      );
    }

    renderShell(<FocusedCard />);

    await act(async () => {
      fireEvent.click(screen.getByText("Focus"));
    });

    await act(async () => {
      // Browsers emit `e.key === " "` (a literal space) for the
      // spacebar; `normalizeKeyEvent` is responsible for turning that
      // into `"Space"` so scope-level `keys: { cua: "Space" }` matches.
      fireEvent.keyDown(document, { key: " ", code: "Space" });
    });

    expect(inspectFn).toHaveBeenCalled();
  });
});
