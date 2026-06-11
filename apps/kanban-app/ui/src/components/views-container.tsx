/**
 * ViewsContainer owns the view system providers and sidebar navigation.
 *
 * Owns:
 * - ViewsProvider (view list and active view state)
 * - LeftNav sidebar presenter
 * - Flex layout wrapping LeftNav + children
 *
 * View switching is fully canonical — there is no client-minted command id
 * involved (card 01KTED8XDX4728QR4WT9EZ0WRF removed the last
 * `view.switch:${id}` indirection):
 *
 * - Selecting a view (LeftNav click / Enter) dispatches the canonical
 *   `view.set` command with the view id in `args.view_id`. The command is
 *   defined in `builtin/plugins/ui-commands/index.ts`.
 * - The palette's per-view "Switch to <ViewName>" rows are emitted by Rust
 *   (`swissarmyhammer_kanban::scope_commands::emit_view_switch`) as
 *   `view.set` rows with pre-filled args.
 * - Per-view scope bookkeeping lives in presentation: LeftNav wraps each
 *   view button in a `CommandScopeProvider` carrying the `view:{id}`
 *   moniker (see `ScopedViewButton` in `left-nav.tsx`).
 *
 * Hierarchy:
 * ```
 * BoardContainer
 *   └─ ViewsContainer           ← this component
 *        ├─ LeftNav (sidebar)
 *        └─ children (ViewContainer)
 * ```
 */

import { type ReactNode } from "react";
import { ViewsProvider } from "@/lib/views-context";
import { LeftNav } from "@/components/left-nav";

interface ViewsContainerProps {
  children: ReactNode;
}

/**
 * Top-level views container that provides view state and sidebar navigation.
 *
 * Wraps children in ViewsProvider so all descendants have access to the
 * views list and active view, and renders LeftNav as a sidebar.
 */
export function ViewsContainer({ children }: ViewsContainerProps) {
  return (
    <ViewsProvider>
      <div className="flex-1 flex min-h-0 min-w-0">
        <LeftNav />
        {children}
      </div>
    </ViewsProvider>
  );
}
