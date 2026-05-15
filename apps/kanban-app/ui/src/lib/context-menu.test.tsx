import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { useContextMenu } from "./context-menu";
import { CommandScopeProvider } from "./command-scope";
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
    return (
      showCall[1] as {
        items: Array<{ name: string; cmd: string; separator: boolean }>;
      }
    ).items;
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

// ---------------------------------------------------------------------------
// Scope-chain propagation
//
// These tests verify that the scope chain captured at right-click time
// matches the nearest `CommandScopeProvider` ancestor's moniker stack —
// innermost-first. They are the regression guard for the perspective
// context-menu routing bug: when `PerspectivesContainer` wraps the view
// body in `CommandScopeProvider moniker="perspective:<id>"`, every
// right-click below the tab bar must carry that moniker into both the
// `list_commands_for_scope` query and the `show_context_menu` items so
// `useDispatchCommand` on the backend-side can resolve the correct
// perspective.
// ---------------------------------------------------------------------------

describe("useContextMenu scope chain propagation", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  /** Wrap `useContextMenu` in N nested `CommandScopeProvider`s. */
  function makeWrapper(monikers: string[]) {
    // monikers[0] is the innermost — the provider that is rendered
    // *closest* to the hook.
    return ({ children }: { children: React.ReactNode }) => {
      // Build outermost → innermost so React nesting matches the input order.
      let tree: React.ReactNode = children;
      for (const m of monikers) {
        tree = <CommandScopeProvider moniker={m}>{tree}</CommandScopeProvider>;
      }
      return <EntityFocusProvider>{tree}</EntityFocusProvider>;
    };
  }

  it("forwards the nearest-provider moniker to list_commands_for_scope", async () => {
    mockResolvedCommands([
      {
        id: "perspective.clearFilter",
        name: "Clear Filter",
        group: "perspective",
        context_menu: true,
        available: true,
      },
    ]);

    const { result } = renderHook(() => useContextMenu(), {
      // Innermost first: "perspective:p1" is closest to the hook.
      wrapper: makeWrapper(["perspective:p1", "window:main"]),
    });

    await act(async () => {
      result.current(fakeMouseEvent());
      await new Promise((r) => setTimeout(r, 10));
    });

    // list_commands_for_scope should receive the innermost → outermost
    // chain — exactly what the Rust resolver walks.
    expect(invoke).toHaveBeenCalledWith("list_commands_for_scope", {
      scopeChain: ["perspective:p1", "window:main"],
      contextMenu: true,
    });
  });

  it("writes the CommandScopeContext chain into every ContextMenuItem", async () => {
    mockResolvedCommands([
      {
        id: "perspective.clearFilter",
        name: "Clear Filter",
        group: "perspective",
        context_menu: true,
        available: true,
      },
      {
        id: "perspective.clearGroup",
        name: "Clear Group",
        group: "perspective",
        context_menu: true,
        available: true,
      },
    ]);

    const { result } = renderHook(() => useContextMenu(), {
      wrapper: makeWrapper(["perspective:p-active", "window:main"]),
    });

    await act(async () => {
      result.current(fakeMouseEvent());
      await new Promise((r) => setTimeout(r, 10));
    });

    const showCall = (invoke as ReturnType<typeof vi.fn>).mock.calls.find(
      (c: unknown[]) => c[0] === "show_context_menu",
    );
    expect(showCall).toBeDefined();
    const items = showCall![1].items as Array<{
      cmd: string;
      scope_chain: string[];
      separator: boolean;
    }>;

    // Every non-separator item carries the exact scope chain captured
    // at right-click time. This is the contract `handle_menu_event` +
    // the AppShell `context-menu-command` listener rely on.
    const dispatchItems = items.filter((i) => !i.separator);
    expect(dispatchItems.length).toBe(2);
    for (const item of dispatchItems) {
      expect(item.scope_chain).toEqual(["perspective:p-active", "window:main"]);
    }
  });

  it("returned handler is reference-stable across renders", () => {
    // The hook is called on every cell of every row of every grid body —
    // ~14k invocations per 2000-row grid render. A fresh closure per
    // invocation defeats prop-identity memoization downstream. The handler
    // identity must be stable across re-renders so React's skip-children
    // fast path stays effective.
    const { result, rerender } = renderHook(() => useContextMenu(), {
      wrapper: makeWrapper(["perspective:p1", "window:main"]),
    });

    const first = result.current;
    rerender();
    const second = result.current;
    rerender();
    const third = result.current;

    expect(second).toBe(first);
    expect(third).toBe(first);
  });

  it("handler reflects the scope at click time, not at render time", async () => {
    // The handler is memoised with empty deps, so it is created once. But
    // it must still read the *current* scope when the user right-clicks —
    // not the scope from when the handler was first created. This guards
    // the ref-based scope capture against a regression that would freeze
    // the scope chain at mount.
    mockResolvedCommands([
      {
        id: "entity.inspect",
        name: "Inspect",
        group: "entity",
        context_menu: true,
        available: true,
      },
    ]);

    // Use an outer mutable reference so the wrapper can read the current
    // scope chain each render without needing renderHook props plumbing.
    let currentMonikers: string[] = ["moniker:A", "window:main"];
    const DynamicWrapper = ({ children }: { children: React.ReactNode }) => {
      return makeWrapper(currentMonikers)({ children });
    };

    const { result, rerender } = renderHook(() => useContextMenu(), {
      wrapper: DynamicWrapper,
    });

    // Capture the handler under scope A, then re-render under scope B.
    const handler = result.current;
    currentMonikers = ["moniker:B", "window:main"];
    rerender();

    // Same reference survives the re-render.
    expect(result.current).toBe(handler);

    // Now fire the captured handler — it must see scope B, not scope A.
    await act(async () => {
      handler(fakeMouseEvent());
      await new Promise((r) => setTimeout(r, 10));
    });

    expect(invoke).toHaveBeenCalledWith("list_commands_for_scope", {
      scopeChain: ["moniker:B", "window:main"],
      contextMenu: true,
    });

    const showCall = (invoke as ReturnType<typeof vi.fn>).mock.calls.find(
      (c: unknown[]) => c[0] === "show_context_menu",
    );
    expect(showCall).toBeDefined();
    const items = showCall![1].items as Array<{
      cmd: string;
      scope_chain: string[];
      separator: boolean;
    }>;
    const dispatchItems = items.filter((i) => !i.separator);
    expect(dispatchItems.length).toBeGreaterThan(0);
    for (const item of dispatchItems) {
      expect(item.scope_chain).toEqual(["moniker:B", "window:main"]);
    }
  });

  it("captures a deep scope chain (perspective + view + window) verbatim", async () => {
    // Real right-click from a grid cell — the inner cell provider sits
    // under entity providers, perspective provider, view provider, and
    // the window provider.
    mockResolvedCommands([
      {
        id: "perspective.sort.clear",
        name: "Clear Sort",
        group: "perspective",
        context_menu: true,
        available: true,
      },
    ]);

    const chain = [
      "task:01ABC",
      "column:todo",
      "perspective:p1",
      "view:tasks-grid",
      "window:main",
    ];
    const { result } = renderHook(() => useContextMenu(), {
      wrapper: makeWrapper(chain),
    });

    await act(async () => {
      result.current(fakeMouseEvent());
      await new Promise((r) => setTimeout(r, 10));
    });

    expect(invoke).toHaveBeenCalledWith("list_commands_for_scope", {
      scopeChain: chain,
      contextMenu: true,
    });

    const showCall = (invoke as ReturnType<typeof vi.fn>).mock.calls.find(
      (c: unknown[]) => c[0] === "show_context_menu",
    );
    const items = showCall![1].items as Array<{
      cmd: string;
      scope_chain: string[];
      separator: boolean;
    }>;
    const clearSort = items.find((i) => i.cmd === "perspective.sort.clear");
    expect(clearSort).toBeDefined();
    expect(clearSort!.scope_chain).toEqual(chain);
  });
});
