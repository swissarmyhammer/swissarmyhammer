import { useMemo } from "react";
import { ColumnView } from "@/components/column-view";
import type { Board, Task } from "@/types/kanban";

interface BoardViewProps {
  board: Board;
  tasks: Task[];
}

export function BoardView({ board, tasks }: BoardViewProps) {
  const columns = useMemo(
    () => [...board.columns].sort((a, b) => a.order - b.order),
    [board.columns]
  );

  // Group tasks by column
  const tasksByColumn = useMemo(() => {
    const map = new Map<string, Task[]>();
    for (const col of columns) {
      map.set(col.id, []);
    }
    for (const task of tasks) {
      const list = map.get(task.position.column);
      if (list) {
        list.push(task);
      }
    }
    return map;
  }, [columns, tasks]);

  // Collect blocked task IDs
  const blockedIds = useMemo(() => {
    const set = new Set<string>();
    for (const task of tasks) {
      if (task.depends_on.length > 0) {
        // Check if any dependency is not in the terminal column
        const terminalCol = columns[columns.length - 1]?.id;
        const hasIncomplete = task.depends_on.some((depId) => {
          const dep = tasks.find((t) => t.id === depId);
          return dep && dep.position.column !== terminalCol;
        });
        if (hasIncomplete) {
          set.add(task.id);
        }
      }
    }
    return set;
  }, [tasks, columns]);

  return (
    <div className="flex flex-1 min-h-0">
      {columns.map((col, i) => (
        <div key={col.id} className="flex flex-1 min-w-0">
          {i > 0 && <div className="w-px bg-border shrink-0 my-3" />}
          <ColumnView
            column={col}
            tasks={tasksByColumn.get(col.id) ?? []}
            blockedIds={blockedIds}
          />
        </div>
      ))}
    </div>
  );
}
