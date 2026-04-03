import { useCallback, useContext } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  CommandScopeContext,
  scopeChainFromScope,
} from "@/lib/command-scope";

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

/** Shape sent to the backend `show_context_menu`. Self-contained dispatch info. */
interface ContextMenuItem {
  name: string;
  cmd: string;
  target?: string;
  scope_chain: string[];
  separator: boolean;
}

/**
 * Hook that returns an onContextMenu handler.
 *
 * Scope chain comes from CommandScopeContext. Each menu item sent to the
 * backend carries its full dispatch info (cmd, target, scope_chain).
 * When the user selects an item, Rust dispatches directly — no round-trip.
 *
 * @returns Event handler to attach to onContextMenu.
 */
export function useContextMenu(): (e: React.MouseEvent) => void {
  const scope = useContext(CommandScopeContext);
  const scopeChain = scopeChainFromScope(scope);

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

          const items: ContextMenuItem[] = [];
          let lastGroup: string | null = null;

          for (const cmd of commands) {
            if (lastGroup !== null && cmd.group !== lastGroup) {
              items.push({
                name: "",
                cmd: "",
                separator: true,
                scope_chain: [],
              });
            }
            items.push({
              name: cmd.name,
              cmd: cmd.id,
              target: cmd.target,
              scope_chain: scopeChain,
              separator: false,
            });
            lastGroup = cmd.group;
          }

          invoke("show_context_menu", { items }).catch(console.error);
        })
        .catch(console.error);
    },
    [scopeChain],
  );
}
