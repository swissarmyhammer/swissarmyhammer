import { describe, it, expect, vi } from "vitest";
import { renderHook, act } from "@testing-library/react";
import type { ReactNode } from "react";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ label: "main" }),
}));

import {
  CommandScopeProvider,
  resolveCommand,
  useAvailableCommands,
  collectAvailableCommands,
  useExecuteCommand,
  dispatchCommand,
  type CommandDef,
  type CommandScope,
} from "./command-scope";

/* ---------- helpers ---------- */

/** Build a CommandScope value directly (no React) for unit-testing resolveCommand. */
function makeScope(
  commands: CommandDef[],
  parent: CommandScope | null = null,
): CommandScope {
  const map = new Map<string, CommandDef>();
  for (const cmd of commands) map.set(cmd.id, cmd);
  return { commands: map, parent };
}

/** Shorthand for creating a minimal CommandDef. */
function cmd(id: string, overrides: Partial<CommandDef> = {}): CommandDef {
  return { id, name: id, execute: overrides.execute ?? vi.fn(), ...overrides };
}

/** Wrap children in one or more nested CommandScopeProviders. */
function wrapper(
  layers: CommandDef[][],
): ({ children }: { children: ReactNode }) => ReactNode {
  return ({ children }: { children: ReactNode }) => {
    let el = children;
    // Wrap from outermost (last) to innermost (first)
    for (let i = layers.length - 1; i >= 0; i--) {
      el = (
        <CommandScopeProvider commands={layers[i]}>{el}</CommandScopeProvider>
      );
    }
    return el;
  };
}

/* ---------- resolveCommand (pure) ---------- */

describe("resolveCommand", () => {
  it("returns a command registered in the scope", () => {
    const save = cmd("save");
    const scope = makeScope([save]);
    expect(resolveCommand(scope, "save")).toBe(save);
  });

  it("returns null for an unknown id", () => {
    const scope = makeScope([cmd("save")]);
    expect(resolveCommand(scope, "delete")).toBeNull();
  });

  it("walks up to parent when child does not have the id", () => {
    const save = cmd("save");
    const parent = makeScope([save]);
    const child = makeScope([cmd("open")], parent);
    expect(resolveCommand(child, "save")).toBe(save);
  });

  it("returns null from an empty scope with no parent", () => {
    const scope = makeScope([]);
    expect(resolveCommand(scope, "anything")).toBeNull();
  });

  it("returns null when scope is null", () => {
    expect(resolveCommand(null, "save")).toBeNull();
  });

  it("child command shadows parent command with same id", () => {
    const parentSave = cmd("save", { name: "Parent Save" });
    const childSave = cmd("save", { name: "Child Save" });
    const parent = makeScope([parentSave]);
    const child = makeScope([childSave], parent);
    expect(resolveCommand(child, "save")).toBe(childSave);
  });

  it("blocks resolution when child marks command available:false", () => {
    const parentSave = cmd("save", { name: "Parent Save" });
    const blocker = cmd("save", { available: false });
    const parent = makeScope([parentSave]);
    const child = makeScope([blocker], parent);
    expect(resolveCommand(child, "save")).toBeNull();
  });

  it("available:true behaves the same as omitting available", () => {
    const save = cmd("save", { available: true });
    const scope = makeScope([save]);
    expect(resolveCommand(scope, "save")).toBe(save);
  });
});

/* ---------- useAvailableCommands (hook) ---------- */

describe("useAvailableCommands", () => {
  it("returns commands from a single scope at depth 0", () => {
    const { result } = renderHook(() => useAvailableCommands(), {
      wrapper: wrapper([[cmd("save"), cmd("open")]]),
    });
    expect(result.current).toHaveLength(2);
    expect(result.current.every((c) => c.depth === 0)).toBe(true);
    const ids = result.current.map((c) => c.command.id);
    expect(ids).toContain("save");
    expect(ids).toContain("open");
  });

  it("returns commands from parent at depth 1", () => {
    const { result } = renderHook(() => useAvailableCommands(), {
      wrapper: wrapper([[cmd("open")], [cmd("save")]]),
    });
    // "save" is in the innermost (depth 0), "open" is in the outer (depth 1)
    expect(result.current).toHaveLength(2);
    const save = result.current.find((c) => c.command.id === "save")!;
    const open = result.current.find((c) => c.command.id === "open")!;
    expect(save.depth).toBe(0);
    expect(open.depth).toBe(1);
  });

  it("child shadows parent command with same id", () => {
    const { result } = renderHook(() => useAvailableCommands(), {
      wrapper: wrapper([
        [cmd("save", { name: "Parent Save" })],
        [cmd("save", { name: "Child Save" })],
      ]),
    });
    expect(result.current).toHaveLength(1);
    expect(result.current[0].command.name).toBe("Child Save");
    expect(result.current[0].depth).toBe(0);
  });

  it("excludes blocked commands and prevents parent from surfacing", () => {
    const { result } = renderHook(() => useAvailableCommands(), {
      wrapper: wrapper([
        [cmd("save", { name: "Parent Save" })],
        [cmd("save", { available: false })],
      ]),
    });
    const ids = result.current.map((c) => c.command.id);
    expect(ids).not.toContain("save");
  });

  it("returns empty array when no scope is provided", () => {
    const { result } = renderHook(() => useAvailableCommands());
    expect(result.current).toEqual([]);
  });

  it("handles three-level nesting correctly", () => {
    const { result } = renderHook(() => useAvailableCommands(), {
      wrapper: wrapper([
        [cmd("a"), cmd("b")], // depth 2  (grandparent)
        [cmd("b", { name: "B2" })], // depth 1  (parent — shadows b)
        [cmd("c")], // depth 0  (child)
      ]),
    });
    expect(result.current).toHaveLength(3);
    const a = result.current.find((c) => c.command.id === "a")!;
    const b = result.current.find((c) => c.command.id === "b")!;
    const c = result.current.find((c) => c.command.id === "c")!;
    expect(a.depth).toBe(2);
    expect(b.depth).toBe(1);
    expect(b.command.name).toBe("B2");
    expect(c.depth).toBe(0);
  });
});

/* ---------- useExecuteCommand (hook) ---------- */

describe("useExecuteCommand", () => {
  it("executes a resolved command and returns true", async () => {
    const fn = vi.fn();
    const { result } = renderHook(() => useExecuteCommand(), {
      wrapper: wrapper([[cmd("save", { execute: fn })]]),
    });

    let executed: boolean = false;
    await act(async () => {
      executed = await result.current("save");
    });
    expect(executed).toBe(true);
    expect(fn).toHaveBeenCalledOnce();
  });

  it("returns false for an unknown command", async () => {
    const { result } = renderHook(() => useExecuteCommand(), {
      wrapper: wrapper([[cmd("save")]]),
    });

    let executed: boolean = true;
    await act(async () => {
      executed = await result.current("nope");
    });
    expect(executed).toBe(false);
  });

  it("returns false for a blocked command", async () => {
    const parentFn = vi.fn();
    const { result } = renderHook(() => useExecuteCommand(), {
      wrapper: wrapper([
        [cmd("save", { execute: parentFn })],
        [cmd("save", { available: false })],
      ]),
    });

    let executed: boolean = true;
    await act(async () => {
      executed = await result.current("save");
    });
    expect(executed).toBe(false);
    expect(parentFn).not.toHaveBeenCalled();
  });

  it("executes parent command when child does not register it", async () => {
    const parentFn = vi.fn();
    const { result } = renderHook(() => useExecuteCommand(), {
      wrapper: wrapper([[cmd("save", { execute: parentFn })], [cmd("open")]]),
    });

    let executed: boolean = false;
    await act(async () => {
      executed = await result.current("save");
    });
    expect(executed).toBe(true);
    expect(parentFn).toHaveBeenCalledOnce();
  });

  it("handles async execute functions", async () => {
    const fn = vi.fn(async () => {
      await new Promise((r) => setTimeout(r, 0));
    });
    const { result } = renderHook(() => useExecuteCommand(), {
      wrapper: wrapper([[cmd("save", { execute: fn })]]),
    });

    await act(async () => {
      await result.current("save");
    });
    expect(fn).toHaveBeenCalledOnce();
  });
});

/* ---------- target-aware accumulation ---------- */

describe("useAvailableCommands with target", () => {
  it("same id + different target → both visible", () => {
    const { result } = renderHook(() => useAvailableCommands(), {
      wrapper: wrapper([
        [cmd("entity.inspect", { name: "Inspect task", target: "task:abc" })],
        [cmd("entity.inspect", { name: "Inspect tag", target: "tag:xyz" })],
      ]),
    });
    expect(result.current).toHaveLength(2);
    const names = result.current.map((c) => c.command.name);
    expect(names).toContain("Inspect task");
    expect(names).toContain("Inspect tag");
  });

  it("same id + same target → inner shadows outer", () => {
    const { result } = renderHook(() => useAvailableCommands(), {
      wrapper: wrapper([
        [cmd("entity.inspect", { name: "Outer", target: "task:abc" })],
        [cmd("entity.inspect", { name: "Inner", target: "task:abc" })],
      ]),
    });
    expect(result.current).toHaveLength(1);
    expect(result.current[0].command.name).toBe("Inner");
  });

  it("no target → shadows by id alone (existing behavior)", () => {
    const { result } = renderHook(() => useAvailableCommands(), {
      wrapper: wrapper([
        [cmd("save", { name: "Parent Save" })],
        [cmd("save", { name: "Child Save" })],
      ]),
    });
    expect(result.current).toHaveLength(1);
    expect(result.current[0].command.name).toBe("Child Save");
  });

  it("available:false blocks same (id, target) key from parent", () => {
    const { result } = renderHook(() => useAvailableCommands(), {
      wrapper: wrapper([
        [cmd("entity.inspect", { name: "Parent", target: "tag:xyz" })],
        [cmd("entity.inspect", { available: false, target: "tag:xyz" })],
      ]),
    });
    const names = result.current.map((c) => c.command.name);
    expect(names).not.toContain("Parent");
  });

  it("available:false with different target does NOT block parent", () => {
    const { result } = renderHook(() => useAvailableCommands(), {
      wrapper: wrapper([
        [cmd("entity.inspect", { name: "Inspect task", target: "task:abc" })],
        [cmd("entity.inspect", { available: false, target: "tag:xyz" })],
      ]),
    });
    expect(result.current).toHaveLength(1);
    expect(result.current[0].command.name).toBe("Inspect task");
  });
});

/* ---------- Multiple scopes: only the focused branch participates ---------- */

describe("multiple scope branches", () => {
  it("sibling branches do not see each other's commands", () => {
    // Simulate two sibling branches by rendering two independent hooks,
    // each in its own nested scope under the same parent.
    const parentCmds = [cmd("global")];

    const branchA = [cmd("action-a")];
    const branchB = [cmd("action-b")];

    const wrapperA = ({ children }: { children: ReactNode }) => (
      <CommandScopeProvider commands={parentCmds}>
        <CommandScopeProvider commands={branchA}>
          {children}
        </CommandScopeProvider>
      </CommandScopeProvider>
    );
    const wrapperB = ({ children }: { children: ReactNode }) => (
      <CommandScopeProvider commands={parentCmds}>
        <CommandScopeProvider commands={branchB}>
          {children}
        </CommandScopeProvider>
      </CommandScopeProvider>
    );

    const { result: a } = renderHook(() => useAvailableCommands(), {
      wrapper: wrapperA,
    });
    const { result: b } = renderHook(() => useAvailableCommands(), {
      wrapper: wrapperB,
    });

    const aIds = a.current.map((c) => c.command.id);
    const bIds = b.current.map((c) => c.command.id);

    // Branch A sees global + action-a, but NOT action-b
    expect(aIds).toContain("global");
    expect(aIds).toContain("action-a");
    expect(aIds).not.toContain("action-b");

    // Branch B sees global + action-b, but NOT action-a
    expect(bIds).toContain("global");
    expect(bIds).toContain("action-b");
    expect(bIds).not.toContain("action-a");
  });
});

/* ---------- CommandScope with moniker ---------- */

describe("CommandScope moniker", () => {
  it("scope can carry an optional moniker field", () => {
    const scope = makeScope([cmd("save")]);
    expect(scope.moniker).toBeUndefined();

    const namedScope: CommandScope = {
      commands: new Map(),
      parent: null,
      moniker: "task:abc",
    };
    expect(namedScope.moniker).toBe("task:abc");
  });

  it("resolveCommand works with moniker-bearing scopes", () => {
    const parent: CommandScope = {
      commands: new Map([["global", cmd("global")]]),
      parent: null,
      moniker: "board:main",
    };
    const child: CommandScope = {
      commands: new Map([["local", cmd("local")]]),
      parent,
      moniker: "task:abc",
    };
    expect(resolveCommand(child, "global")).toBeTruthy();
    expect(resolveCommand(child, "local")).toBeTruthy();
  });
});

/* ---------- collectAvailableCommands ---------- */

describe("collectAvailableCommands", () => {
  it("returns commands from an explicit scope", () => {
    const scope = makeScope([cmd("save"), cmd("open")]);
    const result = collectAvailableCommands(scope);
    expect(result).toHaveLength(2);
    const ids = result.map((c) => c.command.id);
    expect(ids).toContain("save");
    expect(ids).toContain("open");
  });

  it("returns empty array for null scope", () => {
    expect(collectAvailableCommands(null)).toEqual([]);
  });

  it("walks parent chain like useAvailableCommands", () => {
    const parent = makeScope([cmd("global")]);
    const child = makeScope([cmd("local")], parent);
    const result = collectAvailableCommands(child);
    expect(result).toHaveLength(2);
    const local = result.find((c) => c.command.id === "local")!;
    const global = result.find((c) => c.command.id === "global")!;
    expect(local.depth).toBe(0);
    expect(global.depth).toBe(1);
  });
});

/* ---------- dispatchCommand ---------- */

describe("dispatchCommand", () => {
  it("calls execute when set", async () => {
    const execute = vi.fn();
    await dispatchCommand({ id: "test", name: "Test", execute });
    expect(execute).toHaveBeenCalledOnce();
  });

  it("dispatches to Rust by id when no execute is set", async () => {
    const { invoke } = await import("@tauri-apps/api/core");
    (invoke as ReturnType<typeof vi.fn>).mockResolvedValue({});
    await dispatchCommand({
      id: "entity.delete",
      name: "Test",
      target: "task:abc",
      args: { moniker: "task:abc" },
    });
    expect(invoke).toHaveBeenCalledWith("dispatch_command", {
      cmd: "entity.delete",
      target: "task:abc",
      args: { moniker: "task:abc" },
      windowLabel: "main",
    });
  });

  it("prefers execute over Rust dispatch", async () => {
    const execute = vi.fn();
    const { invoke } = await import("@tauri-apps/api/core");
    (invoke as ReturnType<typeof vi.fn>).mockClear();
    await dispatchCommand({
      id: "entity.delete",
      name: "Test",
      execute,
      args: { moniker: "task:abc" },
    });
    expect(execute).toHaveBeenCalledOnce();
    // invoke should NOT have been called for dispatch_command
    expect(invoke).not.toHaveBeenCalledWith(
      "dispatch_command",
      expect.anything(),
    );
  });

  it("dispatches to Rust when no execute is set (no args)", async () => {
    const { invoke } = await import("@tauri-apps/api/core");
    (invoke as ReturnType<typeof vi.fn>).mockResolvedValue({});
    await dispatchCommand({ id: "app.undo", name: "Undo" });
    expect(invoke).toHaveBeenCalledWith("dispatch_command", {
      cmd: "app.undo",
      target: undefined,
      args: undefined,
      windowLabel: "main",
    });
  });
});
