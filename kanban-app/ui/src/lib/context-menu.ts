import { useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";

/** Shape returned by the backend `list_commands_for_scope`. */
interface ResolvedCommand {
  id: string;
  name: string;
  target?: string;
  group: string;
  context_menu: boolean;
  keys?: { vim?: string; cua?: string; emacs?: string };
  available: boolean;
}

/**
 * Module-level map of pending context menu commands.
 * Only one context menu is open at a time, so this is safe.
 * Maps menu item key → { cmd id, target } for dispatch.
 */
const pendingCommands = new Map<string, { id: string; target?: string }>();

/**
 * Dispatch a context menu command by menu item key.
 * Called by the global event listener when the user selects a menu item.
 */
export async function dispatchContextMenuCommand(
  menuItemKey: string,
): Promise<boolean> {
  const cmd = pendingCommands.get(menuItemKey);
  if (!cmd) return false;
  try {
    await invoke("dispatch_command", {
      cmd: cmd.id,
      target: cmd.target,
    });
  } catch (e) {
    console.error("context menu dispatch failed:", e);
  }
  return true;
}

/**
 * Hook that returns an onContextMenu handler.
 *
 * The backend is the single source of truth — it computes available commands,
 * resolves names, checks clipboard state, and handles dedup. The frontend
 * just renders what it gets back and dispatches on click.
 *
 * @param scopeChain - Monikers from focused entity to root.
 */
export function useContextMenu(
  scopeChain: string[],
): (e: React.MouseEvent) => void {
  return useCallback(
    (e: React.MouseEvent) => {
      e.preventDefault();
      e.stopPropagation();

      invoke<ResolvedCommand[]>("list_commands_for_scope", {
        scopeChain,
        contextMenu: true,
      })
        .then((commands) => {
          if (commands.length === 0) return;

          pendingCommands.clear();
          const items: Array<{ id: string; name: string }> = [];
          let lastGroup: string | null = null;

          for (const cmd of commands) {
            // Insert separator between groups (e.g. tag commands vs task commands)
            if (lastGroup !== null && cmd.group !== lastGroup) {
              items.push({ id: "__separator__", name: "" });
            }
            const key = cmd.target ? `${cmd.id}:${cmd.target}` : cmd.id;
            pendingCommands.set(key, { id: cmd.id, target: cmd.target });
            items.push({ id: key, name: cmd.name });
            lastGroup = cmd.group;
          }

          invoke("show_context_menu", { items }).catch(console.error);
        })
        .catch(console.error);
    },
    [scopeChain],
  );
}
