import {
  createContext,
  useContext,
  useMemo,
  useCallback,
  type ReactNode,
  useRef,
} from "react";
import { invoke } from "@tauri-apps/api/core";

// ---------------------------------------------------------------------------
// ActiveBoardPath context — per-window board path for multi-window dispatch
// ---------------------------------------------------------------------------

const ActiveBoardPathContext = createContext<string | undefined>(undefined);

/** Props for the per-window active board path provider. */
export interface ActiveBoardPathProviderProps {
  value: string | undefined;
  children: ReactNode;
}

/** Provide the per-window active board path so dispatch_command targets the correct board. */
export function ActiveBoardPathProvider({
  value,
  children,
}: ActiveBoardPathProviderProps) {
  return (
    <ActiveBoardPathContext.Provider value={value}>
      {children}
    </ActiveBoardPathContext.Provider>
  );
}

/** Read the per-window active board path. */
export function useActiveBoardPath(): string | undefined {
  return useContext(ActiveBoardPathContext);
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

/**
 * Context for the currently focused entity's scope.
 *
 * Defined here (in command-scope) so useDispatchCommand can read it without
 * importing from entity-focus-context (which would create a circular dep).
 * Provided by EntityFocusProvider in entity-focus-context.tsx.
 *
 * When populated, useDispatchCommand prefers this over CommandScopeContext
 * for scope chain computation — ensuring dispatched commands carry the
 * focused entity's full scope chain (e.g. task:abc → column:todo → window:main).
 */
export const FocusedScopeContext = createContext<CommandScope | null>(null);

/**
 * Walk the scope chain from innermost to root, collecting monikers.
 *
 * Returns an array like `["task:abc", "column:todo", "window:board-2"]`.
 * Scopes without monikers are skipped.
 */
export function scopeChainFromScope(scope: CommandScope | null): string[] {
  const chain: string[] = [];
  let current = scope;
  while (current) {
    if (current.moniker) chain.push(current.moniker);
    current = current.parent;
  }
  return chain;
}

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
export function CommandScopeProvider({
  commands,
  children,
  moniker,
}: CommandScopeProviderProps) {
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
export function resolveCommand(
  scope: CommandScope | null,
  id: string,
): CommandDef | null {
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
export function collectAvailableCommands(
  scope: CommandScope | null,
): CommandAtDepth[] {
  /**
   * Shadow key: `id + ":" + (target ?? "")`.
   * Same id + same target → shadow (inner wins).
   * Same id + different target → both visible (Copy Tag ≠ Copy Task).
   * No target → shadow by id alone.
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
 * Dispatch a command to the Rust backend via Tauri IPC.
 *
 * Every call MUST include `scopeChain` so the backend knows which window
 * the command originates from. This is enforced by the type signature.
 *
 * @deprecated Use useDispatchCommand instead.
 */
export async function backendDispatch(
  params: { scopeChain: string[] } & Record<string, unknown>,
): Promise<unknown> {
  return invoke("dispatch_command", params);
}

/**
 * Execute a command. If `execute` is set, calls it directly.
 * Otherwise dispatches to Rust by command id via backendDispatch().
 *
 * @deprecated Use useDispatchCommand instead.
 *
 * @param boardPath — optional per-window board path for multi-window dispatch.
 *   When provided, Rust routes the command to the correct board instead of the
 *   global "last-used" active board.
 */
export async function dispatchCommand(
  cmd: CommandDef,
  boardPath: string | undefined,
  scopeChain: string[],
): Promise<void> {
  if (cmd.execute) {
    // Log to Rust backend so every command appears in the unified log
    Promise.resolve(
      invoke("log_command", { cmd: cmd.id, target: cmd.target }),
    ).catch(() => {});
    await cmd.execute();
  } else {
    // Dispatch to Rust by command ID (dispatch_command logs internally)
    await backendDispatch({
      cmd: cmd.id,
      target: cmd.target,
      args: cmd.args,
      scopeChain,
      ...(boardPath ? { boardPath } : {}),
    });
  }
}

/**
 * Returns a function that resolves a command id through the current scope
 * chain and executes it.
 *
 * @deprecated Use useDispatchCommand instead.
 *
 * @returns An async function `(id: string) => Promise<boolean>`.
 *          Resolves to `true` if the command was found and executed,
 *          `false` otherwise.
 */
export function useExecuteCommand(): (id: string) => Promise<boolean> {
  const scope = useContext(CommandScopeContext);
  const boardPath = useContext(ActiveBoardPathContext);
  // Store in ref so the callback always sees the latest value without
  // re-creating on every board path change.
  const boardPathRef = useRef(boardPath);
  boardPathRef.current = boardPath;

  return useCallback(
    async (id: string): Promise<boolean> => {
      const cmd = resolveCommand(scope, id);
      if (cmd === null) return false;
      const chain = scopeChainFromScope(scope);
      await dispatchCommand(cmd, boardPathRef.current, chain);
      return true;
    },
    [scope],
  );
}

// ---------------------------------------------------------------------------
// useDispatchCommand — unified dispatch hook
// ---------------------------------------------------------------------------

/** Options for dispatching a command via useDispatchCommand. */
export interface DispatchOptions {
  /** Additional arguments to pass to the backend command handler. */
  args?: Record<string, unknown>;
  /** Target moniker (e.g. "task:abc") to associate with the dispatch. */
  target?: string;
}

/**
 * Unified hook for dispatching commands from React components.
 *
 * Automatically reads the scope chain and board path from context.
 * Commands registered in scope with an `execute` handler run client-side;
 * all others are forwarded to the Rust backend via `dispatch_command`.
 *
 * @overload Ad-hoc dispatch: returns a function that takes a command ID and options.
 */
export function useDispatchCommand(): (
  cmd: string,
  opts?: DispatchOptions,
) => Promise<unknown>;
/**
 * @overload Pre-bound dispatch: returns a function that takes just options.
 */
export function useDispatchCommand(
  cmd: string,
): (opts?: DispatchOptions) => Promise<unknown>;
export function useDispatchCommand(presetCmd?: string) {
  const treeScope = useContext(CommandScopeContext);
  const focusedScope = useContext(FocusedScopeContext);
  const boardPath = useContext(ActiveBoardPathContext);

  // Prefer focused scope (includes entity + window monikers) over tree scope
  // (just the component's position in the React tree). When nothing is focused,
  // fall back to tree scope.
  const effectiveScope = focusedScope ?? treeScope;

  return useCallback(
    async (
      cmdOrOpts?: string | DispatchOptions,
      maybeOpts?: DispatchOptions,
    ): Promise<unknown> => {
      let cmdId: string;
      let opts: DispatchOptions;
      if (presetCmd) {
        cmdId = presetCmd;
        opts = (cmdOrOpts as DispatchOptions) ?? {};
      } else {
        cmdId = cmdOrOpts as string;
        opts = maybeOpts ?? {};
      }

      const chain = scopeChainFromScope(effectiveScope);

      // Try frontend execute handler first
      const resolved = resolveCommand(effectiveScope, cmdId);
      if (resolved?.execute) {
        Promise.resolve(
          invoke("log_command", {
            cmd: cmdId,
            target: opts.target ?? resolved.target,
          }),
        ).catch(() => {});
        await resolved.execute();
        return;
      }

      // Backend dispatch
      return backendDispatch({
        cmd: cmdId,
        target: opts.target,
        args: opts.args,
        scopeChain: chain,
        ...(boardPath ? { boardPath } : {}),
      });
    },
    [presetCmd, effectiveScope, boardPath],
  );
}
