import { createContext, useContext, useMemo, useCallback, type ReactNode } from "react";

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
  execute: () => void | Promise<void>;
  /**
   * Whether the command is currently available. Defaults to true.
   * When false, the command blocks resolution — parent scopes will NOT
   * be searched for a command with the same id.
   */
  available?: boolean;
}

/** A node in the scope chain linking a set of commands to an optional parent. */
export interface CommandScope {
  commands: Map<string, CommandDef>;
  parent: CommandScope | null;
}

const CommandScopeContext = createContext<CommandScope | null>(null);

interface CommandScopeProviderProps {
  /** Commands to register in this scope. */
  commands: CommandDef[];
  children: ReactNode;
}

/**
 * Provides a new command scope that links to the nearest parent scope via
 * React context.  Children see this scope as their nearest scope and can
 * resolve commands upward through the chain.
 */
export function CommandScopeProvider({ commands, children }: CommandScopeProviderProps) {
  const parent = useContext(CommandScopeContext);

  const scope = useMemo<CommandScope>(() => {
    const map = new Map<string, CommandDef>();
    for (const cmd of commands) {
      map.set(cmd.id, cmd);
    }
    return { commands: map, parent };
  }, [commands, parent]);

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
 * Collect all commands visible from the current scope, grouped by depth.
 *
 * Commands at deeper (nearer) scopes shadow same-id commands at shallower
 * (farther) scopes.  Commands with `available === false` are excluded and
 * also block same-id commands from parent scopes.
 *
 * @returns An array of `{ command, depth }` sorted by depth ascending (nearest first).
 */
export function useAvailableCommands(): CommandAtDepth[] {
  const scope = useContext(CommandScopeContext);

  return useMemo(() => {
    /** Ids we have already seen — either included or blocked. */
    const seen = new Set<string>();
    const result: CommandAtDepth[] = [];
    let current = scope;
    let depth = 0;

    while (current !== null) {
      for (const [id, cmd] of current.commands) {
        if (seen.has(id)) continue;
        seen.add(id);
        if (cmd.available !== false) {
          result.push({ command: cmd, depth });
        }
        // Whether available or not, the id is now "seen" — blocks parents.
      }
      current = current.parent;
      depth++;
    }

    return result;
  }, [scope]);
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
      await cmd.execute();
      return true;
    },
    [scope],
  );
}
