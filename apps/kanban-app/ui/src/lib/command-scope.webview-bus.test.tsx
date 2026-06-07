/**
 * Dispatch-integration tests for the webview command bus
 * (`webview-command-bus.ts`) wired into `useDispatchCommand`
 * (`command-scope.tsx`).
 *
 * These pin the routing contract Card B introduces: a command id with a
 * registered webview handler is dispatched to that handler and short-circuits
 * the backend `execute command`; an id with no registered handler falls
 * through to the backend dispatch unchanged. The registered handler receives
 * the same `DispatchOptions` the dispatcher was called with.
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import type { ReactNode } from "react";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ label: "main" }),
}));

vi.mock("@/lib/mcp-transport", () => ({
  callCommandTool: vi.fn().mockResolvedValue({ ok: true }),
  subscribeCommandsChanged: vi.fn().mockResolvedValue(() => {}),
  COMMAND_TOOL: "command",
  COMMANDS_CHANGED_EVENT: "notifications/commands/changed",
}));

import {
  ActiveBoardPathProvider,
  CommandScopeProvider,
  useDispatchCommand,
  type CommandDef,
} from "./command-scope";
import {
  registerWebviewCommandHandler,
  resetWebviewCommandBusForTest,
} from "./webview-command-bus";
import { callCommandTool } from "@/lib/mcp-transport";

const mockCallCommandTool = vi.mocked(callCommandTool);

beforeEach(() => {
  mockCallCommandTool.mockReset();
  mockCallCommandTool.mockResolvedValue({ ok: true });
  resetWebviewCommandBusForTest();
});

/** Wrapper providing a board path + a (possibly empty) command scope. */
function boardWrapper(
  layers: CommandDef[][] = [[]],
  boardPath = "/boards/test",
): ({ children }: { children: ReactNode }) => ReactNode {
  return ({ children }: { children: ReactNode }) => {
    let el = children;
    for (let i = layers.length - 1; i >= 0; i--) {
      el = (
        <CommandScopeProvider commands={layers[i]}>{el}</CommandScopeProvider>
      );
    }
    return (
      <ActiveBoardPathProvider value={boardPath}>{el}</ActiveBoardPathProvider>
    );
  };
}

describe("useDispatchCommand + webview command bus", () => {
  it("dispatches to a registered webview handler instead of the backend", async () => {
    const handler = vi.fn();
    registerWebviewCommandHandler("overlay.open", handler);

    const { result } = renderHook(() => useDispatchCommand(), {
      wrapper: boardWrapper(),
    });

    await act(async () => {
      await result.current("overlay.open", { args: { which: "jump" } });
    });

    expect(handler).toHaveBeenCalledOnce();
    // The handler receives the dispatch options.
    expect(handler).toHaveBeenCalledWith({ args: { which: "jump" } });
    // The backend must NOT be invoked for a webview-handled id.
    expect(mockCallCommandTool).not.toHaveBeenCalled();
  });

  it("falls through to backend dispatch when no webview handler is registered", async () => {
    const { result } = renderHook(() => useDispatchCommand(), {
      wrapper: boardWrapper(),
    });

    await act(async () => {
      await result.current("task.move", { target: "task:abc" });
    });

    expect(mockCallCommandTool).toHaveBeenCalledWith("execute command", {
      id: "task.move",
      ctx: {
        scope_chain: [],
        target: "task:abc",
        args: undefined,
      },
      board_path: "/boards/test",
    });
  });

  it("returns the webview handler's resolved value", async () => {
    registerWebviewCommandHandler("overlay.open", async () => "opened");

    const { result } = renderHook(() => useDispatchCommand(), {
      wrapper: boardWrapper(),
    });

    let returned: unknown;
    await act(async () => {
      returned = await result.current("overlay.open");
    });

    expect(returned).toBe("opened");
    expect(mockCallCommandTool).not.toHaveBeenCalled();
  });

  it("after the handler is unregistered, the id falls back to the backend", async () => {
    const handler = vi.fn();
    const cleanup = registerWebviewCommandHandler("overlay.open", handler);
    cleanup();

    const { result } = renderHook(() => useDispatchCommand(), {
      wrapper: boardWrapper(),
    });

    await act(async () => {
      await result.current("overlay.open");
    });

    expect(handler).not.toHaveBeenCalled();
    expect(mockCallCommandTool).toHaveBeenCalledWith(
      "execute command",
      expect.objectContaining({ id: "overlay.open" }),
    );
  });

  it("a scope execute handler still wins over the bus (fast-path untouched)", async () => {
    const executeFn = vi.fn();
    const busHandler = vi.fn();
    registerWebviewCommandHandler("local.action", busHandler);

    const { result } = renderHook(() => useDispatchCommand(), {
      wrapper: boardWrapper([
        [{ id: "local.action", name: "local.action", execute: executeFn }],
      ]),
    });

    await act(async () => {
      await result.current("local.action");
    });

    // The existing scope `execute` fast-path is checked first and wins.
    expect(executeFn).toHaveBeenCalledOnce();
    expect(busHandler).not.toHaveBeenCalled();
    expect(mockCallCommandTool).not.toHaveBeenCalled();
  });
});
