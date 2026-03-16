import { createContext, useContext, useMemo, useCallback, type ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";

/** Describes where a command should appear in the native OS menu bar. */
export interface MenuPlacement {
  /** Which menu to place the command in. */
  menu: "app" | "file" | "edit" | "settings" | "window";
  /** Separator group number within the menu (items in the same group are contiguous). */
  group: number;
  /** Sort order within the group. */
  order: number;
  /** If set, this command is part of a radio group (only one checked at a time). */
  radioGroup?: string;
  /** Whether this command's menu item is currently checked. */
  checked?: boolean;
}

/** Definition of a single command that can be registered in a scope. */
export interface CommandDef {
  /** Unique identifier used to resolve the command through the scope chain. */
  id: string;
  /** Human-readable name for display in palettes and tooltips. */
  name: string;
  /** Optional longer description of what the command does. */
  description?: string;
  /** Optional key bindings per keymap mode. */
  keys?: { vim?: string; cua?: string; emacs?: string };
  /** The action to run when the command is executed. */
  execute?: () => void | Promise<void>;
  /**
   * Whether the command is currently available. Defaults to true.
   * When false, the command blocks resolution — parent scopes will NOT
   * be searched for a command with the same id.
   */
  available?: boolean;
  /** Optional placement in the native menu bar. */
  menuPlacement?: MenuPlacement;
  /** Whether this command appears in context menus. */
  contextMenu?: boolean;
  /**
   * Optional target moniker (e.g. "tag:xyz", "task:abc").
   *
   * The shadow key becomes `(id, target)`:
   * - Same id + same target → shadow (inner wins)
   * - Same id + different target → accumulate (both visible)
   * - No target → shadow by id alone (existing behavior for app.quit etc.)
   */
  target?: string;
  /** Optional explicit args to pass when dispatching to Rust. */
  args?: Record<string, unknown>;
}

/** A node in the scope chain linking a set of commands to an optional parent. */
export interface CommandScope {
  commands: Map<string, CommandDef>;
  parent: CommandScope | null;
  /** Optional moniker identifying which FocusScope created this scope. */
  moniker?: string;
}

export const CommandScopeContext = createContext<CommandScope | null>(null);

interface CommandScopeProviderProps {
  /** Commands to register in this scope. */
  commands: CommandDef[];
  children: ReactNode;
  /** Optional moniker identifying which FocusScope created this scope. */
  moniker?: string;
}

/**
 * Provides a new command scope that links to the nearest parent scope via
 * React context.  Children see this scope as their nearest scope and can
 * resolve commands upward through the chain.
 */
export function CommandScopeProvider({ commands, children, moniker }: CommandScopeProviderProps) {
  const parent = useContext(CommandScopeContext);

  const scope = useMemo<CommandScope>(() => {
    const map = new Map<string, CommandDef>();
    for (const cmd of commands) {
      map.set(cmd.id, cmd);
    }
    return { commands: map, parent, moniker };
  }, [commands, parent, moniker]);

  return (
    <CommandScopeContext.Provider value={scope}>
      {children}
    </CommandScopeContext.Provider>
  );
}

/**
 * Walk the scope chain from `scope` toward the root looking for `id`.
 *
 * - If the command is found and `available !== false`, return it.
 * - If the command is found and `available === false`, stop searching (blocking).
 * - If the command is not found in the current scope, continue to parent.
 *
 * Note: this resolves by `id` alone — the first match wins regardless of
 * `target`. This is intentional: keyboard shortcuts act on the nearest
 * (innermost) entity. For target-aware accumulation (e.g. context menus
 * showing all targeted commands), use `collectAvailableCommands` instead.
 *
 * @returns The resolved CommandDef, or null if not found or blocked.
 */
export function resolveCommand(scope: CommandScope | null, id: string): CommandDef | null {
  let current = scope;
  while (current !== null) {
    const cmd = current.commands.get(id);
    if (cmd !== undefined) {
      // Found the id — check availability
      return cmd.available !== false ? cmd : null;
    }
    current = current.parent;
  }
  return null;
}

/** A command annotated with its depth in the scope chain (0 = deepest / nearest). */
export interface CommandAtDepth {
  command: CommandDef;
  depth: number;
}

/**
 * Collect all commands visible from a given scope, grouped by depth.
 *
 * Commands at deeper (nearer) scopes shadow same-id commands at shallower
 * (farther) scopes.  Commands with `available === false` are excluded and
 * also block same-id commands from parent scopes.
 *
 * @param scope - The starting scope to walk from.
 * @returns An array of `{ command, depth }` sorted by depth ascending (nearest first).
 */
export function collectAvailableCommands(scope: CommandScope | null): CommandAtDepth[] {
  /**
   * Shadow key: `id + ":" + (target ?? "")`.
   * Same id + same target → shadow (inner wins).
   * Same id + different target → accumulate (both visible).
   * No target → shadow by id alone (existing behaviour for app.quit etc.)
   */
  const seen = new Set<string>();
  const result: CommandAtDepth[] = [];
  let current = scope;
  let depth = 0;

  while (current !== null) {
    for (const [, cmd] of current.commands) {
      const key = cmd.id + ":" + (cmd.target ?? "");
      if (seen.has(key)) continue;
      seen.add(key);
      if (cmd.available !== false) {
        result.push({ command: cmd, depth });
      }
      // Whether available or not, the key is now "seen" — blocks parents.
    }
    current = current.parent;
    depth++;
  }

  return result;
}

/**
 * Hook version: collect all commands visible from the nearest scope context.
 *
 * @returns An array of `{ command, depth }` sorted by depth ascending (nearest first).
 */
export function useAvailableCommands(): CommandAtDepth[] {
  const scope = useContext(CommandScopeContext);
  return useMemo(() => collectAvailableCommands(scope), [scope]);
}

/**
 * Execute a command. If `execute` is set, calls it directly.
 * Otherwise dispatches to Rust by command id via invoke("dispatch_command").
 */
export async function dispatchCommand(cmd: CommandDef): Promise<void> {
  if (cmd.execute) {
    // Log to Rust backend so every command appears in the unified log
    Promise.resolve(invoke("log_command", { cmd: cmd.id, target: cmd.target })).catch(() => {});
    await cmd.execute();
  } else {
    // Dispatch to Rust by command ID (dispatch_command logs internally)
    await invoke("dispatch_command", {
      cmd: cmd.id,
      target: cmd.target,
      args: cmd.args,
    });
  }
}

/**
 * Returns a function that resolves a command id through the current scope
 * chain and executes it.
 *
 * @returns An async function `(id: string) => Promise<boolean>`.
 *          Resolves to `true` if the command was found and executed,
 *          `false` otherwise.
 */
export function useExecuteCommand(): (id: string) => Promise<boolean> {
  const scope = useContext(CommandScopeContext);

  return useCallback(
    async (id: string): Promise<boolean> => {
      const cmd = resolveCommand(scope, id);
      if (cmd === null) return false;
      await dispatchCommand(cmd);
      return true;
    },
    [scope],
  );
}
