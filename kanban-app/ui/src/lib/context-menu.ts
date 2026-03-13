import { useCallback, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useAvailableCommands, dispatchCommand, type CommandDef } from "@/lib/command-scope";

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
export async function dispatchContextMenuCommand(key: string): Promise<boolean> {
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
 * 2. Registers their handlers in the pending map
 * 3. Calls Rust to show a native popup menu with those items
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

      // Register handlers for the current context menu invocation
      pendingHandlers.clear();
      const items = contextCommands.map((c) => {
        const key = handlerKey(c.command);
        pendingHandlers.set(key, c.command);
        return { id: key, name: c.command.name };
      });

      invoke("show_context_menu", { items }).catch(console.error);
    },
    [contextCommands],
  );
}
