import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { dispatchContextMenuCommand, useContextMenu } from "./context-menu";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ label: "main" }),
}));

/** Helper to create a synthetic MouseEvent with preventDefault/stopPropagation spies. */
function fakeMouseEvent() {
  return {
    preventDefault: vi.fn(),
    stopPropagation: vi.fn(),
  } as unknown as React.MouseEvent;
}

/** Shape matching the backend ResolvedCommand. */
interface ResolvedCommand {
  id: string;
  name: string;
  target?: string;
  group: string;
  context_menu: boolean;
  keys?: { vim?: string; cua?: string; emacs?: string };
  available: boolean;
}

/**
 * Set up invoke mock so that `list_commands_for_scope` returns the given
 * commands, and all other invocations resolve to undefined.
 */
function mockResolvedCommands(commands: ResolvedCommand[]) {
  (invoke as ReturnType<typeof vi.fn>).mockImplementation(
    (cmd: string, _args?: unknown) => {
      if (cmd === "list_commands_for_scope") return Promise.resolve(commands);
      return Promise.resolve(undefined);
    },
  );
}

describe("useContextMenu", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("calls list_commands_for_scope and show_context_menu with resolved items", async () => {
    const commands: ResolvedCommand[] = [
      {
        id: "entity.inspect",
        name: "Inspect Task",
        group: "entity",
        context_menu: true,
        available: true,
      },
      {
        id: "entity.archive",
        name: "Archive Task",
        group: "entity",
        context_menu: true,
        available: true,
      },
    ];
    mockResolvedCommands(commands);

    const scopeChain = ["task:t1"];
    const { result } = renderHook(() => useContextMenu(scopeChain));

    await act(async () => {
      result.current(fakeMouseEvent());
      // Let the promise chain settle
      await new Promise((r) => setTimeout(r, 10));
    });

    expect(invoke).toHaveBeenCalledWith("list_commands_for_scope", {
      scopeChain: ["task:t1"],
      contextMenu: true,
    });
    expect(invoke).toHaveBeenCalledWith("show_context_menu", {
      items: [
        { id: "entity.inspect", name: "Inspect Task" },
        { id: "entity.archive", name: "Archive Task" },
      ],
    });
  });

  it("prevents default and stops propagation", async () => {
    mockResolvedCommands([]);
    const { result } = renderHook(() => useContextMenu(["task:t1"]));

    const event = fakeMouseEvent();
    await act(async () => {
      result.current(event);
    });

    expect(event.preventDefault).toHaveBeenCalled();
    expect(event.stopPropagation).toHaveBeenCalled();
  });

  it("does not call show_context_menu when command list is empty", async () => {
    mockResolvedCommands([]);
    const { result } = renderHook(() => useContextMenu(["task:t1"]));

    await act(async () => {
      result.current(fakeMouseEvent());
      await new Promise((r) => setTimeout(r, 10));
    });

    expect(invoke).toHaveBeenCalledTimes(1); // only list_commands_for_scope
    expect(invoke).not.toHaveBeenCalledWith(
      "show_context_menu",
      expect.anything(),
    );
  });

  it("uses target in the pending key when present", async () => {
    const commands: ResolvedCommand[] = [
      {
        id: "entity.inspect",
        name: "Inspect Task",
        target: "task:t1",
        group: "entity",
        context_menu: true,
        available: true,
      },
    ];
    mockResolvedCommands(commands);

    const { result } = renderHook(() => useContextMenu(["task:t1"]));

    await act(async () => {
      result.current(fakeMouseEvent());
      await new Promise((r) => setTimeout(r, 10));
    });

    expect(invoke).toHaveBeenCalledWith("show_context_menu", {
      items: [{ id: "entity.inspect:task:t1", name: "Inspect Task" }],
    });
  });

  it("inserts separators between different groups", async () => {
    const commands: ResolvedCommand[] = [
      {
        id: "entity.inspect",
        name: "Inspect Task",
        group: "entity",
        context_menu: true,
        available: true,
      },
      {
        id: "task.archive",
        name: "Archive",
        group: "task",
        context_menu: true,
        available: true,
      },
    ];
    mockResolvedCommands(commands);

    const { result } = renderHook(() => useContextMenu(["task:t1"]));

    await act(async () => {
      result.current(fakeMouseEvent());
      await new Promise((r) => setTimeout(r, 10));
    });

    expect(invoke).toHaveBeenCalledWith("show_context_menu", {
      items: [
        { id: "entity.inspect", name: "Inspect Task" },
        { id: "__separator__", name: "" },
        { id: "task.archive", name: "Archive" },
      ],
    });
  });

  it("does not insert a separator when all commands are in the same group", async () => {
    const commands: ResolvedCommand[] = [
      {
        id: "entity.inspect",
        name: "Inspect",
        group: "entity",
        context_menu: true,
        available: true,
      },
      {
        id: "entity.archive",
        name: "Archive",
        group: "entity",
        context_menu: true,
        available: true,
      },
    ];
    mockResolvedCommands(commands);

    const { result } = renderHook(() => useContextMenu(["task:t1"]));

    await act(async () => {
      result.current(fakeMouseEvent());
      await new Promise((r) => setTimeout(r, 10));
    });

    const showCall = (invoke as ReturnType<typeof vi.fn>).mock.calls.find(
      (c: unknown[]) => c[0] === "show_context_menu",
    );
    expect(showCall).toBeDefined();
    const items: Array<{ id: string }> = showCall![1].items;
    expect(items.some((item) => item.id === "__separator__")).toBe(false);
  });
});

describe("dispatchContextMenuCommand", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("dispatches to Rust without windowLabel", async () => {
    const commands: ResolvedCommand[] = [
      {
        id: "task.archive",
        name: "Archive",
        group: "task",
        context_menu: true,
        available: true,
      },
    ];
    mockResolvedCommands(commands);

    const { result } = renderHook(() => useContextMenu(["task:t1"]));

    // Open context menu to populate pendingCommands
    await act(async () => {
      result.current(fakeMouseEvent());
      await new Promise((r) => setTimeout(r, 10));
    });

    vi.clearAllMocks();
    (invoke as ReturnType<typeof vi.fn>).mockResolvedValue(undefined);

    const dispatched = await dispatchContextMenuCommand("task.archive");
    expect(dispatched).toBe(true);
    expect(invoke).toHaveBeenCalledWith("dispatch_command", {
      cmd: "task.archive",
      target: undefined,
      scopeChain: ["task:t1"],
    });
  });

  it("returns false for unknown id", async () => {
    const result = await dispatchContextMenuCommand("nonexistent");
    expect(result).toBe(false);
  });

  it("handlers are cleared on each context menu open", async () => {
    // First open registers "task.archive"
    mockResolvedCommands([
      {
        id: "task.archive",
        name: "Archive",
        group: "task",
        context_menu: true,
        available: true,
      },
    ]);
    const { result: r1, unmount } = renderHook(() =>
      useContextMenu(["task:t1"]),
    );
    await act(async () => {
      r1.current(fakeMouseEvent());
      await new Promise((r) => setTimeout(r, 10));
    });
    unmount();

    // Second open registers "task.delete" and clears "task.archive"
    mockResolvedCommands([
      {
        id: "task.delete",
        name: "Delete",
        group: "task",
        context_menu: true,
        available: true,
      },
    ]);
    const { result: r2 } = renderHook(() => useContextMenu(["task:t2"]));
    await act(async () => {
      r2.current(fakeMouseEvent());
      await new Promise((r) => setTimeout(r, 10));
    });

    // Old handler should be gone
    const dispatched = await dispatchContextMenuCommand("task.archive");
    expect(dispatched).toBe(false);
  });

  it("does not add separator IDs to pending commands", async () => {
    const commands: ResolvedCommand[] = [
      {
        id: "entity.inspect",
        name: "Inspect",
        group: "entity",
        context_menu: true,
        available: true,
      },
      {
        id: "task.archive",
        name: "Archive",
        group: "task",
        context_menu: true,
        available: true,
      },
    ];
    mockResolvedCommands(commands);

    const { result } = renderHook(() => useContextMenu(["task:t1"]));
    await act(async () => {
      result.current(fakeMouseEvent());
      await new Promise((r) => setTimeout(r, 10));
    });

    const dispatched = await dispatchContextMenuCommand("__separator__");
    expect(dispatched).toBe(false);
  });
});
