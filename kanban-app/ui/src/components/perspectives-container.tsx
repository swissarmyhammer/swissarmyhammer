/**
 * PerspectivesContainer owns the perspective system providers and tab bar.
 *
 * Owns:
 * - PerspectiveProvider (perspective list and active perspective state)
 * - PerspectiveTabBar (rendered once above the content well)
 *
 * Hierarchy:
 * ```
 * BoardContainer
 *   └─ PerspectivesContainer         ← this component
 *        ├─ PerspectiveTabBar (tab bar)
 *        └─ children (PerspectiveContainer > views)
 * ```
 */

import type { ReactNode } from "react";
import { PerspectiveProvider } from "@/lib/perspective-context";
import { PerspectiveTabBar } from "@/components/perspective-tab-bar";

interface PerspectivesContainerProps {
  children: ReactNode;
}

/**
 * Wraps children in PerspectiveProvider and renders PerspectiveTabBar above
 * the content well. The tab bar is rendered once here rather than inside
 * each view component.
 */
export function PerspectivesContainer({
  children,
}: PerspectivesContainerProps) {
  return (
    <PerspectiveProvider>
      <div className="flex flex-col flex-1 min-h-0 min-w-0">
        <PerspectiveTabBar />
        {children}
      </div>
    </PerspectiveProvider>
  );
}
