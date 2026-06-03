/**
 * `useContextMenu` sources commands from the metadata-driven Command registry
 * (`useCommandList`) and, at right-click time, surfaces only those flagged
 * `context_menu: true` whose `scope` matches the right-click point's scope
 * chain. These tests mock the list seam and the `window` MCP transport and
 * assert the `show context menu` payload shape: self-contained items carrying the
 * click-time scope chain, separators between `context_menu_group` buckets, and
 * the reference-stable / read-at-click-time handler contract.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { callMcpTool } from "@/lib/mcp-transport";
import type { CommandMetadata } from "@/hooks/use-command-list";

vi.mock("@/lib/mcp-transport", async (importActual) => ({
  // Keep the real module's other exports (e.g. `callCommandTool`, consumed by
  // `command-scope`) intact; only `callMcpTool` is stubbed so the test can
  // assert the `show context menu` payload without a live transport.
  ...(await importActual<typeof import("@/lib/mcp-transport")>()),
  callMcpTool: vi.fn(),
}));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ label: "main" }),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

// The command source. Set per-test via REGISTRY.
let REGISTRY: CommandMetadata[] = [];
vi.mock("@/hooks/use-command-list", () => ({
  useCommandList: () => ({
    commands: REGISTRY,
    loading: false,
    refresh: vi.fn(),
  }),
}));

import { useContextMenu } from "./context-menu";
import { CommandScopeContext, type CommandScope } from "./command-scope";

/** Synthetic right-click event with spied handlers. */
function fakeMouseEvent() {
  return {
    preventDefault: vi.fn(),
    stopPropagation: vi.fn(),
  } as unknown as React.MouseEvent;
}

/** Build a linked scope chain (innermost-first) from monikers. */
function buildScope(monikers: string[]): CommandScope | null {
  let scope: CommandScope | null = null;
  for (let i = monikers.length - 1; i >= 0; i--) {
    scope = { commands: new Map(), parent: scope, moniker: monikers[i] };
  }
  return scope;
}

/** Wrap the hook in a CommandScopeContext carrying the given chain. */
function wrapperFor(monikers: string[]) {
  const scope = buildScope(monikers);
  return ({ children }: { children: React.ReactNode }) => (
    <CommandScopeContext.Provider value={scope}>
      {children}
    </CommandScopeContext.Provider>
  );
}

/** One item in the `show context menu` payload. */
interface ShownItem {
  name: string;
  cmd: string;
  target?: string;
  scope_chain: string[];
  separator: boolean;
}

/**
 * Items handed to the `window` server's `show context menu` op, or null if it
 * was not called. The hook calls
 * `callMcpTool("window", "show context menu", { items, window_label })`, so the
 * items ride in the third argument's `items` field.
 */
function shownItems(): ShownItem[] | null {
  const call = (callMcpTool as ReturnType<typeof vi.fn>).mock.calls.find(
    ([tool, op]) => tool === "window" && op === "show context menu",
  );
  if (!call) return null;
  return (call[2] as { items: ShownItem[] }).items;
}

async function fireContextMenu(monikers: string[]) {
  const { result } = renderHook(() => useContextMenu(), {
    wrapper: wrapperFor(monikers),
  });
  await act(async () => {
    result.current(fakeMouseEvent());
    await new Promise((r) => setTimeout(r, 10));
  });
  return result;
}

describe("useContextMenu", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    (callMcpTool as ReturnType<typeof vi.fn>).mockResolvedValue(undefined);
    REGISTRY = [];
  });

  it("sends self-contained items for matching context_menu commands", async () => {
    REGISTRY = [
      {
        id: "entity.inspect",
        name: "Inspect Task",
        context_menu: true,
        scope: ["entity:task"],
      },
      {
        id: "entity.archive",
        name: "Archive Task",
        context_menu: true,
        scope: ["entity:task"],
      },
    ];

    await fireContextMenu(["entity:task"]);

    expect(callMcpTool).toHaveBeenCalledWith("window", "show context menu", {
      // The calling window's label rides alongside the items so the shell can
      // pop the menu on this window deterministically (mocked to "main").
      window_label: "main",
      items: [
        {
          name: "Inspect Task",
          cmd: "entity.inspect",
          target: "entity:task",
          scope_chain: ["entity:task"],
          separator: false,
        },
        {
          name: "Archive Task",
          cmd: "entity.archive",
          target: "entity:task",
          scope_chain: ["entity:task"],
          separator: false,
        },
      ],
    });
  });

  it("prevents default and stops propagation", async () => {
    const { result } = renderHook(() => useContextMenu(), {
      wrapper: wrapperFor([]),
    });
    const event = fakeMouseEvent();
    await act(async () => {
      result.current(event);
    });
    expect(event.preventDefault).toHaveBeenCalled();
    expect(event.stopPropagation).toHaveBeenCalled();
  });

  it("does not call show context menu when nothing matches", async () => {
    REGISTRY = [];
    await fireContextMenu(["entity:task"]);
    expect(callMcpTool).not.toHaveBeenCalledWith(
      "window",
      "show context menu",
      expect.anything(),
    );
  });

  it("uses menu_name override for the item label when present", async () => {
    REGISTRY = [
      {
        id: "entity.inspect",
        name: "Inspect",
        menu_name: "Inspect Task…",
        context_menu: true,
        scope: ["entity:task"],
      },
    ];
    await fireContextMenu(["entity:task"]);
    const items = shownItems();
    expect(items?.[0].name).toBe("Inspect Task…");
  });

  it("inserts separators between different context_menu_group buckets", async () => {
    REGISTRY = [
      {
        id: "entity.inspect",
        name: "Inspect",
        context_menu: true,
        context_menu_group: 0,
        scope: ["entity:task"],
      },
      {
        id: "task.archive",
        name: "Archive",
        context_menu: true,
        context_menu_group: 1,
        scope: ["entity:task"],
      },
    ];
    await fireContextMenu(["entity:task"]);

    const items = shownItems();
    expect(items).toEqual([
      {
        name: "Inspect",
        cmd: "entity.inspect",
        target: "entity:task",
        scope_chain: ["entity:task"],
        separator: false,
      },
      { name: "", cmd: "", scope_chain: [], separator: true },
      {
        name: "Archive",
        cmd: "task.archive",
        target: "entity:task",
        scope_chain: ["entity:task"],
        separator: false,
      },
    ]);
  });

  it("does not insert a separator when all commands share a group", async () => {
    REGISTRY = [
      {
        id: "entity.inspect",
        name: "Inspect",
        context_menu: true,
        context_menu_group: 0,
        scope: ["entity:task"],
      },
      {
        id: "entity.archive",
        name: "Archive",
        context_menu: true,
        context_menu_group: 0,
        scope: ["entity:task"],
      },
    ];
    await fireContextMenu(["entity:task"]);
    const items = shownItems();
    expect(items?.some((i) => i.separator)).toBe(false);
  });
});

// ---------------------------------------------------------------------------
// Per-entity-type context-menu rendering — one test per entity type.
// ---------------------------------------------------------------------------

describe("useContextMenu per-entity-type rendering", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    (callMcpTool as ReturnType<typeof vi.fn>).mockResolvedValue(undefined);
  });

  it('right-click on a task shows "New Task"', async () => {
    REGISTRY = [
      {
        id: "entity.add:task",
        name: "New Task",
        context_menu: true,
        scope: ["entity:task"],
      },
    ];
    await fireContextMenu(["entity:task"]);
    expect(shownItems()?.find((i) => i.cmd === "entity.add:task")?.name).toBe(
      "New Task",
    );
  });

  it('right-click on a tag shows "New Tag"', async () => {
    REGISTRY = [
      {
        id: "entity.add:tag",
        name: "New Tag",
        context_menu: true,
        scope: ["entity:tag"],
      },
    ];
    await fireContextMenu(["entity:tag"]);
    expect(shownItems()?.find((i) => i.cmd === "entity.add:tag")?.name).toBe(
      "New Tag",
    );
  });

  it('right-click on a project shows "New Project"', async () => {
    REGISTRY = [
      {
        id: "entity.add:project",
        name: "New Project",
        context_menu: true,
        scope: ["entity:project"],
      },
    ];
    await fireContextMenu(["entity:project"]);
    expect(
      shownItems()?.find((i) => i.cmd === "entity.add:project")?.name,
    ).toBe("New Project");
  });
});

// ---------------------------------------------------------------------------
// Scope-chain propagation + handler-identity contract.
// ---------------------------------------------------------------------------

describe("useContextMenu scope chain propagation", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    (callMcpTool as ReturnType<typeof vi.fn>).mockResolvedValue(undefined);
  });

  it("writes the click-time scope chain into every dispatch item", async () => {
    REGISTRY = [
      {
        id: "perspective.clearFilter",
        name: "Clear Filter",
        context_menu: true,
        scope: ["perspective:p-active"],
      },
      {
        id: "perspective.clearGroup",
        name: "Clear Group",
        context_menu: true,
        scope: ["perspective:p-active"],
      },
    ];

    await fireContextMenu(["perspective:p-active", "window:main"]);

    const dispatchItems = shownItems()!.filter((i) => !i.separator);
    expect(dispatchItems.length).toBe(2);
    for (const item of dispatchItems) {
      expect(item.scope_chain).toEqual(["perspective:p-active", "window:main"]);
    }
  });

  it("returned handler is reference-stable across renders", () => {
    REGISTRY = [];
    const { result, rerender } = renderHook(() => useContextMenu(), {
      wrapper: wrapperFor(["perspective:p1", "window:main"]),
    });
    const first = result.current;
    rerender();
    rerender();
    expect(result.current).toBe(first);
  });

  it("handler reflects the scope at click time, not render time", async () => {
    REGISTRY = [
      {
        id: "entity.inspect",
        name: "Inspect",
        context_menu: true,
        scope: ["moniker:B"],
      },
    ];

    let currentMonikers: string[] = ["moniker:A", "window:main"];
    const DynamicWrapper = ({ children }: { children: React.ReactNode }) => (
      <CommandScopeContext.Provider value={buildScope(currentMonikers)}>
        {children}
      </CommandScopeContext.Provider>
    );

    const { result, rerender } = renderHook(() => useContextMenu(), {
      wrapper: DynamicWrapper,
    });

    const handler = result.current;
    currentMonikers = ["moniker:B", "window:main"];
    rerender();
    expect(result.current).toBe(handler);

    await act(async () => {
      handler(fakeMouseEvent());
      await new Promise((r) => setTimeout(r, 10));
    });

    const dispatchItems = shownItems()!.filter((i) => !i.separator);
    expect(dispatchItems.length).toBeGreaterThan(0);
    for (const item of dispatchItems) {
      expect(item.scope_chain).toEqual(["moniker:B", "window:main"]);
    }
  });
});
