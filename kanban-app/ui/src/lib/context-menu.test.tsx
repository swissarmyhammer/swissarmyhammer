import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { CommandScopeProvider, type CommandDef } from "@/lib/command-scope";
import { useContextMenu, dispatchContextMenuCommand } from "./context-menu";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ label: "main" }),
}));

/** Wraps children in a CommandScopeProvider with the given commands. */
function wrapper(commands: CommandDef[]) {
  return ({ children }: { children: React.ReactNode }) => (
    <CommandScopeProvider commands={commands}>{children}</CommandScopeProvider>
  );
}

/** Helper to create a synthetic MouseEvent with preventDefault/stopPropagation spies. */
function fakeMouseEvent() {
  return {
    preventDefault: vi.fn(),
    stopPropagation: vi.fn(),
  } as unknown as React.MouseEvent;
}

describe("useContextMenu", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    (invoke as ReturnType<typeof vi.fn>).mockResolvedValue(undefined);
  });

  it("collects only contextMenu:true commands", () => {
    const commands: CommandDef[] = [
      { id: "a", name: "A", contextMenu: true, execute: vi.fn() },
      { id: "b", name: "B", execute: vi.fn() },
      { id: "c", name: "C", contextMenu: true, execute: vi.fn() },
    ];
    const { result } = renderHook(() => useContextMenu(), {
      wrapper: wrapper(commands),
    });

    result.current(fakeMouseEvent());

    expect(invoke).toHaveBeenCalledWith("show_context_menu", {
      items: [
        { id: "a", name: "A" },
        { id: "c", name: "C" },
      ],
    });
  });

  it("sends correct items to show_context_menu", () => {
    const commands: CommandDef[] = [
      {
        id: "entity.inspect",
        name: "Inspect Task",
        contextMenu: true,
        execute: vi.fn(),
      },
    ];
    const { result } = renderHook(() => useContextMenu(), {
      wrapper: wrapper(commands),
    });

    result.current(fakeMouseEvent());

    expect(invoke).toHaveBeenCalledWith("show_context_menu", {
      items: [{ id: "entity.inspect", name: "Inspect Task" }],
    });
  });

  it("prevents default and stops propagation", () => {
    const commands: CommandDef[] = [
      { id: "a", name: "A", contextMenu: true, execute: vi.fn() },
    ];
    const { result } = renderHook(() => useContextMenu(), {
      wrapper: wrapper(commands),
    });

    const event = fakeMouseEvent();
    result.current(event);

    expect(event.preventDefault).toHaveBeenCalled();
    expect(event.stopPropagation).toHaveBeenCalled();
  });

  it("does not call invoke when no contextMenu commands", () => {
    const commands: CommandDef[] = [{ id: "a", name: "A", execute: vi.fn() }];
    const { result } = renderHook(() => useContextMenu(), {
      wrapper: wrapper(commands),
    });

    result.current(fakeMouseEvent());

    expect(invoke).not.toHaveBeenCalled();
  });
});

describe("dispatchContextMenuCommand", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    (invoke as ReturnType<typeof vi.fn>).mockResolvedValue(undefined);
  });

  it("executes correct handler", async () => {
    const execute = vi.fn();
    const commands: CommandDef[] = [
      { id: "entity.inspect", name: "Inspect", contextMenu: true, execute },
    ];

    // Open context menu to register handlers in the pendingHandlers map
    const { result } = renderHook(() => useContextMenu(), {
      wrapper: wrapper(commands),
    });
    result.current(fakeMouseEvent());

    const dispatched = await dispatchContextMenuCommand("entity.inspect");
    expect(dispatched).toBe(true);
    expect(execute).toHaveBeenCalled();
  });

  it("dispatches to Rust by id when no execute is set", async () => {
    const commands: CommandDef[] = [
      {
        id: "task.untag",
        name: "Remove",
        contextMenu: true,
        args: { id: "t1", tag: "bug" },
      },
    ];

    const { result } = renderHook(() => useContextMenu(), {
      wrapper: wrapper(commands),
    });
    result.current(fakeMouseEvent());

    await dispatchContextMenuCommand("task.untag");
    expect(invoke).toHaveBeenCalledWith("dispatch_command", {
      cmd: "task.untag",
      target: undefined,
      args: { id: "t1", tag: "bug" },
      windowLabel: "main",
    });
  });

  it("returns false for unknown id", async () => {
    const result = await dispatchContextMenuCommand("nonexistent");
    expect(result).toBe(false);
  });

  it("handlers are cleared on each context menu open", async () => {
    const exec1 = vi.fn();
    const exec2 = vi.fn();

    // First open registers handler "a"
    const { result: r1, unmount } = renderHook(() => useContextMenu(), {
      wrapper: wrapper([
        { id: "a", name: "A", contextMenu: true, execute: exec1 },
      ]),
    });
    r1.current(fakeMouseEvent());
    unmount();

    // Second open registers handler "b" and clears "a"
    const { result: r2 } = renderHook(() => useContextMenu(), {
      wrapper: wrapper([
        { id: "b", name: "B", contextMenu: true, execute: exec2 },
      ]),
    });
    r2.current(fakeMouseEvent());

    // Old handler should be gone
    const dispatched = await dispatchContextMenuCommand("a");
    expect(dispatched).toBe(false);
  });
});
