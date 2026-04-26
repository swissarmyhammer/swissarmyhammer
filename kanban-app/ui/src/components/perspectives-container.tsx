/**
 * PerspectivesContainer owns the perspective system providers and tab bar.
 *
 * Owns:
 * - PerspectiveProvider (perspective list and active perspective state)
 * - PerspectiveTabBar (rendered once above the content well)
 * - ActivePerspectiveScope (per-window `perspective:<id>` moniker injected
 *   into the scope chain for every right-click / command dispatch that
 *   happens inside the view body)
 *
 * Hierarchy:
 * ```
 * BoardContainer
 *   └─ PerspectivesContainer                 ← this component
 *        ├─ PerspectiveTabBar (tab bar — each tab has its own
 *        │                     `perspective:<id>` moniker)
 *        └─ ActivePerspectiveScope            (view-body scope)
 *             └─ children (PerspectiveContainer > views)
 * ```
 */

import type { ReactNode } from "react";
import {
  usePerspectives,
  PerspectiveProvider,
} from "@/lib/perspective-context";
import { CommandScopeProvider } from "@/lib/command-scope";
import { moniker } from "@/lib/moniker";
import { PerspectiveTabBar } from "@/components/perspective-tab-bar";

interface PerspectivesContainerProps {
  children: ReactNode;
}

/**
 * Wrap the view body in a `perspective:<active-id>` scope when a perspective
 * is active. Right-clicks on grid rows, board columns, cells — anything
 * below the tab bar — now carry the active perspective's moniker in their
 * scope chain, so `resolve_perspective_id` on the backend picks
 * `ResolvedFrom::Scope` instead of falling through to `UiState`.
 *
 * When no perspective is active the children render without an extra
 * provider; the backend's `scope: "entity:perspective"` filter on
 * `perspective.*` commands hides them from right-click menus in that
 * (rare, transient) state, which is the correct behavior — there's
 * nothing to mutate.
 *
 * Kept as a tiny component rather than inlined in `PerspectivesContainer`
 * so the `usePerspectives()` hook lives under the `PerspectiveProvider`
 * the parent renders.
 */
function ActivePerspectiveScope({ children }: { children: ReactNode }) {
  const { activePerspective } = usePerspectives();
  if (!activePerspective) return <>{children}</>;
  return (
    <CommandScopeProvider
      moniker={moniker("perspective", activePerspective.id)}
    >
      {children}
    </CommandScopeProvider>
  );
}

/**
 * Wraps children in PerspectiveProvider and renders PerspectiveTabBar above
 * the content well. The tab bar is rendered once here rather than inside
 * each view component.
 *
 * The children sit inside `ActivePerspectiveScope` so every right-click /
 * command dispatch that originates from the view body carries the active
 * perspective's moniker in its scope chain (see `ActivePerspectiveScope`
 * for the full rationale).
 */
export function PerspectivesContainer({
  children,
}: PerspectivesContainerProps) {
  return (
    <PerspectiveProvider>
      <div className="flex flex-col flex-1 min-h-0 min-w-0">
        <PerspectiveTabBar />
        <ActivePerspectiveScope>{children}</ActivePerspectiveScope>
      </div>
    </PerspectiveProvider>
  );
}
