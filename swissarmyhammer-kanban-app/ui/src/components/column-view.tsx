import { TaskCard } from "@/components/task-card";
import type { Column, Task } from "@/types/kanban";

interface ColumnViewProps {
  column: Column;
  tasks: Task[];
  blockedIds: Set<string>;
}

export function ColumnView({ column, tasks, blockedIds }: ColumnViewProps) {
  // Sort tasks by ordinal within the column
  const sorted = [...tasks].sort((a, b) =>
    a.position.ordinal.localeCompare(b.position.ordinal)
  );

  return (
    <div className="flex flex-col min-h-0 flex-1">
      <div className="px-3 py-2 text-center">
        <h2 className="text-sm font-medium text-muted-foreground uppercase tracking-wide">
          {column.name}
        </h2>
      </div>
      <div className="flex-1 overflow-y-auto px-2 pb-2 space-y-1.5">
        {sorted.map((task) => (
          <TaskCard
            key={task.id}
            task={task}
            isBlocked={blockedIds.has(task.id)}
          />
        ))}
      </div>
    </div>
  );
}
