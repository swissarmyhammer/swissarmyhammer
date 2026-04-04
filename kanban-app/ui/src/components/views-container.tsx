/**
 * ViewsContainer owns the view system providers and sidebar navigation.
 *
 * Owns:
 * - ViewsProvider (view list and active view state)
 * - CommandScopeProvider with dynamic `view.switch:{id}` commands
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
import { getCurrentWindow } from "@tauri-apps/api/window";
import { ViewsProvider, useViews } from "@/lib/views-context";
import { LeftNav } from "@/components/left-nav";
import {
  CommandScopeProvider,
  backendDispatch,
  type CommandDef,
} from "@/lib/command-scope";

/** Window label for scope chain references. */
const WINDOW_LABEL = getCurrentWindow().label;

// ---------------------------------------------------------------------------
// Inner component that reads views context (must be inside ViewsProvider)
// ---------------------------------------------------------------------------

/**
 * Generates view.switch command definitions from the views registry and
 * wraps children in a CommandScopeProvider + flex layout with LeftNav.
 */
function ViewsCommandScope({ children }: { children: ReactNode }) {
  const { views } = useViews();

  const viewCommands: CommandDef[] = useMemo(() => {
    return views.map((view) => ({
      id: `view.switch:${view.id}`,
      name: `View: ${view.name}`,
      execute: () => {
        backendDispatch({
          cmd: `view.switch:${view.id}`,
          scopeChain: [`window:${WINDOW_LABEL}`],
        }).catch(console.error);
      },
    }));
  }, [views]);

  return (
    <CommandScopeProvider commands={viewCommands}>
      <LeftNav />
      {children}
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
