/**
 * ViewsContainer owns the view system providers and sidebar navigation.
 *
 * Owns:
 * - ViewsProvider (view list and active view state)
 * - CommandScopeProvider with one `view.switch:{id}` client-side command
 *   per known view — each dispatches the canonical `view.set` command
 *   with `view_id` pre-filled in `args`. The `view.switch:{id}` id is a
 *   stable per-view identifier for scope-chain bookkeeping and React key
 *   purposes; it is NOT a command id the backend recognises. The dynamic
 *   palette entries that surface "Switch to <ViewName>" rows come from
 *   `swissarmyhammer_kanban::scope_commands::emit_view_switch` and already
 *   emit `view.set` directly.
 * - LeftNav sidebar presenter
 * - Flex layout wrapping LeftNav + children
 *
 * Hierarchy:
 * ```
 * BoardContainer
 *   └─ ViewsContainer           ← this component
 *        ├─ LeftNav (sidebar)
 *        └─ children (ViewContainer)
 * ```
 */

import { useMemo, type ReactNode } from "react";
import { ViewsProvider, useViews } from "@/lib/views-context";
import { LeftNav } from "@/components/left-nav";
import {
  CommandScopeProvider,
  useDispatchCommand,
  type CommandDef,
} from "@/lib/command-scope";

// ---------------------------------------------------------------------------
// Inner component that reads views context (must be inside ViewsProvider)
// ---------------------------------------------------------------------------

/**
 * Generates per-view command definitions and wraps children in a
 * CommandScopeProvider + flex layout with LeftNav.
 *
 * Each registered command uses a per-view id (`view.switch:{id}`) as its
 * scope-map key so multiple views coexist in the same scope; the execute
 * handler dispatches the canonical `view.set` command with `view_id` in
 * `args`. The `view.switch:{id}` id is NOT sent to the backend — it is
 * purely a client-side identifier for the scope map / React key. The old
 * dispatcher-side rewrite from `view.switch:*` to `view.set` was retired
 * in 01KPZMXXEXKVE3RNPA4XJP0105.
 */
function ViewsCommandScope({ children }: { children: ReactNode }) {
  const { views } = useViews();
  const dispatch = useDispatchCommand("view.set");

  const viewCommands: CommandDef[] = useMemo(() => {
    return views.map((view) => ({
      id: `view.switch:${view.id}`,
      name: `View: ${view.name}`,
      execute: () => {
        dispatch({ args: { view_id: view.id } }).catch(console.error);
      },
    }));
  }, [views, dispatch]);

  return (
    <CommandScopeProvider commands={viewCommands}>
      <div className="flex-1 flex min-h-0 min-w-0">
        <LeftNav />
        {children}
      </div>
    </CommandScopeProvider>
  );
}

// ---------------------------------------------------------------------------
// ViewsContainer
// ---------------------------------------------------------------------------

interface ViewsContainerProps {
  children: ReactNode;
}

/**
 * Top-level views container that provides view state and sidebar navigation.
 *
 * Wraps children in ViewsProvider so all descendants have access to the
 * views list and active view. Registers `view.switch:{id}` commands for
 * each view and renders LeftNav as a sidebar.
 */
export function ViewsContainer({ children }: ViewsContainerProps) {
  return (
    <ViewsProvider>
      <ViewsCommandScope>{children}</ViewsCommandScope>
    </ViewsProvider>
  );
}
