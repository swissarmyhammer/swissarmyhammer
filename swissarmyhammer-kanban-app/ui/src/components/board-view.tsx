import { useCallback, useMemo } from "react";
import {
  DndContext,
  closestCenter,
  PointerSensor,
  useSensor,
  useSensors,
  type DragEndEvent,
} from "@dnd-kit/core";
import { arrayMove } from "@dnd-kit/sortable";
import { invoke } from "@tauri-apps/api/core";
import { ColumnView } from "@/components/column-view";
import type { Board, Task } from "@/types/kanban";

interface BoardViewProps {
  board: Board;
  tasks: Task[];
  onTaskClick?: (task: Task) => void;
  onTaskMoved?: () => void;
}

export function BoardView({ board, tasks, onTaskClick, onTaskMoved }: BoardViewProps) {
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

  const sensors = useSensors(
    useSensor(PointerSensor, {
      activationConstraint: { distance: 5 },
    })
  );

  const handleDragEnd = useCallback(
    async (event: DragEndEvent) => {
      const { active, over } = event;
      if (!over || active.id === over.id) return;

      // Find the column containing the dragged task
      const activeTask = tasks.find((t) => t.id === active.id);
      if (!activeTask) return;

      const columnId = activeTask.position.column;
      const columnTasks = tasksByColumn.get(columnId);
      if (!columnTasks) return;

      const sorted = [...columnTasks].sort((a, b) =>
        a.position.ordinal.localeCompare(b.position.ordinal)
      );

      const oldIndex = sorted.findIndex((t) => t.id === active.id);
      const newIndex = sorted.findIndex((t) => t.id === over.id);
      if (oldIndex === -1 || newIndex === -1) return;

      const reordered = arrayMove(sorted, oldIndex, newIndex);

      // Calculate a new ordinal based on neighbors
      const prev = reordered[newIndex - 1]?.position.ordinal;
      const next = reordered[newIndex + 1]?.position.ordinal;

      // Compute ordinal for the moved task
      let ordinal: string;
      if (!prev && next) {
        // Moving to the top — generate ordinal before the next item
        // Use a simple approach: prepend character before the next ordinal
        const code = next.charCodeAt(0);
        ordinal = code > 97 ? String.fromCharCode(code - 1) + "0" : "a0";
        if (ordinal >= next) ordinal = "a0";
      } else if (prev && !next) {
        // Moving to the bottom
        const lastChar = prev.charCodeAt(prev.length - 1);
        ordinal = prev.slice(0, -1) + String.fromCharCode(lastChar + 1);
      } else if (prev && next) {
        // Moving between two items — find midpoint
        ordinal = midpointOrdinal(prev, next);
      } else {
        ordinal = "a0";
      }

      try {
        await invoke("move_task", {
          id: active.id as string,
          column: columnId,
          ordinal,
          swimlane: activeTask.position.swimlane ?? null,
        });
        onTaskMoved?.();
      } catch (e) {
        console.error("Failed to reorder task:", e);
      }
    },
    [tasks, tasksByColumn, onTaskMoved]
  );

  return (
    <DndContext
      sensors={sensors}
      collisionDetection={closestCenter}
      onDragEnd={handleDragEnd}
    >
      <div className="flex flex-1 min-h-0 overflow-x-auto">
        {columns.map((col, i) => (
          <div key={col.id} className="flex flex-1 min-w-[20em] max-w-[60em]">
            {i > 0 && <div className="w-px bg-border shrink-0 my-3" />}
            <ColumnView
              column={col}
              tasks={tasksByColumn.get(col.id) ?? []}
              blockedIds={blockedIds}
              onTaskClick={onTaskClick}
            />
          </div>
        ))}
      </div>
    </DndContext>
  );
}

/** Compute a string midpoint between two ordinals for fractional indexing. */
function midpointOrdinal(before: string, after: string): string {
  const maxLen = Math.max(before.length, after.length);
  const result: number[] = [];

  for (let i = 0; i < maxLen; i++) {
    const b = i < before.length ? before.charCodeAt(i) : 48; // '0'
    const a = i < after.length ? after.charCodeAt(i) : 122; // 'z'

    if (b < a) {
      const mid = b + Math.floor((a - b) / 2);
      if (mid > b) {
        result.push(mid);
        return String.fromCharCode(...result);
      }
      result.push(b);
    } else {
      result.push(b);
    }
  }

  // Couldn't find midpoint — append a middle character
  result.push(86); // 'V'
  return String.fromCharCode(...result);
}
