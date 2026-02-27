import { Inbox } from "lucide-react";
import { TaskCard } from "@/components/task-card";
import { Badge } from "@/components/ui/badge";
import type { Column, Task } from "@/types/kanban";

interface ColumnViewProps {
  column: Column;
  tasks: Task[];
  blockedIds: Set<string>;
  onTaskClick?: (task: Task) => void;
}

export function ColumnView({ column, tasks, blockedIds, onTaskClick }: ColumnViewProps) {
  // Sort tasks by ordinal within the column
  const sorted = [...tasks].sort((a, b) =>
    a.position.ordinal.localeCompare(b.position.ordinal)
  );

  return (
    <div className="flex flex-col min-h-0 flex-1">
      <div className="px-3 py-2 flex items-center justify-center gap-2">
        <h2 className="text-sm font-medium text-muted-foreground uppercase tracking-wide">
          {column.name}
        </h2>
        <Badge variant="secondary">{tasks.length}</Badge>
      </div>
      <div className="flex-1 overflow-y-auto px-2 pb-2 space-y-1.5">
        {sorted.length === 0 ? (
          <div className="flex flex-col items-center justify-center h-full text-muted-foreground opacity-40">
            <Inbox className="h-8 w-8 mb-2" />
            <p className="text-xs">No tasks</p>
          </div>
        ) : (
          sorted.map((task) => (
            <TaskCard
              key={task.id}
              task={task}
              isBlocked={blockedIds.has(task.id)}
              onClick={onTaskClick}
            />
          ))
        )}
      </div>
    </div>
  );
}
