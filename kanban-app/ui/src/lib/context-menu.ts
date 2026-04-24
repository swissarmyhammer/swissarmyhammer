import { useCallback, useContext } from "react";
import { invoke } from "@tauri-apps/api/core";
import { CommandScopeContext, scopeChainFromScope } from "@/lib/command-scope";

/** Shape returned by the backend `list_commands_for_scope`. */
interface ResolvedCommand {
  id: string;
  name: string;
  target?: string;
  group: string;
  context_menu: boolean;
  keys?: { vim?: string; cua?: string; emacs?: string };
  available: boolean;
  /**
   * Pre-filled dispatch arguments for fan-out palette rows (e.g. per-view
   * "Switch to X" emits `view.set` with `{ view_id: "..." }`). All current
   * fan-out rows are `context_menu: false`, so they never reach this
   * right-click surface; `ContextMenuItem` has no matching `args` field
   * and the loop below intentionally drops this. If a future fan-out row
   * opts into the context menu, `args` must be added to `ContextMenuItem`
   * (both TS and Rust `kanban-app/src/commands.rs`) and forwarded through
   * `show_context_menu` before that row can dispatch correctly.
   */
  args?: Record<string, unknown>;
}

/** Shape sent to the backend `show_context_menu`. Self-contained dispatch info. */
interface ContextMenuItem {
  name: string;
  cmd: string;
  target?: string;
  scope_chain: string[];
  separator: boolean;
}

// Note: `ContextMenuItem` has no `args` field — see `ResolvedCommand.args`
// above for why the palette's fan-out `args` is intentionally dropped here.

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
