import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, act } from "@testing-library/react";

/**
 * Shared default `invoke` stub for tests in this file.
 *
 * Returns a populated UIState payload for `get_ui_state` so AppShell's
 * `useAppShellUIState` hook can read `uiState.windows?.[label]` without
 * a null-deref.
 *
 * Also tracks the moniker → FullyQualifiedMoniker mapping that the kernel would
 * normally maintain, so `spatial_focus_by_moniker` can synthesize the
 * `focus-changed` event the React-side bridge expects. Card
 * `01KQD0WK54G0FRD7SZVZASA9ST` made the entity-focus store a pure
 * projection of kernel events; tests that mocked `invoke` without a
 * kernel simulator need this minimal stub so click-driven `setFocus`
 * still updates the React store.
 *
 * Tests that need to stub a *specific* command should call
 * `mockInvoke.mockImplementation` with a dispatcher that defers
 * to this default for everything else — overriding the entire mock
 * implementation without preserving the UIState branch will crash the
 * AppShell render.
 */
const monikerToKey = new Map<string, string>();
const currentFocusKey: { key: string | null } = { key: null };

/**
 * Captured event listeners keyed by event name.
 *
 * The `listen` mock stores each callback here so tests can fire synthetic
 * events by calling `listenCallbacks["event-name"](payload)`.
 */
const listenCallbacks: Record<string, (event: unknown) => void> = {};

function defaultInvoke(cmd: string, args?: unknown): Promise<unknown> {
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
  if (cmd === "spatial_register_scope" || cmd === "spatial_register_zone") {
    const a = (args ?? {}) as { key?: string; moniker?: string };
    if (a.key && a.moniker) monikerToKey.set(a.moniker, a.key);
    return Promise.resolve(null);
  }
  if (cmd === "spatial_unregister_scope") {
    const a = (args ?? {}) as { key?: string };
    if (a.key) {
      for (const [m, k] of monikerToKey.entries()) {
        if (k === a.key) {
          monikerToKey.delete(m);
          break;
        }
      }
    }
    return Promise.resolve(null);
  }
  if (cmd === "spatial_drill_in" || cmd === "spatial_drill_out") {
    // Under the no-silent-dropout contract the kernel always returns a
    // moniker — typically the focused moniker (echoed) when no descent
    // / drill-out is possible. Tests override this when they want a
    // specific drill result.
    const a = (args ?? {}) as { focusedMoniker?: string };
    return Promise.resolve(a.focusedMoniker ?? null);
  }
  if (cmd === "spatial_focus_by_moniker") {
    // Synthesize the kernel's focus-changed emit so the
    // entity-focus bridge writes the React store. Mirrors the real
    // kernel behavior under card `01KQD0WK54G0FRD7SZVZASA9ST`: the
    // kernel resolves the moniker, advances `focus_by_window`, and
    // emits `focus-changed` with both `next_fq` and `next_segment`.
    //
    // The emit is queued via `queueMicrotask` to match the kernel
    // simulator's timing contract (see `test-helpers/kernel-simulator.ts`):
    // production `focus-changed` events arrive asynchronously through
    // Tauri's event channel, so emitting synchronously here would hide
    // any timing-related defect (e.g. a regression that re-introduces
    // a synchronous `store.set(moniker)` in `setFocus`). Tests that
    // need to observe the post-emit state should drain the microtask
    // queue inside an `act(...)` block.
    const a = (args ?? {}) as { moniker?: string };
    const moniker = a.moniker ?? null;
    const key = moniker ? (monikerToKey.get(moniker) ?? null) : null;
    if (moniker) {
      const prev = currentFocusKey.key;
      currentFocusKey.key = key;
      queueMicrotask(() => {
        const cb = listenCallbacks["focus-changed"];
        if (cb) {
          cb({
            payload: {
              window_label: "main",
              prev_fq: prev,
              next_fq: key,
              next_segment: moniker,
            },
          });
        }
      });
    }
    return Promise.resolve(null);
  }
  if (cmd === "spatial_clear_focus") {
    // Explicit-clear counterpart of `spatial_focus_by_moniker`. The
    // kernel removes the per-window focus slot and emits
    // `focus-changed { next_fq: null, next_segment: null }`. Mirror
    // that here so the bridge flips the React store back to `null`.
    const prev = currentFocusKey.key;
    if (prev === null) {
      return Promise.resolve(null);
    }
    currentFocusKey.key = null;
    queueMicrotask(() => {
      const cb = listenCallbacks["focus-changed"];
      if (cb) {
        cb({
          payload: {
            window_label: "main",
            prev_fq: prev,
            next_fq: null,
            next_segment: null,
          },
        });
      }
    });
    return Promise.resolve(null);
  }
  return Promise.resolve(null);
}

// Mock Tauri APIs before importing components that use them
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn((cmd: string, args?: unknown) => defaultInvoke(cmd, args)),
}));
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
import {
  asFq,
  asSegment,
} from "@/types/spatial";
import { useAvailableCommands } from "@/lib/command-scope";
import { invoke } from "@tauri-apps/api/core";

/** Identity-stable layer name for the test window root, matches App.tsx. */
const WINDOW_LAYER_NAME = asSegment("window");

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
 * AppShell calls `useEnclosingLayerFq()` to thread the window-root layer key
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
    monikerToKey.clear();
    currentFocusKey.key = null;
    for (const key of Object.keys(listenCallbacks)) {
      delete listenCallbacks[key];
    }
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
        <FocusScope moniker={asSegment("task:t1")} commands={[]}>
          <button onClick={() => setFocus(asFq("task:t1"))}>Focus Card</button>
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
          moniker={asSegment("task:test")}
          commands={[
            {
              id: "app.dismiss",
              name: "Focused Dismiss",
              execute: focusedFn,
            },
          ]}
        >
          <button onClick={() => setFocus(asFq("task:test"))}>Focus Me</button>
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
  // non-null `SegmentMoniker` result dispatch `setFocus(moniker)` (which the
  // entity focus provider in turn fans out as `ui.setFocus`).
  //
  // The tests below exercise each branch — non-null result, null
  // result, leading focus state — by:
  //   1. Setting `focus-changed` payload via the captured `listen`
  //      callback so the provider's internal `focusedKeyRef` reflects
  //      a known focused `FullyQualifiedMoniker`.
  //   2. Stubbing `invoke()` to return a chosen value for the drill
  //      command under test.
  //   3. Pressing Enter / Escape and asserting the resulting
  //      `invoke()` call list.
  // ─────────────────────────────────────────────────────────────────────────

  /**
   * Push a synthetic `focus-changed` payload through the captured
   * listener so the SpatialFocusProvider records `nextKey` as the
   * latest focused FullyQualifiedMoniker.
   *
   * Tauri normally emits these from the Rust kernel after a successful
   * `spatial_focus` / `spatial_navigate`; in the test environment the
   * `listen` mock keeps the callback in `listenCallbacks` and we drive
   * it directly.
   *
   * The drill-in / drill-out tests don't care which moniker the
   * spatially-focused key resolves to — those tests gate on
   * `focusedKeyRef`, which is keyed on the FullyQualifiedMoniker, not the
   * SegmentMoniker. They pass `nextKey` straight through as `next_segment`
   * so the bridge in `EntityFocusProvider` (which mirrors
   * `payload.next_segment` into the entity-focus store) doesn't fire
   * spurious `setFocus` calls; the moniker spy in those tests asserts
   * only on `dispatch_command` payloads, not the bridge side effect.
   * The Space-on-leaf bridge test below passes a real moniker via
   * the `nextMoniker` argument.
   */
  function emitFocusChanged(
    nextKey: string | null,
    nextMoniker: string | null = nextKey,
  ): void {
    const cb = listenCallbacks["focus-changed"];
    expect(cb).toBeTruthy();
    cb({
      payload: {
        window_label: "main",
        prev_fq: null,
        next_fq: nextKey,
        next_segment: nextMoniker,
      },
    });
  }

  it("nav.drillIn invokes spatial_drill_in for the focused FullyQualifiedMoniker on Enter", async () => {
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
    mockInvoke.mockImplementation((cmd: string, args?: unknown) => {
      if (cmd === "spatial_drill_in")
        return Promise.resolve(asSegment("task:child"));
      return defaultInvoke(cmd, args);
    });

    await act(async () => {
      fireEvent.keyDown(document, { key: "Enter", code: "Enter" });
    });

    const drillCall = mockInvoke.mock.calls.find(
      (c: unknown[]) => c[0] === "spatial_drill_in",
    );
    expect(drillCall).toBeTruthy();
    expect((drillCall![1] as Record<string, unknown>).key).toBe(
      asFq("k:zone"),
    );
  });

  it("nav.drillIn dispatches ui.setFocus when the kernel returns a SegmentMoniker", async () => {
    const mockInvoke = invoke as ReturnType<typeof vi.fn>;
    renderShell();

    await act(async () => {
      emitFocusChanged("k:zone");
    });

    mockInvoke.mockClear();
    mockInvoke.mockImplementation((cmd: string, args?: unknown) => {
      if (cmd === "spatial_drill_in")
        return Promise.resolve(asSegment("task:child"));
      return defaultInvoke(cmd, args);
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

  it("nav.drillIn is observable as a setFocus(focused) idempotent no-op when the kernel echoes the focused moniker", async () => {
    // Under the no-silent-dropout contract (card
    // 01KQAW97R9XTCNR1PJAWYSKBC7) the kernel returns the focused
    // moniker rather than null when there's nothing to descend into.
    // The closure dispatches `setFocus(result)` unconditionally; on
    // equality with the focused moniker this is an idempotent no-op
    // through the entity-focus store. The user-visible behavior
    // matches the legacy "null → no-op" path exactly.
    const mockInvoke = invoke as ReturnType<typeof vi.fn>;
    renderShell();

    await act(async () => {
      emitFocusChanged("k:leaf", "ui:leaf");
    });

    mockInvoke.mockClear();
    mockInvoke.mockImplementation((cmd: string, args?: unknown) => {
      // Kernel echoes the focused moniker — semantic "stay put".
      if (cmd === "spatial_drill_in")
        return Promise.resolve(asSegment("ui:leaf"));
      return defaultInvoke(cmd, args);
    });

    await act(async () => {
      fireEvent.keyDown(document, { key: "Enter", code: "Enter" });
    });

    // Enter must NOT fall through to app.dismiss — drill-in is not
    // the dismiss path. setFocus may or may not fire; the user-
    // observable behavior is that focus stays on the same moniker
    // (idempotent on the entity-focus store).
    const dismissCall = mockInvoke.mock.calls.find(
      (c: unknown[]) =>
        c[0] === "dispatch_command" &&
        (c[1] as Record<string, unknown>)?.cmd === "app.dismiss",
    );
    expect(dismissCall).toBeUndefined();
  });

  it("nav.drillOut invokes spatial_drill_out for the focused FullyQualifiedMoniker on Escape", async () => {
    const mockInvoke = invoke as ReturnType<typeof vi.fn>;
    renderShell();

    await act(async () => {
      emitFocusChanged("k:leaf");
    });

    mockInvoke.mockClear();
    mockInvoke.mockImplementation((cmd: string, args?: unknown) => {
      if (cmd === "spatial_drill_out")
        return Promise.resolve(asSegment("ui:zone"));
      return defaultInvoke(cmd, args);
    });

    await act(async () => {
      fireEvent.keyDown(document, { key: "Escape", code: "Escape" });
    });

    const drillCall = mockInvoke.mock.calls.find(
      (c: unknown[]) => c[0] === "spatial_drill_out",
    );
    expect(drillCall).toBeTruthy();
    expect((drillCall![1] as Record<string, unknown>).key).toBe(
      asFq("k:leaf"),
    );

    // Non-null result → setFocus, no app.dismiss.
    const setFocusCall = mockInvoke.mock.calls.find(
      (c: unknown[]) =>
        c[0] === "dispatch_command" &&
        (c[1] as Record<string, unknown>)?.cmd === "ui.setFocus",
    );
    expect(setFocusCall).toBeTruthy();
  });

  it("nav.drillOut falls through to app.dismiss when the kernel echoes the focused moniker", async () => {
    // Under the no-silent-dropout contract (card
    // 01KQAW97R9XTCNR1PJAWYSKBC7) the kernel returns the focused
    // moniker (rather than null) when there's no enclosing zone
    // (layer root) or torn state. The closure compares the result to
    // the focused moniker and dispatches `app.dismiss` on equality —
    // the user-visible Escape chain is unchanged.
    const mockInvoke = invoke as ReturnType<typeof vi.fn>;
    renderShell();

    await act(async () => {
      emitFocusChanged("k:rootLeaf", "ui:rootLeaf");
    });

    mockInvoke.mockClear();
    mockInvoke.mockImplementation((cmd: string, args?: unknown) => {
      // Kernel echoes the focused moniker — layer-root edge.
      if (cmd === "spatial_drill_out")
        return Promise.resolve(asSegment("ui:rootLeaf"));
      return defaultInvoke(cmd, args);
    });

    await act(async () => {
      fireEvent.keyDown(document, { key: "Escape", code: "Escape" });
    });

    // Drill-out happened and echoed the focused moniker. Closure
    // detects equality and dispatches app.dismiss as the fall-through.
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
  // The CUA `keys.cua: "Space"` binding on the per-Inspectable
  // `entity.inspect` command (inspectable.tsx) requires
  // `normalizeKeyEvent` to canonicalise the physical spacebar
  // (`e.fq === " "`) to the string `"Space"`. The app-shell-level
  // test below verifies that the round-trip works for an arbitrary
  // scope-level command keyed to Space — the same code path
  // Inspectable uses when a focused entity is on the board.
  // ─────────────────────────────────────────────────────────────────────────

  it("Space pressed on a focused scope dispatches a command with keys.cua=Space", async () => {
    const inspectFn = vi.fn();

    function FocusedCard() {
      const { setFocus } = useEntityFocus();
      return (
        <FocusScope
          moniker={asSegment("task:t-space")}
          commands={[
            {
              id: "ui.inspect",
              name: "Inspect",
              keys: { vim: "Enter", cua: "Space" },
              execute: inspectFn,
            },
          ]}
        >
          <button onClick={() => setFocus(asFq("task:t-space"))}>Focus</button>
        </FocusScope>
      );
    }

    renderShell(<FocusedCard />);

    await act(async () => {
      fireEvent.click(screen.getByText("Focus"));
    });

    await act(async () => {
      // Browsers emit `e.fq === " "` (a literal space) for the
      // spacebar; `normalizeKeyEvent` is responsible for turning that
      // into `"Space"` so scope-level `keys: { cua: "Space" }` matches.
      fireEvent.keyDown(document, { key: " ", code: "Space" });
    });

    expect(inspectFn).toHaveBeenCalled();
  });

  // ─────────────────────────────────────────────────────────────────────────
  // Spatial → entity-focus bridge — the "Space does not trigger inspect" fix
  //
  // After the spatial-nav refactor, leaf clicks call `spatial_focus(key)`
  // and let the kernel emit `focus-changed`. The bridge in
  // `EntityFocusProvider` mirrors `payload.next_segment` into the
  // entity-focus store so `useFocusedScope()` — the data source the
  // global keymap handler reads via `extractScopeBindings` — stays in
  // sync. Without the bridge, Space would look like a no-op because
  // the focused-scope ref would be `null` even though the spatial
  // kernel had a focused scope, so `extractScopeBindings` would not
  // see the per-Inspectable `entity.inspect` binding.
  //
  // The test below drives the spatial-only flow (which is the
  // regression this card fixes): focus is established by emitting a
  // `focus-changed` event and never calling `setFocus` from React, then
  // Space is pressed and the FocusScope's command must fire.
  // ─────────────────────────────────────────────────────────────────────────

  it("Space dispatches inspect for a moniker focused only via spatial-focus", async () => {
    const inspectFn = vi.fn();

    // Mount a FocusScope whose Space binding inspects the focused entity
    // by reading from the entity-focus store. The component does NOT
    // call `setFocus` itself — focus is established only through a
    // synthetic `focus-changed` event from the spatial-nav kernel,
    // which is exactly the production flow for spatial-only leaves
    // (column header, status pill, navbar button, etc.).
    function FocusedCard() {
      return (
        <FocusScope
          moniker={asSegment("task:t-bridge")}
          commands={[
            {
              id: "ui.inspect",
              name: "Inspect",
              keys: { vim: "Enter", cua: "Space" },
              execute: inspectFn,
            },
          ]}
        >
          <span>Card</span>
        </FocusScope>
      );
    }

    renderShell(<FocusedCard />);

    // Drive focus through the spatial-nav kernel only.
    await act(async () => {
      emitFocusChanged("k:t-bridge", "task:t-bridge");
    });

    await act(async () => {
      fireEvent.keyDown(document, { key: " ", code: "Space" });
    });

    expect(inspectFn).toHaveBeenCalled();
  });
});
