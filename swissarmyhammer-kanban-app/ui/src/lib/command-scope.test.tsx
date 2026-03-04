import { describe, it, expect, vi } from "vitest";
import { renderHook, act } from "@testing-library/react";
import type { ReactNode } from "react";
import {
  CommandScopeProvider,
  resolveCommand,
  useAvailableCommands,
  useExecuteCommand,
  type CommandDef,
  type CommandScope,
} from "./command-scope";

/* ---------- helpers ---------- */

/** Build a CommandScope value directly (no React) for unit-testing resolveCommand. */
function makeScope(commands: CommandDef[], parent: CommandScope | null = null): CommandScope {
  const map = new Map<string, CommandDef>();
  for (const cmd of commands) map.set(cmd.id, cmd);
  return { commands: map, parent };
}

/** Shorthand for creating a minimal CommandDef. */
function cmd(id: string, overrides: Partial<CommandDef> = {}): CommandDef {
  return { id, name: id, execute: overrides.execute ?? vi.fn(), ...overrides };
}

/** Wrap children in one or more nested CommandScopeProviders. */
function wrapper(layers: CommandDef[][]): ({ children }: { children: ReactNode }) => ReactNode {
  return ({ children }: { children: ReactNode }) => {
    let el = children;
    // Wrap from outermost (last) to innermost (first)
    for (let i = layers.length - 1; i >= 0; i--) {
      el = <CommandScopeProvider commands={layers[i]}>{el}</CommandScopeProvider>;
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
        [cmd("a"), cmd("b")],       // depth 2  (grandparent)
        [cmd("b", { name: "B2" })], // depth 1  (parent — shadows b)
        [cmd("c")],                 // depth 0  (child)
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
      wrapper: wrapper([
        [cmd("save", { execute: parentFn })],
        [cmd("open")],
      ]),
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
        <CommandScopeProvider commands={branchA}>{children}</CommandScopeProvider>
      </CommandScopeProvider>
    );
    const wrapperB = ({ children }: { children: ReactNode }) => (
      <CommandScopeProvider commands={parentCmds}>
        <CommandScopeProvider commands={branchB}>{children}</CommandScopeProvider>
      </CommandScopeProvider>
    );

    const { result: a } = renderHook(() => useAvailableCommands(), { wrapper: wrapperA });
    const { result: b } = renderHook(() => useAvailableCommands(), { wrapper: wrapperB });

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
