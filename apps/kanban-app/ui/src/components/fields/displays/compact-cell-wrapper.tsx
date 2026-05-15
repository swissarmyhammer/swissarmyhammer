/**
 * CompactCellWrapper — fixed-height shell for compact-mode display output.
 *
 * Every compact-mode field display in a virtualized grid row must render
 * at the same pixel height; otherwise rows visibly jitter and the
 * virtualizer's fixed `ROW_HEIGHT` estimate (see
 * `data-table.tsx::ROW_HEIGHT`) drifts away from actual rendered
 * positions. This wrapper enforces that contract structurally with
 * `h-6 flex items-center` and clips overflow so taller content (CM6
 * mention pills, large icons) cannot break the layout invariant.
 *
 * Used by:
 * - `AvatarDisplay` — populated avatars + empty placeholder span
 * - `BadgeListDisplay` — populated mention pills + empty placeholder span
 * - `BadgeDisplay` — populated mention pill + empty placeholder span
 * - `AttachmentDisplay`, `AttachmentListDisplay` — populated attachment
 *   row + empty placeholder span
 * - `ProgressRingDisplay` — populated radial chart (taller than text-line)
 *
 * The `data-compact-cell="true"` attribute is the public hook tests use
 * to assert that populated and empty branches share the same wrapper.
 *
 * Not used in `mode="full"`: inspector rows have their own height
 * accounting and can render at the natural content height.
 */

import { cn } from "@/lib/utils";
import type { ReactNode } from "react";

/**
 * Fixed compact-mode pixel height in Tailwind units. `h-6` = 1.5rem = 24px.
 * Combined with the row's `py-1.5` padding (12px), this yields a 36px row
 * total — see {@link COMPACT_ROW_HEIGHT_PX} (the single source of truth
 * imported by `data-table.tsx::ROW_HEIGHT`). Update both together if the
 * wrapper height ever changes.
 */
export const COMPACT_CELL_HEIGHT_CLASS = "h-6";

/**
 * Total pixel height of a compact-mode data row.
 *
 * Math: 24px wrapper content (`COMPACT_CELL_HEIGHT_CLASS = "h-6"` =
 * 1.5rem) + 12px row padding (`py-1.5` = 6px × 2). Imported by
 * `data-table.tsx::ROW_HEIGHT` so the row-virtualizer's fixed-height
 * estimate is derived from this single source of truth — change the
 * wrapper class and this constant together, and the data table picks
 * up the new height automatically.
 */
export const COMPACT_ROW_HEIGHT_PX = 36;

/** Props for {@link CompactCellWrapper}. */
export interface CompactCellWrapperProps {
  /** Display content to render inside the fixed-height shell. */
  children: ReactNode;
  /**
   * Optional extra classes appended after the height-fixing utilities.
   * Use sparingly — anything that changes height/padding breaks the
   * row-virtualization invariant. Reserved for things like text alignment
   * (`justify-end` for right-aligned numbers) that don't affect height.
   */
  className?: string;
}

/**
 * Wrap compact-mode display output in a fixed-height flex shell.
 *
 * @param props - {@link CompactCellWrapperProps}
 * @returns A `<div>` with a stable height contract for grid cells.
 */
export function CompactCellWrapper({
  children,
  className,
}: CompactCellWrapperProps) {
  return (
    <div
      data-compact-cell="true"
      className={cn(
        COMPACT_CELL_HEIGHT_CLASS,
        "flex items-center overflow-hidden",
        className,
      )}
    >
      {children}
    </div>
  );
}
