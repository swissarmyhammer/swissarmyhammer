import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, act, fireEvent } from "@testing-library/react";

/**
 * Frontend regression: AppShell exposes the nine `nav.*` commands as
 * CommandDefs in `globalCommands`, each carrying a non-null `execute`
 * closure.
 *
 * The metadata (id, name, keys, menu) ships from the Rust
 * `swissarmyhammer-focus` crate's YAML stub
 * (`builtin/commands/nav.yaml`); execution lives here in
 * `app-shell.tsx` because the closures need live `SpatialFocusActions`
 * (or, for `nav.jump`, the AppShell's `jumpOpen` setter).
 * If `buildNavCommands` / `buildDrillCommands` / the `nav.jump` wiring
 * ever stop emitting one of the nine ids (or stop wiring `execute`),
 * this test fails before the user discovers the broken keybinding.
 */

/**
 * Captured `listen` callbacks keyed by event name. AppShell's
 * `KeybindingHandler` calls `listen("focus-changed", …)` etc.; mocking
 * `listen` to push into this map lets tests fire synthetic events.
 */
const listenCallbacks: Record<string, (event: unknown) => void> = {};

/**
 * Mutable keymap mode for the default `invoke` stub. Lets a single test
 * opt into a non-cua mode (e.g. vim for the `s` → jump-to assertion)
 * without minting a parallel mock module.
 */
let invokeKeymapMode: "vim" | "cua" | "emacs" = "cua";

import { commandToolCall } from "@/test/mock-command-list";

/** Default `invoke` stub returning a populated UIState payload. */
function defaultInvoke(cmd: string, args?: unknown): Promise<unknown> {
  if (cmd === "get_ui_state")
    return Promise.resolve({
      palette_open: false,
      palette_mode: "command",
      keymap_mode: invokeKeymapMode,
      scope_chain: [],
      open_boards: [],
      windows: {},
      recent_boards: [],
    });
  // The hotkey global layer is sourced from the Command registry via
  // `useCommandList` → `command_tool_call("list command")`. Serve the global
  // set (including vim `s` → nav.jump) so global keybindings resolve.
  if (cmd === "command_tool_call") return commandToolCall(args);
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
 * Stub `<JumpToOverlay>` so this test can assert on the AppShell's
 * `jumpOpen` flag without spinning up the overlay's full focus-layer /
 * sneak-code / portal stack. The real overlay's mount-and-self-dismiss
 * path (when no scopes are enumerable, the body unmounts immediately
 * via `onClose`) makes a "render → assert" flow flaky, and the
 * overlay's own contract is covered by `jump-to-overlay.browser.test.tsx`.
 *
 * The stub renders a stable sentinel `<div>` whenever `open` is true,
 * so this test can verify that AppShell flips `jumpOpen` in response
 * to the keystroke.
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
  type CommandDef,
} from "@/lib/command-scope";

/** Identity-stable layer name for the test window root, matches App.tsx. */
const WINDOW_LAYER_NAME = asSegment("window");

/**
 * Render-phase probe: snapshots the CommandScope's available commands
 * by id into a global map keyed by `id` so the test can assert on the
 * exact CommandDefs AppShell pushed onto the scope without serialising
 * the closures through the DOM.
 */
const capturedCommands = new Map<string, CommandDef>();

function CommandProbe() {
  const commands = useAvailableCommands();
  capturedCommands.clear();
  for (const c of commands) {
    capturedCommands.set(c.command.id, c.command);
  }
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

describe("AppShell nav.* commands", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    capturedCommands.clear();
    invokeKeymapMode = "cua";
    for (const key of Object.keys(listenCallbacks)) {
      delete listenCallbacks[key];
    }
  });

  it("registers all nine nav.* commands with non-null execute closures", async () => {
    // UIStateProvider fetches state asynchronously on mount; wrap in
    // act() so the resulting state update doesn't log a warning.
    await act(async () => {
      renderShell();
    });
    // Sanity: the probe rendered, so commands are populated.
    expect(screen.getByTestId("command-probe-ready")).toBeTruthy();

    const expectedIds = [
      "nav.up",
      "nav.down",
      "nav.left",
      "nav.right",
      "nav.first",
      "nav.last",
      "nav.drillIn",
      "nav.drillOut",
      "nav.jump",
    ] as const;

    for (const id of expectedIds) {
      const cmd = capturedCommands.get(id);
      expect(cmd, `globalCommands missing ${id}`).toBeTruthy();
      // Execution lives in React closures (they need
      // `SpatialFocusActions`, or for `nav.jump`, the AppShell's
      // `jumpOpen` setter). The YAML stubs are pure metadata, so the
      // closure must come from `buildNavCommands` /
      // `buildDrillCommands` / the AppShell `nav.jump` wiring —
      // `execute` must be a function.
      expect(
        typeof cmd!.execute,
        `${id} must carry a callable execute closure`,
      ).toBe("function");
    }
  });

  it("opens the Jump-To overlay when vim mode `s` fires the nav.jump command", async () => {
    invokeKeymapMode = "vim";
    await act(async () => {
      renderShell();
    });

    // Sanity: probe rendered and nav.jump is registered.
    expect(screen.getByTestId("command-probe-ready")).toBeTruthy();
    expect(capturedCommands.get("nav.jump")).toBeTruthy();

    // Overlay stub is closed before the keystroke (AppShell's
    // `jumpOpen` defaults to false).
    expect(screen.queryByTestId("jump-to-overlay-stub")).toBeNull();

    // Simulate vim-mode `s` keystroke at the document level (the global
    // key handler attaches its `keydown` listener on `document`). Wrap
    // in act() because firing the keystroke triggers a setState
    // (`setJumpOpen(true)`).
    await act(async () => {
      fireEvent.keyDown(document, { key: "s" });
    });

    // After the keystroke fires the `nav.jump` command's `execute`
    // closure, AppShell's `jumpOpen` flips to true and the stub
    // overlay renders its sentinel.
    expect(screen.getByTestId("jump-to-overlay-stub")).toBeTruthy();
  });
});
