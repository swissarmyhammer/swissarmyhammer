import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, act } from "@testing-library/react";

/**
 * Frontend regression: `AppShell` registers the five `ai.*` window-layer
 * commands in `globalCommands`, each with a callable `execute` closure, and
 * `ai.cancel` is availability-gated to the AI conversation's streaming state.
 *
 * The command metadata (id, name, keys) mirrors
 * `swissarmyhammer-kanban`'s `builtin/commands/ai.yaml`; execution lives here
 * in `app-shell.tsx` because the closures call into the `ai/commands.ts`
 * module registry the AI panel components populate. This test pins:
 *
 *   - all five `ai.*` ids resolve from the window-layer scope chain;
 *   - each carries a non-null `execute` closure;
 *   - `ai.cancel` is excluded from `useAvailableCommands()` when the
 *     conversation is idle and included while it streams — the
 *     "unavailable when idle, available while streaming" contract.
 */

/** Default `invoke` stub returning a populated UIState payload. */
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

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn((cmd: string) => defaultInvoke(cmd)),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ label: "main" }),
}));
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
import { useAvailableCommands, type CommandDef } from "@/lib/command-scope";
import { resetAiCommandsForTest, setAiStreaming } from "@/ai/commands";

/** Identity-stable layer name for the test window root, matches App.tsx. */
const WINDOW_LAYER_NAME = asSegment("window");

/**
 * Render-phase probe: snapshots the CommandScope's available commands into a
 * map keyed by id so the test can assert on the exact `CommandDef`s `AppShell`
 * pushed onto the scope. `useAvailableCommands` runs `collectAvailableCommands`
 * which excludes any `CommandDef` with `available: false` — so an absent
 * `ai.cancel` entry IS the "unavailable" signal.
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

/** Render `AppShell` with all required parent providers. */
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

describe("AppShell ai.* commands", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    capturedCommands.clear();
    resetAiCommandsForTest();
  });

  it("registers ai.toggle, ai.focus, ai.newChat, ai.model with execute closures", async () => {
    await act(async () => {
      renderShell();
    });
    expect(screen.getByTestId("command-probe-ready")).toBeTruthy();

    for (const id of ["ai.toggle", "ai.focus", "ai.newChat", "ai.model"]) {
      const cmd = capturedCommands.get(id);
      expect(cmd, `globalCommands missing ${id}`).toBeTruthy();
      expect(
        typeof cmd!.execute,
        `${id} must carry a callable execute closure`,
      ).toBe("function");
    }
  });

  it("the ai.* commands carry the documented keybindings", async () => {
    await act(async () => {
      renderShell();
    });
    // `ai.cancel` is `available: false` when idle, so the probe (which runs
    // `collectAvailableCommands`) drops it — stream so all five are present.
    await act(async () => {
      setAiStreaming(true);
    });
    // Lowercase canonical form `normalizeKeyEvent` emits, matching
    // `BINDING_TABLES` and the rest of `STATIC_GLOBAL_COMMANDS`.
    expect(capturedCommands.get("ai.toggle")!.keys).toEqual({
      vim: "Mod+j",
      cua: "Mod+j",
      emacs: "Mod+j",
    });
    expect(capturedCommands.get("ai.focus")!.keys).toEqual({
      vim: "Mod+i",
      cua: "Mod+i",
      emacs: "Mod+i",
    });
    expect(capturedCommands.get("ai.newChat")!.keys).toEqual({
      vim: "Mod+Shift+J",
      cua: "Mod+Shift+J",
      emacs: "Mod+Shift+J",
    });
    expect(capturedCommands.get("ai.cancel")!.keys).toEqual({
      vim: "Mod+.",
      cua: "Mod+.",
      emacs: "Mod+.",
    });
  });

  it("ai.cancel is unavailable when idle and available while streaming", async () => {
    // Idle (the default): `collectAvailableCommands` drops `available: false`
    // commands, so `ai.cancel` must not appear in the probe.
    await act(async () => {
      renderShell();
    });
    expect(capturedCommands.get("ai.cancel")).toBeUndefined();

    // The conversation starts streaming — `AppShell` re-renders via the
    // `useAiStreaming` external store and rebuilds `ai.cancel` available.
    await act(async () => {
      setAiStreaming(true);
    });
    const cancel = capturedCommands.get("ai.cancel");
    expect(cancel, "ai.cancel must appear while streaming").toBeTruthy();
    expect(typeof cancel!.execute).toBe("function");

    // The turn ends — `ai.cancel` is gated out again.
    await act(async () => {
      setAiStreaming(false);
    });
    expect(capturedCommands.get("ai.cancel")).toBeUndefined();
  });
});
