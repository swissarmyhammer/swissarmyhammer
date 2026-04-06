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
    <div className="flex flex-col min-h-0 border border-border rounded-lg mb-2">
      <button
        type="button"
        aria-label={bucket.label}
        className="flex items-center gap-2 px-3 py-2 text-sm font-medium text-foreground hover:bg-muted/50 rounded-t-lg transition-colors w-full text-left"
        onClick={() => setCollapsed((c) => !c)}
      >
        {collapsed ? (
          <ChevronRight className="h-4 w-4 text-muted-foreground" />
        ) : (
          <ChevronDown className="h-4 w-4 text-muted-foreground" />
        )}
        <span>{bucket.label}</span>
        <Badge variant="secondary">{bucket.tasks.length}</Badge>
      </button>
      {!collapsed && (
        <div className="flex flex-col flex-1 min-h-0">
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
