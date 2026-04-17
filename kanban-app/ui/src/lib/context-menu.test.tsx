import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { useContextMenu } from "./context-menu";
import { EntityFocusProvider } from "./entity-focus-context";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ label: "main" }),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
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

function mockResolvedCommands(commands: ResolvedCommand[]) {
  (invoke as ReturnType<typeof vi.fn>).mockImplementation(
    (cmd: string, _args?: unknown) => {
      if (cmd === "list_commands_for_scope") return Promise.resolve(commands);
      return Promise.resolve(undefined);
    },
  );
}

const wrapper = ({ children }: { children: React.ReactNode }) => (
  <EntityFocusProvider>{children}</EntityFocusProvider>
);

describe("useContextMenu", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("calls list_commands_for_scope and show_context_menu with self-contained items", async () => {
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

    const { result } = renderHook(() => useContextMenu(), { wrapper });

    await act(async () => {
      result.current(fakeMouseEvent());
      await new Promise((r) => setTimeout(r, 10));
    });

    expect(invoke).toHaveBeenCalledWith("list_commands_for_scope", {
      scopeChain: [],
      contextMenu: true,
    });
    // Items carry full dispatch info — cmd, scope_chain, separator flag
    expect(invoke).toHaveBeenCalledWith("show_context_menu", {
      items: [
        {
          name: "Inspect Task",
          cmd: "entity.inspect",
          scope_chain: [],
          separator: false,
        },
        {
          name: "Archive Task",
          cmd: "entity.archive",
          scope_chain: [],
          separator: false,
        },
      ],
    });
  });

  it("prevents default and stops propagation", async () => {
    mockResolvedCommands([]);
    const { result } = renderHook(() => useContextMenu(), { wrapper });

    const event = fakeMouseEvent();
    await act(async () => {
      result.current(event);
    });

    expect(event.preventDefault).toHaveBeenCalled();
    expect(event.stopPropagation).toHaveBeenCalled();
  });

  it("does not call show_context_menu when command list is empty", async () => {
    mockResolvedCommands([]);
    const { result } = renderHook(() => useContextMenu(), { wrapper });

    await act(async () => {
      result.current(fakeMouseEvent());
      await new Promise((r) => setTimeout(r, 10));
    });

    expect(invoke).toHaveBeenCalledTimes(1);
    expect(invoke).not.toHaveBeenCalledWith(
      "show_context_menu",
      expect.anything(),
    );
  });

  it("includes target in the menu item when present", async () => {
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

    const { result } = renderHook(() => useContextMenu(), { wrapper });

    await act(async () => {
      result.current(fakeMouseEvent());
      await new Promise((r) => setTimeout(r, 10));
    });

    expect(invoke).toHaveBeenCalledWith("show_context_menu", {
      items: [
        {
          name: "Inspect Task",
          cmd: "entity.inspect",
          target: "task:t1",
          scope_chain: [],
          separator: false,
        },
      ],
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

    const { result } = renderHook(() => useContextMenu(), { wrapper });

    await act(async () => {
      result.current(fakeMouseEvent());
      await new Promise((r) => setTimeout(r, 10));
    });

    expect(invoke).toHaveBeenCalledWith("show_context_menu", {
      items: [
        {
          name: "Inspect Task",
          cmd: "entity.inspect",
          scope_chain: [],
          separator: false,
        },
        { name: "", cmd: "", scope_chain: [], separator: true },
        {
          name: "Archive",
          cmd: "task.archive",
          scope_chain: [],
          separator: false,
        },
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

    const { result } = renderHook(() => useContextMenu(), { wrapper });

    await act(async () => {
      result.current(fakeMouseEvent());
      await new Promise((r) => setTimeout(r, 10));
    });

    const showCall = (invoke as ReturnType<typeof vi.fn>).mock.calls.find(
      (c: unknown[]) => c[0] === "show_context_menu",
    );
    expect(showCall).toBeDefined();
    const items = showCall![1].items as Array<{ separator: boolean }>;
    expect(items.some((item) => item.separator)).toBe(false);
  });
});

// ---------------------------------------------------------------------------
// Per-entity-type context-menu rendering tests (section 6 — MANDATORY).
//
// One test per entity type, each independently named. Each test:
//   1. Mocks `list_commands_for_scope` to return the exact payload the
//      real Rust emission produces for the grid's scope chain.
//   2. Fires the context-menu handler (right-click) returned by
//      `useContextMenu`.
//   3. Asserts the `show_context_menu` payload includes the
//      `entity.add:{type}` item with the correct display name.
//
// A regression that drops the dynamic command, mangles its id, or filters
// it out of the context menu fails here as a single named failure, not a
// parameterised one.
// ---------------------------------------------------------------------------

describe("useContextMenu per-entity-type rendering", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  /**
   * Drives the context-menu hook through its async invoke chain and
   * returns the items passed to `show_context_menu`.
   */
  async function captureMenuItems(
    cmds: ResolvedCommand[],
  ): Promise<Array<{ name: string; cmd: string; separator: boolean }>> {
    mockResolvedCommands(cmds);
    const { result } = renderHook(() => useContextMenu(), { wrapper });

    await act(async () => {
      result.current(fakeMouseEvent());
      await new Promise((r) => setTimeout(r, 10));
    });

    const showCall = (invoke as ReturnType<typeof vi.fn>).mock.calls.find(
      (c: unknown[]) => c[0] === "show_context_menu",
    );
    if (!showCall) return [];
    return (showCall[1] as { items: Array<{ name: string; cmd: string; separator: boolean }> })
      .items;
  }

  it('right-click on tasks grid shows "New Task" in context menu', async () => {
    // Exactly what `list_commands_for_scope` returns when the active view
    // is tasks-grid and `context_menu: true` is requested.
    const items = await captureMenuItems([
      {
        id: "entity.add:task",
        name: "New Task",
        group: "entity",
        context_menu: true,
        available: true,
      },
    ]);
    const newTask = items.find((i) => i.cmd === "entity.add:task");
    expect(newTask).toBeDefined();
    expect(newTask!.name).toBe("New Task");
    expect(newTask!.separator).toBe(false);
  });

  it('right-click on tags grid shows "New Tag" in context menu', async () => {
    const items = await captureMenuItems([
      {
        id: "entity.add:tag",
        name: "New Tag",
        group: "entity",
        context_menu: true,
        available: true,
      },
    ]);
    const newTag = items.find((i) => i.cmd === "entity.add:tag");
    expect(newTag).toBeDefined();
    expect(newTag!.name).toBe("New Tag");
    expect(newTag!.separator).toBe(false);
  });

  it('right-click on projects grid shows "New Project" in context menu', async () => {
    const items = await captureMenuItems([
      {
        id: "entity.add:project",
        name: "New Project",
        group: "entity",
        context_menu: true,
        available: true,
      },
    ]);
    const newProject = items.find((i) => i.cmd === "entity.add:project");
    expect(newProject).toBeDefined();
    expect(newProject!.name).toBe("New Project");
    expect(newProject!.separator).toBe(false);
  });
});
