import { useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";

/**
 * Module-level map of pending context menu command IDs.
 * Only one context menu is open at a time, so this is safe.
 */
const pendingCommandIds = new Map<string, string>();

/**
 * Dispatch a context menu command by menu item ID.
 * Called by the global event listener when the user selects a menu item.
 *
 * @param menuItemId - The menu item ID selected from the native context menu.
 * @returns true if the command was found and dispatched.
 */
export async function dispatchContextMenuCommand(
  menuItemId: string,
): Promise<boolean> {
  const cmdId = pendingCommandIds.get(menuItemId);
  if (!cmdId) return false;
  try {
    await invoke("dispatch_command", { cmd: cmdId });
  } catch (e) {
    console.error("context menu dispatch failed:", e);
  }
  return true;
}

/**
 * Hook that returns an onContextMenu handler.
 *
 * When fired, it passes the scope chain to the backend which returns
 * the available context menu commands. The backend is the single source
 * of truth for command availability, names, and filtering.
 *
 * @param scopeChain - The scope chain monikers from the focused entity to root.
 */
export function useContextMenu(
  scopeChain: string[],
): (e: React.MouseEvent) => void {
  return useCallback(
    (e: React.MouseEvent) => {
      e.preventDefault();
      e.stopPropagation();

      // The backend computes everything: availability, names, filtering.
      invoke<Array<{ id: string; name: string }>>("list_available_commands", {
        contextMenu: true,
        scopeChain,
      })
        .then((commands) => {
          if (commands.length === 0) return;

          pendingCommandIds.clear();
          const items: Array<{ id: string; name: string }> = [];

          for (const cmd of commands) {
            pendingCommandIds.set(cmd.id, cmd.id);
            items.push({ id: cmd.id, name: cmd.name });
          }

          invoke("show_context_menu", { items }).catch(console.error);
        })
        .catch(console.error);
    },
    [scopeChain],
  );
}
