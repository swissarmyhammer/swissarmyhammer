/**
 * Context-menu wiring via the metadata-driven Command registry (card
 * `01KS36XGKCQ36QM7P6MH3FHMBJ`).
 *
 * `useContextMenu` sources commands from `useCommandList` and, at right-click
 * time, surfaces only those flagged `context_menu: true` whose `scope` matches
 * the right-click point's scope chain. These tests mock the list seam and the
 * `window` MCP transport and assert that exactly the `context_menu`-tagged,
 * scope-matched commands reach `show context menu` — global-scoped commands
 * included, non-context-menu and out-of-scope commands excluded.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { callMcpTool } from "@/lib/mcp-transport";
import type { CommandMetadata } from "@/hooks/use-command-list";

vi.mock("@/lib/mcp-transport", async (importActual) => ({
  // Preserve the real module's other exports (e.g. `callCommandTool`, consumed
  // transitively by `command-scope`); only `callMcpTool` is stubbed.
  ...(await importActual<typeof import("@/lib/mcp-transport")>()),
  callMcpTool: vi.fn(),
}));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ label: "main" }),
}));

// Drive the command source. The registry is set per-test via REGISTRY.
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

/** Build a single-node scope carrying `moniker` so the chain = [moniker]. */
function scopeWithMoniker(moniker: string): CommandScope {
  return { commands: new Map(), parent: null, moniker };
}

function wrapperFor(moniker: string) {
  const scope = scopeWithMoniker(moniker);
  return ({ children }: { children: React.ReactNode }) => (
    <CommandScopeContext.Provider value={scope}>
      {children}
    </CommandScopeContext.Provider>
  );
}

/** Pull the `cmd` ids of the items passed to `show context menu`. */
function shownItemCmds(): string[] {
  const call = (callMcpTool as ReturnType<typeof vi.fn>).mock.calls.find(
    ([tool, op]) => tool === "window" && op === "show context menu",
  );
  if (!call) return [];
  const items = (call[2] as { items: { cmd: string; separator: boolean }[] })
    .items;
  return items.filter((i) => !i.separator).map((i) => i.cmd);
}

describe("useContextMenu registry wiring", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    (callMcpTool as ReturnType<typeof vi.fn>).mockResolvedValue(undefined);
  });

  it("shows only context_menu:true commands matching the task scope", async () => {
    REGISTRY = [
      {
        id: "task.inspect",
        name: "Inspect",
        context_menu: true,
        scope: ["entity:task"],
      },
      {
        id: "task.untag",
        name: "Untag",
        context_menu: true,
        scope: ["entity:task"],
      },
      // context_menu false — must not appear.
      {
        id: "task.move",
        name: "Move",
        context_menu: false,
        scope: ["entity:task"],
      },
      // Wrong scope — must not appear.
      {
        id: "tag.rename",
        name: "Rename Tag",
        context_menu: true,
        scope: ["entity:tag"],
      },
      // Global (no scope) context-menu command — appears everywhere.
      { id: "app.help", name: "Help", context_menu: true },
    ];

    const { result } = renderHook(() => useContextMenu(), {
      wrapper: wrapperFor("entity:task"),
    });

    await act(async () => {
      result.current(fakeMouseEvent());
      await new Promise((r) => setTimeout(r, 10));
    });

    expect(shownItemCmds()).toEqual(["task.inspect", "task.untag", "app.help"]);
    // The non-context-menu and wrong-scope commands are absent.
    expect(shownItemCmds()).not.toContain("task.move");
    expect(shownItemCmds()).not.toContain("tag.rename");
  });

  it("does not call show context menu when nothing matches", async () => {
    REGISTRY = [
      {
        id: "tag.rename",
        name: "Rename Tag",
        context_menu: true,
        scope: ["entity:tag"],
      },
    ];

    const { result } = renderHook(() => useContextMenu(), {
      wrapper: wrapperFor("entity:task"),
    });

    await act(async () => {
      result.current(fakeMouseEvent());
      await new Promise((r) => setTimeout(r, 10));
    });

    const called = (callMcpTool as ReturnType<typeof vi.fn>).mock.calls.some(
      ([tool, op]) => tool === "window" && op === "show context menu",
    );
    expect(called).toBe(false);
  });
});
