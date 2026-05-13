/**
 * GroupSection renders a single collapsible group within a grouped board view.
 *
 * Each section shows a header with the group label, task count badge, and
 * a collapse/expand chevron. When expanded, it renders a full BoardView
 * containing only the tasks belonging to that group.
 */

import { useState } from "react";
import { ChevronDown, ChevronRight } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { BoardView } from "@/components/board-view";
import type { GroupBucket } from "@/lib/group-utils";
import type { BoardData } from "@/types/kanban";

interface GroupSectionProps {
  /** The group bucket containing the label and tasks for this section. */
  bucket: GroupBucket;
  /** Board data (columns, tags, etc.) shared across all sections. */
  board: BoardData;
  /** The field name being grouped by. */
  groupField: string;
}

/**
 * Tailwind class that gives an expanded group section a bounded, viewport-
 * relative height.
 *
 * Load-bearing for virtualization:
 *
 * `<ColumnView>` uses TanStack `useVirtualizer` with
 * `getScrollElement: () => scrollRef.current` where `scrollRef` is the
 * column's inner `flex-1 overflow-y-auto` div. The virtualizer measures
 * that element's `clientHeight` to compute how many cards intersect the
 * viewport. In the ungrouped path that flex chain bottoms out at the
 * app viewport (`h-screen`), so the column's scroll element ends up
 * bounded and the virtualizer windows correctly.
 *
 * In the grouped path the outer `<GroupedBoardView>` is itself
 * `overflow-y-auto` (vertical scroll between sections), and each
 * `<GroupSection>` is a `shrink-0` child of that vertical strip. Without
 * an explicit height on the section body, the inner `flex-1` chain has
 * no finite ancestor to flex against — `clientHeight` of the column's
 * scroll element collapses to the natural height of its content, which
 * for 2300 cards is several thousand pixels. The virtualizer concludes
 * "the viewport already shows everything" and mounts every card.
 *
 * Pinning the expanded body to a definite viewport-relative height
 * restores the bounded ancestor the virtualizer needs. 70vh leaves room
 * for the next section's header to peek into the viewport so the user
 * keeps the "multiple groups visible at once" affordance the grouped
 * view exists for.
 */
const EXPANDED_BODY_CLASS = "h-[70vh] min-h-0 flex flex-col";

/**
 * Collapsible group section that wraps a BoardView with only the group's tasks.
 *
 * Collapse state is local — each section starts expanded and can be toggled
 * independently by clicking the header.
 *
 * @param bucket - Group bucket with label and tasks.
 * @param board - Board data shared across all group sections.
 * @param groupField - The field name used for grouping.
 */
export function GroupSection({
  bucket,
  board,
  groupField: _groupField,
}: GroupSectionProps) {
  const [collapsed, setCollapsed] = useState(false);

  return (
    <div className="shrink-0">
      <div className="h-px bg-border mx-3" />
      <button
        type="button"
        aria-label={bucket.label}
        className="flex items-center gap-2 px-3 py-1 text-sm font-medium text-muted-foreground hover:text-foreground transition-colors w-full text-left"
        onClick={() => setCollapsed((c) => !c)}
      >
        {collapsed ? (
          <ChevronRight className="h-3.5 w-3.5" />
        ) : (
          <ChevronDown className="h-3.5 w-3.5" />
        )}
        <span>{bucket.label}</span>
        <Badge variant="secondary" className="text-xs px-1.5 py-0">
          {bucket.tasks.length}
        </Badge>
      </button>
      {!collapsed && (
        <div className={EXPANDED_BODY_CLASS} data-testid="group-section-body">
          <BoardView
            board={board}
            tasks={bucket.tasks}
            groupValue={bucket.value}
          />
        </div>
      )}
    </div>
  );
}
