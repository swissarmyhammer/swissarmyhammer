import { useMemo } from "react";
import {
  SortableContext,
  verticalListSortingStrategy,
} from "@dnd-kit/sortable";
import { useDroppable } from "@dnd-kit/core";
import { Inbox, Plus } from "lucide-react";
import { EditableMarkdown } from "@/components/editable-markdown";
import { SortableTaskCard } from "@/components/sortable-task-card";
import { Badge } from "@/components/ui/badge";
import type { Column, Task } from "@/types/kanban";

interface ColumnViewProps {
  column: Column;
  tasks: Task[];
  blockedIds: Set<string>;
  onTaskClick?: (task: Task) => void;
  onAddTask?: (columnId: string) => void;
  onRenameColumn?: (columnId: string, name: string) => void;
  /** When true, tasks are already in display order (virtual layout during drag) */
  presorted?: boolean;
}

export function ColumnView({ column, tasks, blockedIds, onTaskClick, onAddTask, onRenameColumn, presorted }: ColumnViewProps) {
  const sorted = useMemo(
    () =>
      presorted
        ? tasks
        : [...tasks].sort((a, b) => a.position.ordinal.localeCompare(b.position.ordinal)),
    [tasks, presorted]
  );

  const taskIds = useMemo(() => sorted.map((t) => t.id), [sorted]);

  // Use a prefixed ID so the task drop zone doesn't collide with
  // the column's sortable ID registered by SortableColumn.
  const { setNodeRef } = useDroppable({ id: `drop:${column.id}` });

  return (
    <div className="flex flex-col min-h-0 flex-1">
      <div className="px-3 py-2 flex items-center gap-2">
        <EditableMarkdown
          value={column.name}
          onCommit={(name) => onRenameColumn?.(column.id, name)}
          className="text-sm font-semibold text-foreground cursor-text"
          inputClassName="text-sm font-semibold text-foreground bg-transparent border-b border-ring w-full"
        />
        <Badge variant="secondary">{tasks.length}</Badge>
        <div className="flex-1" />
        <button
          type="button"
          className="p-0.5 rounded text-muted-foreground/50 hover:text-muted-foreground hover:bg-muted transition-colors"
          onClick={() => onAddTask?.(column.id)}
          title={`Add task to ${column.name}`}
        >
          <Plus className="h-4 w-4" />
        </button>
      </div>
      <div ref={setNodeRef} className="flex-1 overflow-y-auto px-2 pb-2 space-y-1.5">
        <SortableContext items={taskIds} strategy={verticalListSortingStrategy}>
          {sorted.length === 0 ? (
            <div className="flex flex-col items-center justify-center h-full text-muted-foreground opacity-40">
              <Inbox className="h-8 w-8 mb-2" />
              <p className="text-xs">No tasks</p>
            </div>
          ) : (
            sorted.map((task) => (
              <SortableTaskCard
                key={task.id}
                task={task}
                isBlocked={blockedIds.has(task.id)}
                onClick={onTaskClick}
              />
            ))
          )}
        </SortableContext>
      </div>
    </div>
  );
}
