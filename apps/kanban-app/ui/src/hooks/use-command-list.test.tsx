/**
 * `useCommandList` reads the active command registry from the Command MCP
 * service's `list command` verb and re-fetches when the host emits a debounced
 * `commands/changed` notification. These tests mock the transport seam so they
 * assert the verb shape, the scope filter, and the re-render-on-notification
 * behavior without touching Tauri.
 *
 * Real timers are used (the suite runs in real Chromium, where fake timers
 * deadlock `waitFor`); the hook's debounce is short (~100ms) so the assertions
 * just wait for the trailing edge.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, act, waitFor } from "@testing-library/react";

vi.mock("@/lib/mcp-transport", () => ({
  callCommandTool: vi.fn(),
  subscribeCommandsChanged: vi.fn(),
  COMMAND_TOOL: "command",
  COMMANDS_CHANGED_EVENT: "notifications/commands/changed",
}));

import { useCommandList, type CommandMetadata } from "./use-command-list";
import { callCommandTool, subscribeCommandsChanged } from "@/lib/mcp-transport";

const mockCallCommandTool = vi.mocked(callCommandTool);
const mockSubscribe = vi.mocked(subscribeCommandsChanged);

/** Build a minimal CommandMetadata. */
function meta(
  id: string,
  overrides: Partial<CommandMetadata> = {},
): CommandMetadata {
  return { id, name: id, ...overrides };
}

/** Wire callCommandTool to return a given list. */
function resolveListWith(commands: CommandMetadata[]) {
  mockCallCommandTool.mockResolvedValue({ ok: true, commands });
}

describe("useCommandList", () => {
  /** Captures the notification callback the hook registers. */
  let changedCallback: (() => void) | null;

  beforeEach(() => {
    changedCallback = null;
    mockCallCommandTool.mockReset();
    mockSubscribe.mockReset();
    mockSubscribe.mockImplementation((cb: () => void) => {
      changedCallback = cb;
      return Promise.resolve(() => {});
    });
    resolveListWith([]);
  });

  it('fetches via tools/call("command", { op: "list command" }) on mount', async () => {
    resolveListWith([meta("task.move"), meta("task.delete")]);

    const { result } = renderHook(() => useCommandList());

    await waitFor(() =>
      expect(result.current.commands.map((c) => c.id)).toEqual([
        "task.move",
        "task.delete",
      ]),
    );
    expect(mockCallCommandTool).toHaveBeenCalledWith("list command", {});
  });

  it("passes the scope filter to list command", async () => {
    resolveListWith([meta("task.move")]);

    renderHook(() => useCommandList({ scope: "entity:task" }));

    await waitFor(() =>
      expect(mockCallCommandTool).toHaveBeenCalledWith("list command", {
        scope: "entity:task",
      }),
    );
  });

  it("re-fetches and re-renders on a commands/changed notification", async () => {
    resolveListWith([meta("task.move")]);

    const { result } = renderHook(() => useCommandList());

    await waitFor(() =>
      expect(result.current.commands.map((c) => c.id)).toEqual(["task.move"]),
    );
    expect(mockCallCommandTool).toHaveBeenCalledTimes(1);

    // The registry changed on the host — the next list returns a new set.
    resolveListWith([meta("task.move"), meta("task.archive")]);

    await waitFor(() => expect(changedCallback).not.toBeNull());
    act(() => {
      changedCallback?.();
    });

    await waitFor(() =>
      expect(result.current.commands.map((c) => c.id)).toEqual([
        "task.move",
        "task.archive",
      ]),
    );
    expect(mockCallCommandTool).toHaveBeenCalledTimes(2);
  });

  it("debounces a burst of commands/changed into a single re-fetch", async () => {
    resolveListWith([meta("a")]);
    renderHook(() => useCommandList());

    await waitFor(() =>
      expect(mockCallCommandTool).toHaveBeenCalledTimes(1),
    );

    await waitFor(() => expect(changedCallback).not.toBeNull());

    // Three notifications inside one debounce window collapse to one re-fetch.
    act(() => {
      changedCallback?.();
      changedCallback?.();
      changedCallback?.();
    });

    await waitFor(() =>
      expect(mockCallCommandTool).toHaveBeenCalledTimes(2),
    );
    // Give the debounce window time to (not) fire again; count stays at 2.
    await new Promise((r) => setTimeout(r, 150));
    expect(mockCallCommandTool).toHaveBeenCalledTimes(2);
  });

  it("unsubscribes from commands/changed on unmount", async () => {
    const unsub = vi.fn();
    mockSubscribe.mockImplementation((cb: () => void) => {
      changedCallback = cb;
      return Promise.resolve(unsub);
    });
    resolveListWith([]);

    const { unmount } = renderHook(() => useCommandList());
    await waitFor(() => expect(mockSubscribe).toHaveBeenCalled());

    unmount();

    await waitFor(() => expect(unsub).toHaveBeenCalled());
  });
});
