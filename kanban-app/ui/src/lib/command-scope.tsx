import {
  createContext,
  useContext,
  useMemo,
  useCallback,
  useState,
  type ReactNode,
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

// ---------------------------------------------------------------------------
// CommandBusy context — tracks whether any dispatch_command IPC is in-flight
// ---------------------------------------------------------------------------

/** Shape of the busy-tracking state exposed via context. */
export interface CommandBusyState {
  /** True when at least one backend dispatch_command call is in-flight. */
  isBusy: boolean;
}

const CommandBusyContext = createContext<CommandBusyState>({ isBusy: false });

/**
 * Sentinel default setter used when no `CommandBusyProvider` is mounted.
 *
 * Having an identifiable reference (rather than an anonymous arrow) lets
 * `useSetCommandInflight()` detect the no-provider case and emit a dev-mode
 * warning so wiring regressions (the provider sitting in the wrong place in
 * the tree) fail loudly instead of silently masking busy tracking.
 *
 * Shape matches `React.Dispatch<React.SetStateAction<number>>` but the
 * function is a no-op — any call is ignored.
 */
const NOOP_INFLIGHT_SETTER: React.Dispatch<
  React.SetStateAction<number>
> = () => {};

/**
 * Internal context for the busy-state setter.
 *
 * Separated from the read context so that only `useDispatchCommand` (which
 * needs the setter) re-renders when the setter reference changes, while
 * pure consumers of `isBusy` only re-render on value changes.
 */
const CommandBusySetterContext =
  createContext<React.Dispatch<React.SetStateAction<number>>>(
    NOOP_INFLIGHT_SETTER,
  );

/** Props for the command busy provider. */
export interface CommandBusyProviderProps {
  children: ReactNode;
}

/**
 * Tracks the number of in-flight backend dispatch_command calls and
 * exposes `isBusy` (count > 0) via React context.
 *
 * Wrap this around the component tree that needs busy awareness — typically
 * at the same level as `ActiveBoardPathProvider`.
 */
export function CommandBusyProvider({ children }: CommandBusyProviderProps) {
  const [inflightCount, setInflightCount] = useState(0);
  const busyState = useMemo<CommandBusyState>(
    () => ({ isBusy: inflightCount > 0 }),
    [inflightCount],
  );
  return (
    <CommandBusySetterContext.Provider value={setInflightCount}>
      <CommandBusyContext.Provider value={busyState}>
        {children}
      </CommandBusyContext.Provider>
    </CommandBusySetterContext.Provider>
  );
}

/** Read whether any backend command is currently in-flight. */
export function useCommandBusy(): CommandBusyState {
  return useContext(CommandBusyContext);
}

/**
 * Read the busy-state setter so non-dispatch callers can participate in
 * the same in-flight counter consumed by `useCommandBusy`.
 *
 * Usage pattern (matching `useDispatchCommand`):
 *
 * ```ts
 * const setInflightCount = useSetCommandInflight();
 * setInflightCount((c) => c + 1);
 * try { await longRunningWork(); } finally { setInflightCount((c) => c - 1); }
 * ```
 *
 * Components that want the nav-bar progress bar to light up for their own
 * IPC fan-out (e.g. board refresh) should wrap their async work with this
 * setter rather than introducing a parallel busy context.
 *
 * **Production contract**: every real call site must sit inside a
 * `CommandBusyProvider`. The production tree in `App.tsx` mounts the
 * provider above both writers (`useDispatchCommand` via `WindowContainer`,
 * `refreshEntities` via `RustEngineContainer`).
 *
 * Outside that tree the hook returns a no-op setter so isolated unit tests
 * and synthetic probes do not need to stub the provider. In development
 * builds, calling the no-op setter logs a one-time warning — a silent
 * no-op here once masked a wiring regression where the provider was nested
 * below one of its writers. If you see that warning in real use, the tree
 * is wrong; fix the provider placement.
 */
export function useSetCommandInflight(): React.Dispatch<
  React.SetStateAction<number>
> {
  const setter = useContext(CommandBusySetterContext);
  if (setter === NOOP_INFLIGHT_SETTER) {
    return warnOnceNoopSetter;
  }
  return setter;
}

/**
 * Module-level flag so the dev warning fires at most once per session —
 * avoids log spam when the hook is called in a render loop.
 */
let hasWarnedNoopInflight = false;

/**
 * Dev-only wrapper around the no-op setter: logs a single warning the first
 * time it is invoked from outside a `CommandBusyProvider` tree, so missing
 * provider wiring is noisy in development builds.
 *
 * The warning is gated on `import.meta.env.DEV` so production bundles keep
 * the silent no-op behavior (and do not log console output to end users).
 */
function warnOnceNoopSetter(_value: number | ((prev: number) => number)): void {
  if (
    !hasWarnedNoopInflight &&
    typeof import.meta !== "undefined" &&
    import.meta.env?.DEV
  ) {
    hasWarnedNoopInflight = true;
    // eslint-disable-next-line no-console
    console.warn(
      "[command-scope] useSetCommandInflight() called outside a " +
        "CommandBusyProvider tree. The in-flight counter will not update and " +
        "the nav-bar progress bar will not reflect this work. Ensure the " +
        "provider wraps both dispatch and refetch call sites.",
    );
  }
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

/**
 * Context for the nearest enclosing command scope in the scope tree.
 *
 * Provided by CommandScopeProvider. Each scope registers the commands
 * available at that level and links to its parent, forming a lookup chain
 * from the most specific scope outward to the root. `useDispatchCommand`
 * walks this chain (plus `FocusedScopeContext`) to resolve commands by
 * name and to compute the scope chain dispatched commands receive.
 */
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

// ---------------------------------------------------------------------------
// useDispatchCommand — unified dispatch hook
// ---------------------------------------------------------------------------

/** Options for dispatching a command via useDispatchCommand. */
export interface DispatchOptions {
  /** Additional arguments to pass to the backend command handler. */
  args?: Record<string, unknown>;
  /** Target moniker (e.g. "task:abc") to associate with the dispatch. */
  target?: string;
  /**
   * Explicit scope chain to use instead of the one derived from React context.
   *
   * When provided, this overrides the automatic scope chain computed from the
   * focused or tree scope. This is used by context menu dispatch where the
   * right-click point's scope chain is known from the Rust backend and should
   * take precedence over whatever happens to be focused when the event arrives.
   */
  scopeChain?: string[];
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
  const setInflightCount = useContext(CommandBusySetterContext);

  // Prefer focused scope (includes entity + window monikers) over tree scope
  // (just the component's position in the React tree). When nothing is focused,
  // fall back to tree scope.
  const effectiveScope = focusedScope ?? treeScope;

  return useCallback(
    async (
      cmdOrOpts?: string | DispatchOptions,
      maybeOpts?: DispatchOptions,
    ): Promise<unknown> => {
      const { cmdId, opts } = resolveDispatchArgs(
        presetCmd,
        cmdOrOpts,
        maybeOpts,
      );

      // Try frontend execute handler first
      const resolved = resolveCommand(effectiveScope, cmdId);
      if (resolved?.execute) {
        return runFrontendExecute(cmdId, opts, resolved);
      }

      // Backend dispatch — Tauri IPC with busy tracking. Forward the
      // resolved CommandDef's target so keybinding-driven dispatch
      // (which has no explicit opts.target) still carries the
      // entity-level target stamped by `useEntityCommands`.
      const chain = opts.scopeChain ?? scopeChainFromScope(effectiveScope);
      return runBackendDispatch(
        cmdId,
        opts,
        chain,
        boardPath,
        setInflightCount,
        resolved?.target,
      );
    },
    [presetCmd, effectiveScope, boardPath, setInflightCount],
  );
}

/** Normalize the overloaded call shape into a single `(cmdId, opts)` pair. */
function resolveDispatchArgs(
  presetCmd: string | undefined,
  cmdOrOpts: string | DispatchOptions | undefined,
  maybeOpts: DispatchOptions | undefined,
): { cmdId: string; opts: DispatchOptions } {
  if (presetCmd) {
    return { cmdId: presetCmd, opts: (cmdOrOpts as DispatchOptions) ?? {} };
  }
  return { cmdId: cmdOrOpts as string, opts: maybeOpts ?? {} };
}

/** Run a client-side `execute` handler, fire-and-forget log for telemetry. */
async function runFrontendExecute(
  cmdId: string,
  opts: DispatchOptions,
  resolved: CommandDef,
): Promise<void> {
  Promise.resolve(
    invoke("log_command", {
      cmd: cmdId,
      target: opts.target ?? resolved.target,
    }),
  ).catch(() => {});
  await resolved.execute!();
}

/** Dispatch to the Rust backend, wrapping the call in the busy counter. */
async function runBackendDispatch(
  cmdId: string,
  opts: DispatchOptions,
  chain: string[],
  boardPath: string | undefined,
  setInflightCount: React.Dispatch<React.SetStateAction<number>>,
  resolvedTarget: string | undefined,
): Promise<unknown> {
  setInflightCount((c) => c + 1);
  try {
    return await invoke("dispatch_command", {
      cmd: cmdId,
      // Target priority: caller-supplied opts.target wins (they're
      // explicit), falls back to the target baked into the resolved
      // CommandDef (set e.g. by `useEntityCommands` which stamps
      // every entity command with `target: entityMoniker`). Without
      // this fallback, keybinding-driven dispatch of commands like
      // `ui.inspect` — which rely on the entity-level target for
      // correct inspector routing — hit the backend with
      // `target: undefined` and silently did nothing. See task
      // 01KPX6E0QPNRWZTQXGXX2MBEMV.
      target: opts.target ?? resolvedTarget,
      args: opts.args,
      scopeChain: chain,
      ...(boardPath ? { boardPath } : {}),
    });
  } finally {
    setInflightCount((c) => c - 1);
  }
}
