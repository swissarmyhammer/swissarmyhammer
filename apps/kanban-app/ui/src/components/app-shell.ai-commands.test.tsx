import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, act } from "@testing-library/react";

/**
 * Frontend regression for Card I: `AppShell` no longer builds any `ai.*`
 * `CommandDef` client-side — the five window-layer AI commands are DEFINED
 * solely by the `ai-commands` builtin plugin (id, name, keys, menu in the
 * unified registry; see `builtin/plugins/ai-commands/index.ts` and the
 * `ai-plugin-commands-mirror.spatial.node.test.ts` keys drift guard).
 *
 * What `AppShell` owns instead is the EXECUTION seam: it registers a webview
 * command-bus handler (`registerWebviewCommandHandler`) for each `ai.*` id on
 * mount, routing a dispatched id through the `ai/commands.ts` module registry
 * (`triggerAiToggle` / `triggerAiFocus` / …) into the mounted AI panel and
 * skipping the backend entirely. This test pins:
 *
 *   - NO `ai.*` `CommandDef` is registered in the window-layer React command
 *     scope (the client-built duplicate definition is gone);
 *   - the five ids are webview-bus handled while `AppShell` is mounted, and
 *     the handlers are cleaned up on unmount;
 *   - dispatching `ai.toggle` runs the module-registry handler with no
 *     backend `dispatch_command` round-trip;
 *   - `ai.model` threads `args.model` through to the registry;
 *   - `ai.cancel` keeps its availability gate: a no-op while the conversation
 *     is idle, cancels while it streams (the gate reads `aiStreaming()` at
 *     dispatch time inside the bus handler).
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
import {
  useAvailableCommands,
  useDispatchCommand,
  type CommandDef,
  type DispatchOptions,
} from "@/lib/command-scope";
import {
  hasWebviewCommandHandler,
  resetWebviewCommandBusForTest,
} from "@/lib/webview-command-bus";
import {
  registerAiCommandHandlers,
  resetAiCommandsForTest,
  setAiStreaming,
} from "@/ai/commands";
import { invoke } from "@tauri-apps/api/core";

/** Identity-stable layer name for the test window root, matches App.tsx. */
const WINDOW_LAYER_NAME = asSegment("window");

/** The five plugin-defined window-layer AI command ids. */
const AI_IDS = [
  "ai.toggle",
  "ai.focus",
  "ai.newChat",
  "ai.model",
  "ai.cancel",
] as const;

/**
 * Render-phase probe: snapshots the CommandScope's available commands into a
 * map keyed by id, and captures the unified dispatcher so tests can dispatch
 * command ids exactly as keybindings / the palette do.
 */
const capturedCommands = new Map<string, CommandDef>();
let dispatch:
  | ((cmd: string, opts?: DispatchOptions) => Promise<unknown>)
  | null = null;

function CommandProbe() {
  const commands = useAvailableCommands();
  dispatch = useDispatchCommand();
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

/** Collect the `cmd` of every backend `dispatch_command` invoke call. */
function backendDispatchCmds(): string[] {
  const spy = invoke as ReturnType<typeof vi.fn>;
  return spy.mock.calls
    .filter((c) => c[0] === "dispatch_command")
    .map((c) => (c[1] as { cmd?: string } | undefined)?.cmd ?? "");
}

describe("AppShell ai.* commands (plugin-defined, webview-bus executed)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    capturedCommands.clear();
    dispatch = null;
    resetAiCommandsForTest();
    resetWebviewCommandBusForTest();
  });

  it("registers NO ai.* CommandDef in the window-layer command scope", async () => {
    await act(async () => {
      renderShell();
    });
    expect(screen.getByTestId("command-probe-ready")).toBeTruthy();

    // Even mid-stream (when the legacy `available` gate would have surfaced
    // `ai.cancel`), the scope must carry no client-built ai.* definition —
    // the definitions live in the `ai-commands` builtin plugin.
    await act(async () => {
      setAiStreaming(true);
    });
    const aiScopeIds = [...capturedCommands.keys()].filter((id) =>
      id.startsWith("ai."),
    );
    expect(aiScopeIds).toEqual([]);
  });

  it("webview-bus handles the five ai.* ids while mounted, cleaned up on unmount", async () => {
    let result: ReturnType<typeof render>;
    await act(async () => {
      result = renderShell();
    });

    for (const id of AI_IDS) {
      expect(
        hasWebviewCommandHandler(id),
        `${id} must be webview-bus handled while AppShell is mounted`,
      ).toBe(true);
    }

    await act(async () => {
      result!.unmount();
    });
    for (const id of AI_IDS) {
      expect(
        hasWebviewCommandHandler(id),
        `${id} handler must be cleaned up on unmount`,
      ).toBe(false);
    }
  });

  it("dispatching ai.toggle runs the AI module registry handler, no backend call", async () => {
    const toggleFn = vi.fn();
    registerAiCommandHandlers({ toggle: toggleFn });

    await act(async () => {
      renderShell();
    });
    await act(async () => {
      await dispatch!("ai.toggle");
    });

    expect(toggleFn).toHaveBeenCalledTimes(1);
    expect(backendDispatchCmds()).not.toContain("ai.toggle");
  });

  it("ai.model threads args.model through to the module registry", async () => {
    const setModelFn = vi.fn();
    registerAiCommandHandlers({ setModel: setModelFn });

    await act(async () => {
      renderShell();
    });
    await act(async () => {
      await dispatch!("ai.model", { args: { model: "qwen-3" } });
    });

    expect(setModelFn).toHaveBeenCalledWith("qwen-3");
    expect(backendDispatchCmds()).not.toContain("ai.model");
  });

  it("ai.cancel is a no-op when idle and cancels while streaming", async () => {
    const cancelFn = vi.fn();
    registerAiCommandHandlers({ cancel: cancelFn });

    await act(async () => {
      renderShell();
    });

    // Idle (the default): the dispatch-time gate reads `aiStreaming()` and
    // must not reach the registry handler.
    await act(async () => {
      await dispatch!("ai.cancel");
    });
    expect(cancelFn).not.toHaveBeenCalled();

    // Streaming: the same dispatch cancels the in-flight generation.
    await act(async () => {
      setAiStreaming(true);
    });
    await act(async () => {
      await dispatch!("ai.cancel");
    });
    expect(cancelFn).toHaveBeenCalledTimes(1);

    // The turn ends — the gate closes again.
    await act(async () => {
      setAiStreaming(false);
    });
    await act(async () => {
      await dispatch!("ai.cancel");
    });
    expect(cancelFn).toHaveBeenCalledTimes(1);
  });
});
