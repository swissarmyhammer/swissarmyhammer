import { useCallback, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  useAvailableCommands,
  dispatchCommand,
  type CommandDef,
} from "@/lib/command-scope";

/**
 * Module-level map of pending context menu handlers.
 * Only one context menu is open at a time, so this is safe.
 * Populated when a context menu opens, consumed when an item is selected.
 */
const pendingHandlers = new Map<string, CommandDef>();

/**
 * Build the pendingHandlers key for a command.
 * Uses `id:target` when a target is set, plain `id` otherwise.
 */
function handlerKey(cmd: CommandDef): string {
  return cmd.target ? `${cmd.id}:${cmd.target}` : cmd.id;
}

/**
 * Dispatch a context menu command by handler key.
 * Called by the global event listener when the user selects a menu item.
 *
 * @param key - The handler key selected from the native context menu.
 * @returns true if the command was found and dispatched.
 */
export async function dispatchContextMenuCommand(
  key: string,
): Promise<boolean> {
  const cmd = pendingHandlers.get(key);
  if (!cmd) return false;
  await dispatchCommand(cmd);
  return true;
}

/**
 * Hook that returns an onContextMenu handler.
 *
 * When fired, it:
 * 1. Collects commands with contextMenu: true from the current scope chain
 * 2. Calls the backend to check which commands are actually available
 *    (e.g., paste requires clipboard content — the Rust side knows this)
 * 3. Filters to only available commands
 * 4. Registers their handlers and shows a native popup menu
 *
 * Must be used inside a CommandScopeProvider.
 */
export function useContextMenu(): (e: React.MouseEvent) => void {
  const allCommands = useAvailableCommands();

  const contextCommands = useMemo(
    () => allCommands.filter((c) => c.command.contextMenu),
    [allCommands],
  );

  return useCallback(
    (e: React.MouseEvent) => {
      e.preventDefault();
      e.stopPropagation();

      if (contextCommands.length === 0) return;

      // Ask the backend which commands are available in the current context.
      // This checks Command::available() on each — including clipboard state,
      // scope requirements, and any other dynamic conditions.
      invoke<Array<{ id: string; name: string }>>("list_available_commands", {
        contextMenu: true,
      })
        .then((available) => {
          // Build a map of available command IDs → backend names (may be templated)
          const availableMap = new Map(available.map((c) => [c.id, c.name]));

          // Filter frontend commands to those the backend says are available,
          // deduplicating by command ID (keep innermost scope — lowest depth).
          const seen = new Set<string>();
          const filtered = contextCommands.filter((c) => {
            if (!availableMap.has(c.command.id)) return false;
            if (seen.has(c.command.id)) return false;
            seen.add(c.command.id);
            return true;
          });

          if (filtered.length === 0) return;

          // Register handlers and build menu items.
          // Use backend name when available (e.g. "Paste Task" instead of "Paste").
          pendingHandlers.clear();
          const items: Array<{ id: string; name: string }> = [];
          let lastDepth: number | null = null;

          for (const c of filtered) {
            if (lastDepth !== null && c.depth !== lastDepth) {
              items.push({ id: "__separator__", name: "" });
            }
            const key = handlerKey(c.command);
            pendingHandlers.set(key, c.command);
            const displayName = availableMap.get(c.command.id) ?? c.command.name;
            items.push({ id: key, name: displayName });
            lastDepth = c.depth;
          }

          invoke("show_context_menu", { items }).catch(console.error);
        })
        .catch(console.error);
    },
    [contextCommands],
  );
}
