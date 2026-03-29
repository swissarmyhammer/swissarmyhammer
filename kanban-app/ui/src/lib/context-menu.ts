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
 */
const pendingHandlers = new Map<string, CommandDef>();

/**
 * Build the pendingHandlers key for a command.
 */
function handlerKey(cmd: CommandDef): string {
  return cmd.target ? `${cmd.id}:${cmd.target}` : cmd.id;
}

/**
 * Dispatch a context menu command by handler key.
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
 * The frontend scope chain provides the command list (with targets and
 * execute handlers). The backend provides availability filtering via
 * `list_available_commands`. This gives us both:
 * - Multiple entries for polymorphic commands (Copy Tag + Copy Task)
 * - Correct availability (paste only when clipboard has content)
 *
 * @param scopeChain - Monikers from focused entity to root, passed to backend.
 */
export function useContextMenu(
  scopeChain: string[],
): (e: React.MouseEvent) => void {
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

      // Backend checks availability (clipboard state, scope requirements, etc.)
      invoke<Array<{ id: string }>>("list_available_commands", {
        contextMenu: true,
        scopeChain,
      })
        .then((available) => {
          const availableIds = new Set(available.map((c) => c.id));

          // Keep frontend commands that the backend says are available.
          // Frontend has per-target entries (Copy Tag, Copy Task) — both show
          // if the backend says entity.copy is available.
          const filtered = contextCommands.filter((c) =>
            availableIds.has(c.command.id),
          );

          if (filtered.length === 0) return;

          pendingHandlers.clear();
          const items: Array<{ id: string; name: string }> = [];
          let lastDepth: number | null = null;

          for (const c of filtered) {
            if (lastDepth !== null && c.depth !== lastDepth) {
              items.push({ id: "__separator__", name: "" });
            }
            const key = handlerKey(c.command);
            pendingHandlers.set(key, c.command);
            items.push({ id: key, name: c.command.name });
            lastDepth = c.depth;
          }

          invoke("show_context_menu", { items }).catch(console.error);
        })
        .catch(console.error);
    },
    [contextCommands, scopeChain],
  );
}
