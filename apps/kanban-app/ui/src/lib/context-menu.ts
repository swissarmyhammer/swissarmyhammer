import { useCallback, useContext, useMemo, useRef } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { callMcpTool } from "@/lib/mcp-transport";
import { CommandScopeContext, scopeChainFromScope } from "@/lib/command-scope";
import { useCommandList, type CommandMetadata } from "@/hooks/use-command-list";

/** Shape sent to the `window` server's `show context menu` op. Self-contained dispatch info. */
interface ContextMenuItem {
  name: string;
  cmd: string;
  target?: string;
  scope_chain: string[];
  separator: boolean;
}

/**
 * Expand a scope-chain moniker (`type:id`, e.g. `task:abc`,
 * `attachment:/p.png`) into the scope expressions a command may declare to
 * match it.
 *
 * Two conventions coexist in the YAML command definitions:
 *   - entity-typed scopes — `entity:<type>` (e.g. `entity:task`), and
 *   - bare-namespace scopes — `<type>` (e.g. `attachment`).
 *
 * A moniker therefore matches a command scoped to either form, so each chain
 * moniker contributes both `entity:<type>` and the bare `<type>` (plus the raw
 * moniker itself, for any scope expressed as a literal moniker).
 */
function scopeExpressionsForMoniker(moniker: string): string[] {
  // Scope-chain entries are bare monikers `type:id` — the id may itself be a
  // path (`attachment:/foo/bar.png`), so the type is the token before the
  // FIRST colon. A command may declare its scope as the entity-typed form
  // (`entity:<type>`) or the bare namespace (`<type>`), so emit both.
  const colon = moniker.indexOf(":");
  if (colon <= 0) return [moniker];
  const type = moniker.slice(0, colon);
  return [moniker, `entity:${type}`, type];
}

/**
 * Whether a command's `scope` matches the active scope chain.
 *
 * A command with no `scope` (empty/absent) is global and always matches. A
 * scoped command matches when any of its scope expressions is admitted by some
 * moniker in the chain — the React-side equivalent of `list command`'s
 * "global OR contained" scope rule, generalized across the whole chain so a
 * `task`-scoped command surfaces whether the focused leaf is the task itself
 * or a descendant.
 */
function scopeMatches(cmd: CommandMetadata, chain: string[]): boolean {
  if (!cmd.scope || cmd.scope.length === 0) return true;
  const admitted = new Set<string>();
  for (const moniker of chain) {
    for (const expr of scopeExpressionsForMoniker(moniker)) admitted.add(expr);
  }
  return cmd.scope.some((expr) => admitted.has(expr));
}

/**
 * Sort key for context-menu ordering, mirroring the registry's grouping:
 * primary by `context_menu_group`, secondary by `context_menu_order`. Absent
 * values sort last so explicitly-ordered commands lead.
 */
function contextMenuSortKey(cmd: CommandMetadata): [number, number] {
  return [
    cmd.context_menu_group ?? Number.MAX_SAFE_INTEGER,
    cmd.context_menu_order ?? Number.MAX_SAFE_INTEGER,
  ];
}

/**
 * Hook that returns an onContextMenu handler.
 *
 * Commands are sourced from the metadata-driven Command registry via
 * {@link useCommandList} — no command id list is hardcoded here. The handler
 * filters that live list for `context_menu: true` commands whose `scope`
 * matches the right-click point's scope chain, sorts them into context-menu
 * groups, and hands self-contained dispatch items to the native menu. When the
 * registry changes (`commands/changed`), `useCommandList` re-fetches and the
 * next right-click sees the new set.
 *
 * The hook is called from high-multiplier render sites (one per grid cell, one
 * per data-table row, one per grid body) so both the returned handler and the
 * scope-chain walk must stay off the render hot path:
 *
 * - The handler is memoised with empty deps so its identity is stable across
 *   renders. Downstream components memoised on prop identity keep their
 *   skip-children fast path.
 * - The current scope and the latest command list are kept in refs, updated
 *   every render. The handler reads them at click time, so the scope-chain
 *   walk and the filter run exactly once per right-click — never on render.
 *
 * @returns Event handler to attach to onContextMenu.
 */
export function useContextMenu(): (e: React.MouseEvent) => void {
  const scope = useContext(CommandScopeContext);
  const scopeRef = useRef(scope);
  scopeRef.current = scope;

  // Only `context_menu`-tagged commands can ever reach this surface, so narrow
  // the live registry to them. Scope matching happens at click time against
  // the right-click point's chain (the hook itself is scope-agnostic).
  const { commands } = useCommandList();
  const contextMenuCommands = useMemo(
    () => commands.filter((cmd) => cmd.context_menu === true),
    [commands],
  );
  const commandsRef = useRef(contextMenuCommands);
  commandsRef.current = contextMenuCommands;

  return useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();

    // Walk the scope chain at click time, not at render time. `scopeRef`
    // is written on every commit, so this always reflects the scope that
    // was committed at the most recent render.
    const scopeChain = scopeChainFromScope(scopeRef.current);

    const matching = commandsRef.current
      .filter((cmd) => scopeMatches(cmd, scopeChain))
      .sort((a, b) => {
        const [ag, ao] = contextMenuSortKey(a);
        const [bg, bo] = contextMenuSortKey(b);
        return ag !== bg ? ag - bg : ao - bo;
      });

    if (matching.length === 0) return;

    const items: ContextMenuItem[] = [];
    let lastGroup: number | undefined;
    for (const cmd of matching) {
      const group = cmd.context_menu_group;
      if (lastGroup !== undefined && group !== lastGroup && items.length > 0) {
        items.push({ name: "", cmd: "", separator: true, scope_chain: [] });
      }
      items.push({
        name: cmd.menu_name ?? cmd.name,
        cmd: cmd.id,
        // The innermost chain moniker is the entity the right-click targets;
        // the dispatcher resolves the command against it (and the full
        // `scope_chain` rides alongside for backend scope resolution).
        target: scopeChain[0],
        scope_chain: scopeChain,
        separator: false,
      });
      lastGroup = group;
    }

    // Render the native context menu via the app-wide `window` MCP server.
    // Pass our own window label so the shell pops the menu on the *calling*
    // window (deterministic targeting; the MCP wire has no ambient "calling
    // window" the old native command relied on). Selection delivery is
    // unchanged: the Rust menu-event handler decodes the chosen item and emits
    // `context-menu-command`, which `KeybindingHandler` dispatches — so this
    // call is fire-and-forget, exactly as the prior `invoke("show_context_menu",
    // …)` was.
    const windowLabel = getCurrentWindow().label;
    callMcpTool("window", "show context menu", {
      items,
      window_label: windowLabel,
    }).catch(console.error);
  }, []);
}
