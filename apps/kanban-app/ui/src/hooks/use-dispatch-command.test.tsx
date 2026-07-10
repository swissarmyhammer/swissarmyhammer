/**
 * Pins the wire shape of `useDispatchCommand`'s backend dispatch path: every
 * command that is NOT handled by a client-side `execute` handler must route
 * through the Command MCP service's `execute command` verb, carrying the
 * captured scope chain + target + args inside `ctx`.
 *
 * The hook itself lives in `lib/command-scope.tsx`; this file lives next to the
 * card's named `useDispatchCommand.test.tsx` so the dispatch contract is found
 * where a reader expects it. The transport seam (`lib/mcp-transport`) is mocked
 * so the assertions are purely about the call shape, not the Tauri crossing.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import type { ReactNode } from "react";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn().mockResolvedValue(undefined),
}));

vi.mock("@/lib/mcp-transport", () => ({
  callCommandTool: vi.fn().mockResolvedValue({ ok: true }),
  subscribeCommandsChanged: vi.fn().mockResolvedValue(() => {}),
  COMMAND_TOOL: "command",
  COMMANDS_CHANGED_EVENT: "notifications/commands/changed",
}));

import {
  CommandScopeProvider,
  ActiveBoardPathProvider,
  useDispatchCommand,
  type CommandDef,
} from "@/lib/command-scope";
import { callCommandTool } from "@/lib/mcp-transport";

const mockCallCommandTool = vi.mocked(callCommandTool);

/** Build a minimal CommandDef. */
function cmd(id: string, overrides: Partial<CommandDef> = {}): CommandDef {
  return { id, name: id, ...overrides };
}

/** Wrapper nesting CommandScopeProviders (innermost first) under a board path. */
function boardWrapper(
  layers: CommandDef[][],
  boardPath: string,
  monikers?: string[],
): ({ children }: { children: ReactNode }) => ReactNode {
  return ({ children }: { children: ReactNode }) => {
    let el = children;
    for (let i = layers.length - 1; i >= 0; i--) {
      el = (
        <CommandScopeProvider commands={layers[i]} moniker={monikers?.[i]}>
          {el}
        </CommandScopeProvider>
      );
    }
    return (
      <ActiveBoardPathProvider value={boardPath}>{el}</ActiveBoardPathProvider>
    );
  };
}

describe("useDispatchCommand → Command service", () => {
  beforeEach(() => {
    mockCallCommandTool.mockClear();
    mockCallCommandTool.mockResolvedValue({ ok: true });
  });

  it('dispatches via tools/call("command", { op: "execute command", id, ctx })', async () => {
    const { result } = renderHook(() => useDispatchCommand(), {
      wrapper: boardWrapper([[]], "/boards/my-board", ["window:main"]),
    });

    await act(async () => {
      await result.current("task.move", { args: { to: "done" } });
    });

    expect(mockCallCommandTool).toHaveBeenCalledWith("execute command", {
      id: "task.move",
      ctx: {
        scope_chain: ["window:main"],
        target: undefined,
        args: { to: "done" },
      },
      board_path: "/boards/my-board",
    });
  });

  it("captures the scope chain innermost-first at call time", async () => {
    const { result } = renderHook(() => useDispatchCommand("test.cmd"), {
      wrapper: boardWrapper([[], [], []], "/boards/nested", [
        "window:board-2",
        "column:todo",
        "task:abc",
      ]),
    });

    await act(async () => {
      await result.current();
    });

    expect(mockCallCommandTool).toHaveBeenCalledWith("execute command", {
      id: "test.cmd",
      ctx: {
        scope_chain: ["task:abc", "column:todo", "window:board-2"],
        target: undefined,
        args: undefined,
      },
      board_path: "/boards/nested",
    });
  });

  it("forwards target and an explicit scopeChain into ctx", async () => {
    const { result } = renderHook(() => useDispatchCommand(), {
      wrapper: boardWrapper([[], []], "/boards/test", [
        "window:main",
        "column:doing",
      ]),
    });

    await act(async () => {
      await result.current("entity.copy", {
        target: "task:abc",
        scopeChain: ["task:abc", "column:todo", "window:main"],
      });
    });

    expect(mockCallCommandTool).toHaveBeenCalledWith("execute command", {
      id: "entity.copy",
      ctx: {
        scope_chain: ["task:abc", "column:todo", "window:main"],
        target: "task:abc",
        args: undefined,
      },
      board_path: "/boards/test",
    });
  });

  it("omits board_path from ctx when no active board path is set", async () => {
    const { result } = renderHook(() => useDispatchCommand("global.cmd"), {
      wrapper: ({ children }: { children: ReactNode }) => (
        <CommandScopeProvider moniker="window:main">
          {children}
        </CommandScopeProvider>
      ),
    });

    await act(async () => {
      await result.current();
    });

    expect(mockCallCommandTool).toHaveBeenCalledWith("execute command", {
      id: "global.cmd",
      ctx: {
        scope_chain: ["window:main"],
        target: undefined,
        args: undefined,
      },
    });
  });

  it("does NOT call the Command service for client-side execute handlers", async () => {
    const executeFn = vi.fn();
    const { result } = renderHook(() => useDispatchCommand(), {
      wrapper: boardWrapper(
        [[cmd("local.action", { execute: executeFn })]],
        "/boards/test",
      ),
    });

    await act(async () => {
      await result.current("local.action");
    });

    expect(executeFn).toHaveBeenCalledOnce();
    expect(mockCallCommandTool).not.toHaveBeenCalled();
  });

  it("returns the Command service result to the caller", async () => {
    mockCallCommandTool.mockResolvedValue({ moved: true });
    const { result } = renderHook(() => useDispatchCommand("task.move"), {
      wrapper: boardWrapper([[]], "/boards/test"),
    });

    let dispatchResult: unknown;
    await act(async () => {
      dispatchResult = await result.current();
    });

    expect(dispatchResult).toEqual({ moved: true });
  });
});
