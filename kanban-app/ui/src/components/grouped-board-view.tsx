/**
 * GroupedBoardView conditionally wraps BoardView with group sections.
 *
 * When a group field is active (from the perspective), it computes group
 * buckets and renders a vertical stack of collapsible GroupSection components,
 * each containing a full column layout for that group's tasks.
 *
 * When no grouping is active, it renders BoardView directly with no
 * visual change.
 */

import { useMemo } from "react";
import { useActivePerspective } from "@/components/perspective-container";
import { useSchema } from "@/lib/schema-context";
import { BoardView } from "@/components/board-view";
import { GroupSection } from "@/components/group-section";
import { computeGroups } from "@/lib/group-utils";
import type { BoardData, Entity } from "@/types/kanban";

interface GroupedBoardViewProps {
  /** Board data (columns, tags, summary). */
  board: BoardData;
  /** All task entities — filtering is handled by BoardView/perspective. */
  tasks: Entity[];
}

/**
 * Renders the board with optional grouping.
 *
 * Reads `groupField` from the active perspective. When no groupField is set,
 * delegates directly to BoardView. When grouping is active, computes group
 * buckets via `computeGroups` and renders a GroupSection per bucket.
 *
 * @param board - Board data shared across all sections.
 * @param tasks - All task entities for the board.
 */
export function GroupedBoardView({ board, tasks }: GroupedBoardViewProps) {
  const { groupField } = useActivePerspective();
  const { getSchema } = useSchema();

  const fieldDefs = useMemo(() => {
    const schema = getSchema("task");
    return schema?.fields ?? [];
  }, [getSchema]);

  const groups = useMemo(() => {
    if (!groupField) return null;
    return computeGroups(tasks, groupField, fieldDefs);
  }, [tasks, groupField, fieldDefs]);

  // No grouping active — render the plain board view
  if (!groups) {
    return <BoardView board={board} tasks={tasks} />;
  }

  // Grouped view — vertical stack of collapsible sections
  return (
    <div className="flex flex-col flex-1 min-h-0 overflow-y-auto">
      {groups.map((bucket) => (
        <GroupSection
          key={bucket.value}
          bucket={bucket}
          board={board}
          groupField={groupField}
        />
      ))}
    </div>
  );
}
