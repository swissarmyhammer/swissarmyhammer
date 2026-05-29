/**
 * `useCommandList` — read the active command registry from the Command MCP
 * service and keep it live.
 *
 * The hook is the metadata-driven foundation for every command surface
 * (palette, hotkeys, context menu, tab bar): it calls the service's
 * `list command` verb with the supplied filters and re-fetches whenever the
 * host emits a `commands/changed` notification. No command list is ever
 * hardcoded in React — the registry is the single source of truth.
 *
 * ## Debounce
 *
 * The Command service coalesces registry mutations behind a ~100ms quiet
 * window before emitting `commands/changed` (its `ChangeNotifier`). A single
 * logical change can still arrive as a short burst at the frontend (e.g. a
 * plugin reload that purges then re-registers, or a flush firing alongside the
 * tail of a debounce). This hook applies its own ~100ms trailing debounce on
 * top of the subscription so a burst collapses into one `list command`
 * round-trip, matching the server's window.
 */

import { useCallback, useEffect, useRef, useState } from "react";
import {
  callCommandTool,
  subscribeCommandsChanged,
} from "@/lib/mcp-transport";

/** Trailing debounce (ms) applied to `commands/changed` — matches the server. */
export const COMMANDS_CHANGED_DEBOUNCE_MS = 100;

/**
 * One active (top-of-stack) command as returned by `list command`.
 *
 * Mirrors `swissarmyhammer-command-service`'s `CommandMetadata`. Every field
 * past `id` / `name` is optional so the wire shape can grow without breaking
 * the frontend; surfaces read only the metadata they render.
 */
export interface CommandMetadata {
  /** Stable command id (e.g. `"task.move"`). */
  id: string;
  /** Human-readable name (palette / menu label). */
  name: string;
  /** Display name override for native menus. */
  menu_name?: string;
  /** Long-form description (palette detail, tooltip). */
  description?: string;
  /** Grouping category (e.g. `"Cleanup"`). */
  category?: string;
  /** Scope expression list (e.g. `["entity:task"]`); empty/absent = global. */
  scope?: string[];
  /** Keybindings keyed by keymap mode (`vim` / `cua` / `emacs`). */
  keys?: Record<string, string>;
  /** Native menu-bar placement payload. */
  menu?: unknown;
  /** Whether the command appears in the right-click context menu. */
  context_menu?: boolean;
  /** Context-menu group bucket. */
  context_menu_group?: number;
  /** Sort order within the context-menu group. */
  context_menu_order?: number;
  /** Tab-button affordance payload. */
  tab_button?: unknown;
  /** View-kind UI-surface filter (e.g. `["grid"]`). */
  view_kinds?: string[];
  /** Whether the command produces an undoable change. */
  undoable?: boolean;
  /** Whether the command is visible in surfaces. Defaults to true. */
  visible?: boolean;
  /** Param definitions; absent/empty = no dispatch-time arguments. */
  params?: unknown[];
}

/** Envelope returned by the `list command` verb. */
interface ListCommandResult {
  ok: boolean;
  commands: CommandMetadata[];
}

/** Filters for {@link useCommandList} — all intersect (logical AND), like the service. */
export interface UseCommandListOptions {
  /**
   * Keep only commands whose `scope` is empty (global) or contains this
   * expression (e.g. `"entity:task"`).
   */
  scope?: string;
  /** Exact-match filter on the command's `category`. */
  category?: string;
  /** Keep only commands whose id starts with this prefix (e.g. `"task."`). */
  idPrefix?: string;
}

/** What {@link useCommandList} returns. */
export interface UseCommandListResult {
  /** The current active command set matching the filters. */
  commands: CommandMetadata[];
  /** True while the initial `list command` fetch is in flight. */
  loading: boolean;
  /** Force an immediate re-fetch (bypasses the debounce). */
  refresh: () => void;
  /**
   * Monotonic registry version, incremented on every successful fetch
   * (initial, filter change, or a `commands/changed` re-fetch). Feed it to
   * {@link useCommandAvailability} as its `epoch` so a registry mutation
   * invalidates cached availability verdicts — otherwise an already-open
   * palette keeps showing stale grayed-out/enabled rows after the change.
   */
  epoch: number;
}

/** Build the `list command` filter payload, omitting absent filters. */
function buildListParams(
  options: UseCommandListOptions,
): Record<string, unknown> {
  const params: Record<string, unknown> = {};
  if (options.scope !== undefined) params.scope = options.scope;
  if (options.category !== undefined) params.category = options.category;
  if (options.idPrefix !== undefined) params.id_prefix = options.idPrefix;
  return params;
}

/**
 * Read the active command registry, filtered by `scope` / `category` /
 * `idPrefix`, and re-render when the host's command registry changes.
 *
 * @param options - Intersecting filters forwarded to `list command`.
 * @returns The live command list, a loading flag, and a manual `refresh`.
 */
export function useCommandList(
  options: UseCommandListOptions = {},
): UseCommandListResult {
  const [commands, setCommands] = useState<CommandMetadata[]>([]);
  const [loading, setLoading] = useState(true);
  // Monotonic version bumped on each successful fetch — drives availability
  // cache invalidation (see UseCommandListResult.epoch).
  const [epoch, setEpoch] = useState(0);

  // Snapshot the filters into a ref so the fetch callback stays reference
  // stable while still reading the latest filter values at call time. The
  // primitive filter fields drive the effect's re-fetch dependency below.
  const { scope, category, idPrefix } = options;
  const optionsRef = useRef<UseCommandListOptions>({ scope, category, idPrefix });
  optionsRef.current = { scope, category, idPrefix };

  // Guards against stale resolves clobbering a newer fetch's result.
  const fetchIdRef = useRef(0);

  const fetchCommands = useCallback(async () => {
    const myId = ++fetchIdRef.current;
    const params = buildListParams(optionsRef.current);
    try {
      const result = await callCommandTool<ListCommandResult>(
        "list command",
        params,
      );
      if (myId !== fetchIdRef.current) return; // a newer fetch superseded us
      setCommands(result?.commands ?? []);
      setEpoch((e) => e + 1);
    } catch (err) {
      if (myId !== fetchIdRef.current) return;
      console.error("[useCommandList] list command failed:", err);
      setCommands([]);
    } finally {
      if (myId === fetchIdRef.current) setLoading(false);
    }
  }, []);

  // Initial fetch + re-fetch when the filters change.
  useEffect(() => {
    setLoading(true);
    void fetchCommands();
  }, [fetchCommands, scope, category, idPrefix]);

  // Subscribe to `commands/changed`, debounced ~100ms to match the server's
  // coalescing window so a burst of notifications triggers one re-fetch.
  useEffect(() => {
    let disposed = false;
    let timer: ReturnType<typeof setTimeout> | null = null;

    const onChanged = () => {
      if (timer !== null) clearTimeout(timer);
      timer = setTimeout(() => {
        timer = null;
        void fetchCommands();
      }, COMMANDS_CHANGED_DEBOUNCE_MS);
    };

    const unsubPromise = subscribeCommandsChanged(onChanged);

    return () => {
      disposed = true;
      if (timer !== null) clearTimeout(timer);
      unsubPromise.then((unsub) => {
        // The subscription may resolve after unmount — always release it.
        if (disposed) unsub();
      });
    };
  }, [fetchCommands]);

  return { commands, loading, refresh: fetchCommands, epoch };
}
