import { useCallback, useMemo, useRef, useState } from "react";
import {
  DndContext,
  DragOverlay,
  closestCorners,
  PointerSensor,
  useSensor,
  useSensors,
  type DragEndEvent,
  type DragOverEvent,
  type DragStartEvent,
} from "@dnd-kit/core";
import { arrayMove } from "@dnd-kit/sortable";
import { invoke } from "@tauri-apps/api/core";
import { ColumnView } from "@/components/column-view";
import { TaskCard } from "@/components/task-card";
import type { Board, Task } from "@/types/kanban";

interface BoardViewProps {
  board: Board;
  tasks: Task[];
  onTaskClick?: (task: Task) => void;
  onTaskMoved?: () => void;
}

/**
 * Virtual column layout: maps column id → ordered array of task ids.
 * This is the "live" arrangement shown during a drag, which may differ
 * from the persisted state.
 */
type ColumnLayout = Map<string, string[]>;

export function BoardView({ board, tasks, onTaskClick, onTaskMoved }: BoardViewProps) {
  const columns = useMemo(
    () => [...board.columns].sort((a, b) => a.order - b.order),
    [board.columns]
  );

  const columnIds = useMemo(() => new Set(columns.map((c) => c.id)), [columns]);

  const taskMap = useMemo(() => {
    const map = new Map<string, Task>();
    for (const task of tasks) map.set(task.id, task);
    return map;
  }, [tasks]);

  // The "real" column layout from persisted state
  const baseLayout = useMemo<ColumnLayout>(() => {
    const map: ColumnLayout = new Map();
    for (const col of columns) map.set(col.id, []);
    for (const task of tasks) {
      const list = map.get(task.position.column);
      if (list) list.push(task.id);
    }
    // Sort each column by ordinal
    for (const [colId, ids] of map) {
      ids.sort((a, b) => {
        const ta = taskMap.get(a)!;
        const tb = taskMap.get(b)!;
        return ta.position.ordinal.localeCompare(tb.position.ordinal);
      });
      map.set(colId, ids);
    }
    return map;
  }, [columns, tasks, taskMap]);

  // Virtual layout tracks live arrangement during drag
  const [virtualLayout, setVirtualLayout] = useState<ColumnLayout | null>(null);
  const [activeTask, setActiveTask] = useState<Task | null>(null);
  const activeColumnRef = useRef<string | null>(null);

  const currentLayout = virtualLayout ?? baseLayout;

  // Collect blocked task IDs
  const blockedIds = useMemo(() => {
    const set = new Set<string>();
    for (const task of tasks) {
      if (task.depends_on.length > 0) {
        const terminalCol = columns[columns.length - 1]?.id;
        const hasIncomplete = task.depends_on.some((depId) => {
          const dep = taskMap.get(depId);
          return dep && dep.position.column !== terminalCol;
        });
        if (hasIncomplete) set.add(task.id);
      }
    }
    return set;
  }, [tasks, columns, taskMap]);

  const sensors = useSensors(
    useSensor(PointerSensor, {
      activationConstraint: { distance: 5 },
    })
  );

  /** Find which column contains a given id in the current layout */
  const findColumn = useCallback(
    (id: string, layout: ColumnLayout): string | undefined => {
      // Could be a column id itself
      if (columnIds.has(id)) return id;
      // Otherwise search for the task
      for (const [colId, ids] of layout) {
        if (ids.includes(id)) return colId;
      }
      return undefined;
    },
    [columnIds]
  );

  const handleDragStart = useCallback(
    (event: DragStartEvent) => {
      const task = taskMap.get(event.active.id as string);
      setActiveTask(task ?? null);
      // Clone the base layout as starting point for virtual layout
      const clone: ColumnLayout = new Map();
      for (const [k, v] of baseLayout) clone.set(k, [...v]);
      setVirtualLayout(clone);
      activeColumnRef.current = task?.position.column ?? null;
    },
    [taskMap, baseLayout]
  );

  const handleDragOver = useCallback(
    (event: DragOverEvent) => {
      const { active, over } = event;
      if (!over || !virtualLayout) return;

      const activeId = active.id as string;
      const overId = over.id as string;

      const fromCol = findColumn(activeId, virtualLayout);
      const toCol = findColumn(overId, virtualLayout);
      if (!fromCol || !toCol || fromCol === toCol) return;

      // Move the task from one column to another in the virtual layout
      setVirtualLayout((prev) => {
        if (!prev) return prev;
        const clone: ColumnLayout = new Map();
        for (const [k, v] of prev) clone.set(k, [...v]);

        const fromList = clone.get(fromCol)!;
        const toList = clone.get(toCol)!;

        // Remove from source
        const idx = fromList.indexOf(activeId);
        if (idx !== -1) fromList.splice(idx, 1);

        // Insert into target
        if (columnIds.has(overId)) {
          // Dropped over the column droppable — append at end
          toList.push(activeId);
        } else {
          // Dropped over a task — insert at that position
          const overIdx = toList.indexOf(overId);
          if (overIdx !== -1) {
            toList.splice(overIdx, 0, activeId);
          } else {
            toList.push(activeId);
          }
        }

        return clone;
      });
    },
    [virtualLayout, findColumn, columnIds]
  );

  const handleDragEnd = useCallback(
    async (event: DragEndEvent) => {
      const { active, over } = event;
      const layout = virtualLayout ?? baseLayout;

      setActiveTask(null);
      setVirtualLayout(null);

      if (!over) return;

      const activeId = active.id as string;
      const overId = over.id as string;

      const draggedTask = taskMap.get(activeId);
      if (!draggedTask) return;

      // Find which column the task ended up in
      let targetColumn = findColumn(activeId, layout);
      if (!targetColumn) return;

      // Handle same-position drop (no-op)
      const targetList = layout.get(targetColumn);
      if (!targetList) return;

      // If dropped on itself with no column change
      if (activeId === overId && targetColumn === draggedTask.position.column) return;

      // Handle same-column reorder via arrayMove for correct index
      if (targetColumn === draggedTask.position.column && !columnIds.has(overId)) {
        const oldIndex = targetList.indexOf(activeId);
        const newIndex = targetList.indexOf(overId);
        if (oldIndex !== -1 && newIndex !== -1 && oldIndex !== newIndex) {
          const reordered = arrayMove(targetList, oldIndex, newIndex);
          const ordinal = computeOrdinal(reordered, newIndex, taskMap);
          await persistMove(activeId, targetColumn, ordinal, draggedTask);
          return;
        }
      }

      // Cross-column or drop on column: use the virtual layout position
      const finalIndex = targetList.indexOf(activeId);
      if (finalIndex === -1) return;

      const ordinal = computeOrdinal(targetList, finalIndex, taskMap);
      await persistMove(activeId, targetColumn, ordinal, draggedTask);
    },
    [virtualLayout, baseLayout, taskMap, findColumn, columnIds]
  );

  async function persistMove(
    taskId: string,
    column: string,
    ordinal: string,
    task: Task
  ) {
    try {
      await invoke("move_task", {
        id: taskId,
        column,
        ordinal,
        swimlane: task.position.swimlane ?? null,
      });
      onTaskMoved?.();
    } catch (e) {
      console.error("Failed to move task:", e);
    }
  }

  return (
    <DndContext
      sensors={sensors}
      collisionDetection={closestCorners}
      onDragStart={handleDragStart}
      onDragOver={handleDragOver}
      onDragEnd={handleDragEnd}
    >
      <div className="flex flex-1 min-h-0 overflow-x-auto">
        {columns.map((col, i) => {
          const taskIds = currentLayout.get(col.id) ?? [];
          const colTasks = taskIds
            .map((id) => taskMap.get(id))
            .filter((t): t is Task => t !== undefined);
          return (
            <div key={col.id} className="flex flex-1 min-w-[20em] max-w-[60em]">
              {i > 0 && <div className="w-px bg-border shrink-0 my-3" />}
              <ColumnView
                column={col}
                tasks={colTasks}
                blockedIds={blockedIds}
                onTaskClick={onTaskClick}
                presorted
              />
            </div>
          );
        })}
      </div>
      <DragOverlay dropAnimation={null}>
        {activeTask ? <TaskCard task={activeTask} /> : null}
      </DragOverlay>
    </DndContext>
  );
}

/**
 * Compute an ordinal for the task at `index` within the ordered `ids` list,
 * based on the ordinals of its neighbors.
 */
function computeOrdinal(
  ids: string[],
  index: number,
  taskMap: Map<string, Task>
): string {
  const prevTask = index > 0 ? taskMap.get(ids[index - 1]) : undefined;
  const nextTask = index < ids.length - 1 ? taskMap.get(ids[index + 1]) : undefined;
  const prev = prevTask?.position.ordinal;
  const next = nextTask?.position.ordinal;

  if (!prev && !next) return "a0";
  if (!prev && next) {
    const code = next.charCodeAt(0);
    const ord = code > 97 ? String.fromCharCode(code - 1) + "0" : "a0";
    return ord < next ? ord : "a0";
  }
  if (prev && !next) {
    const lastChar = prev.charCodeAt(prev.length - 1);
    return prev.slice(0, -1) + String.fromCharCode(lastChar + 1);
  }
  return midpointOrdinal(prev!, next!);
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

  result.push(86); // 'V'
  return String.fromCharCode(...result);
}
