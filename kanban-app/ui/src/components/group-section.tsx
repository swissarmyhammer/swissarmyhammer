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
 * Collapsible group section that wraps a BoardView with only the group's tasks.
 *
 * Collapse state is local — each section starts expanded and can be toggled
 * independently by clicking the header.
 *
 * @param bucket - Group bucket with label and tasks.
 * @param board - Board data shared across all group sections.
 * @param groupField - The field name used for grouping.
 */
export function GroupSection({ bucket, board, groupField }: GroupSectionProps) {
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
        <div className="flex-1 min-h-0 overflow-auto">
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
