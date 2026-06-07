import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, act } from "@testing-library/react";

/**
 * Frontend regression for Card A — the `nav.*` commands moved OUT of
 * `app-shell.tsx` into the `nav-commands` builtin plugin.
 *
 * The old `AppShell` registered all nine `nav.*` ids as React `CommandDef`
 * closures in `globalCommands` (via `buildNavCommands` / `buildDrillCommands`
 * and an inline `nav.jump`). Those builders are deleted. Now:
 *
 *   - The directional / first-last / drill `nav.*` commands are plugin
 *     commands that execute host-side through the `focus` kernel, so
 *     dispatching one routes to the backend (`dispatch_command`) — NOT a React
 *     closure, NOT a snapshot threaded from React.
 *   - `nav.jump` is a plugin command with NO backend op: AppShell registers a
 *     webview-bus handler for the id (`registerWebviewCommandHandler`) that
 *     opens the `<JumpToOverlay>`. Dispatching `nav.jump` runs that handler and
 *     never reaches the backend.
 *
 * This test pins all three properties so a regression (re-introducing a React
 * nav closure, or breaking the bus wiring) fails before a user does.
 */

/** Captured `listen` callbacks keyed by event name. */
const listenCallbacks: Record<string, (event: unknown) => void> = {};

import { commandToolCall } from "@/test/mock-command-list";

/** Records every `invoke` call so the test can assert backend dispatch. */
const invokeCalls: Array<{ cmd: string; args?: unknown }> = [];

/** Default `invoke` stub returning a populated UIState payload. */
function defaultInvoke(cmd: string, args?: unknown): Promise<unknown> {
  invokeCalls.push({ cmd, args });
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
  // The hotkey global layer is sourced from the Command registry via
  // `useCommandList` → `command_tool_call("list command")`.
  if (cmd === "command_tool_call") return commandToolCall(args);
  // `dispatch_command` is the lowering target of `execute command` — the
  // backend path a plugin `nav.*` command takes. Return a benign envelope.
  if (cmd === "dispatch_command") return Promise.resolve({ ok: true });
  return Promise.resolve(null);
}

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

/**
 * Stub `<JumpToOverlay>` so this test asserts the AppShell's `jumpOpen` flag
 * without spinning up the overlay's full focus-layer / sneak-code / portal
 * stack. Renders a stable sentinel `<div>` whenever `open` is true.
 */
vi.mock("./jump-to-overlay", () => ({
  JumpToOverlay: ({ open }: { open: boolean; onClose: () => void }) =>
    open ? <div data-testid="jump-to-overlay-stub" /> : null,
}));

import { AppShell } from "./app-shell";
import { FocusLayer } from "./focus-layer";
import { UIStateProvider } from "@/lib/ui-state-context";
import { AppModeProvider } from "@/lib/app-mode-context";
import { UndoProvider } from "@/lib/undo-context";
import { EntityFocusProvider } from "@/lib/entity-focus-context";
import { SpatialFocusProvider } from "@/lib/spatial-focus-context";
import { asSegment } from "@/types/spatial";
import {
  useAvailableCommands,
  useDispatchCommand,
  type CommandDef,
} from "@/lib/command-scope";
import { resetWebviewCommandBusForTest } from "@/lib/webview-command-bus";

/** Identity-stable layer name for the test window root, matches App.tsx. */
const WINDOW_LAYER_NAME = asSegment("window");

/**
 * Render-phase probe: snapshots the CommandScope's available commands by id so
 * the test can assert which `nav.*` ids AppShell did (or did NOT) push onto the
 * scope. Also exposes a dispatcher so a test can fire a `nav.*` id and observe
 * where it routes.
 */
const capturedCommands = new Map<string, CommandDef>();
let dispatch: (id: string) => Promise<unknown> = async () => undefined;

function CommandProbe() {
  const commands = useAvailableCommands();
  capturedCommands.clear();
  for (const c of commands) {
    capturedCommands.set(c.command.id, c.command);
  }
  dispatch = useDispatchCommand();
  return <div data-testid="command-probe-ready" />;
}

/** Render AppShell with all required parent providers. */
function renderShell() {
  return render(
    <SpatialFocusProvider>
      <FocusLayer name={WINDOW_LAYER_NAME}>
        <EntityFocusProvider>
          <UIStateProvider>
            <AppModeProvider>
              <UndoProvider>
                <AppShell>
                  <CommandProbe />
                </AppShell>
              </UndoProvider>
            </AppModeProvider>
          </UIStateProvider>
        </EntityFocusProvider>
      </FocusLayer>
    </SpatialFocusProvider>,
  );
}

describe("AppShell nav.* commands (moved to nav-commands plugin)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    capturedCommands.clear();
    invokeCalls.length = 0;
    resetWebviewCommandBusForTest();
    for (const key of Object.keys(listenCallbacks)) {
      delete listenCallbacks[key];
    }
  });

  it("no longer registers any nav.* as a React global command closure", async () => {
    await act(async () => {
      renderShell();
    });
    expect(screen.getByTestId("command-probe-ready")).toBeTruthy();

    // The deleted builders (`buildNavCommands` / `buildDrillCommands` / the
    // inline `nav.jump`) must leave NO `nav.*` CommandDef in the global scope.
    for (const id of [
      "nav.up",
      "nav.down",
      "nav.left",
      "nav.right",
      "nav.first",
      "nav.last",
      "nav.drillIn",
      "nav.drillOut",
      "nav.jump",
    ]) {
      expect(
        capturedCommands.has(id),
        `${id} must NOT be a React global command (it is a plugin command now)`,
      ).toBe(false);
    }
  });

  it("dispatching nav.jump opens the Jump-To overlay via the webview bus", async () => {
    await act(async () => {
      renderShell();
    });
    expect(screen.getByTestId("command-probe-ready")).toBeTruthy();

    // Overlay closed before dispatch.
    expect(screen.queryByTestId("jump-to-overlay-stub")).toBeNull();

    // Dispatching the plugin id runs the webview-bus handler AppShell
    // registered — no backend dispatch_command is issued for nav.jump.
    await act(async () => {
      await dispatch("nav.jump");
    });

    expect(screen.getByTestId("jump-to-overlay-stub")).toBeTruthy();
    expect(
      invokeCalls.some(
        (c) =>
          c.cmd === "dispatch_command" &&
          (c.args as { cmd?: string } | undefined)?.cmd === "nav.jump",
      ),
      "nav.jump must NOT reach the backend — the webview bus handles it",
    ).toBe(false);
  });

  it("dispatching nav.up routes to the backend focus op (host-driven, no React closure)", async () => {
    await act(async () => {
      renderShell();
    });
    expect(screen.getByTestId("command-probe-ready")).toBeTruthy();

    await act(async () => {
      await dispatch("nav.up");
    });

    // With no React closure and no webview-bus handler for nav.up, the
    // dispatch falls through to the backend — `execute command` lowered onto
    // `dispatch_command` with `cmd: "nav.up"`. The host-side plugin command
    // then drives the focus kernel; the snapshot is pulled host-side (F2), so
    // NO geometry is threaded from React here.
    expect(
      invokeCalls.some(
        (c) =>
          c.cmd === "dispatch_command" &&
          (c.args as { cmd?: string } | undefined)?.cmd === "nav.up",
      ),
      "nav.up must route to the backend dispatch path",
    ).toBe(true);
  });

  it("dispatching nav.drillIn routes to the backend focus op", async () => {
    await act(async () => {
      renderShell();
    });
    expect(screen.getByTestId("command-probe-ready")).toBeTruthy();

    await act(async () => {
      await dispatch("nav.drillIn");
    });

    expect(
      invokeCalls.some(
        (c) =>
          c.cmd === "dispatch_command" &&
          (c.args as { cmd?: string } | undefined)?.cmd === "nav.drillIn",
      ),
      "nav.drillIn must route to the backend dispatch path",
    ).toBe(true);
  });
});
