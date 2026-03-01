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
import {
  SortableContext,
  arrayMove,
  horizontalListSortingStrategy,
} from "@dnd-kit/sortable";
import { invoke } from "@tauri-apps/api/core";
import { ColumnView } from "@/components/column-view";
import { SortableColumn } from "@/components/sortable-column";
import { TaskCard } from "@/components/task-card";
import { reorderColumns } from "@/lib/column-reorder";
import { defaultTaskTitle } from "@/lib/task-defaults";
import type { Board, Column, Task } from "@/types/kanban";

interface BoardViewProps {
  board: Board;
  tasks: Task[];
  onTaskClick?: (task: Task) => void;
  onUpdateTitle?: (taskId: string, title: string) => void;
  onTaskMoved?: () => void;
}

/**
 * Virtual column layout: maps column id → ordered array of task ids.
 * This is the "live" arrangement shown during a drag, which may differ
 * from the persisted state.
 */
type ColumnLayout = Map<string, string[]>;

type DragType = "task" | "column";

export function BoardView({ board, tasks, onTaskClick, onUpdateTitle, onTaskMoved }: BoardViewProps) {
  const columns = useMemo(
    () => [...board.columns].sort((a, b) => a.order - b.order),
    [board.columns]
  );

  const columnIds = useMemo(() => new Set(columns.map((c) => c.id)), [columns]);
  const columnIdList = useMemo(() => columns.map((c) => c.id), [columns]);

  const taskMap = useMemo(() => {
    const map = new Map<string, Task>();
    for (const task of tasks) map.set(task.id, task);
    return map;
  }, [tasks]);

  const columnMap = useMemo(() => {
    const map = new Map<string, Column>();
    for (const col of columns) map.set(col.id, col);
    return map;
  }, [columns]);

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
  const [activeColumn, setActiveColumn] = useState<Column | null>(null);
  const [virtualColumnOrder, setVirtualColumnOrder] = useState<string[] | null>(null);
  const activeColumnRef = useRef<string | null>(null);
  const dragTypeRef = useRef<DragType | null>(null);

  const currentLayout = virtualLayout ?? baseLayout;
  const currentColumnOrder = virtualColumnOrder ?? columnIdList;

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
      // Could be a column drop zone (prefixed with "drop:")
      if (id.startsWith("drop:")) {
        const colId = id.slice(5);
        if (columnIds.has(colId)) return colId;
      }
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
      const id = event.active.id as string;
      const data = event.active.data.current;

      if (data?.type === "column") {
        // Column drag
        dragTypeRef.current = "column";
        setActiveColumn(columnMap.get(id) ?? null);
        setVirtualColumnOrder([...columnIdList]);
      } else {
        // Task drag
        dragTypeRef.current = "task";
        const task = taskMap.get(id);
        setActiveTask(task ?? null);
        const clone: ColumnLayout = new Map();
        for (const [k, v] of baseLayout) clone.set(k, [...v]);
        setVirtualLayout(clone);
        activeColumnRef.current = task?.position.column ?? null;
      }
    },
    [taskMap, baseLayout, columnMap, columnIdList]
  );

  /** Resolve any ID (task, drop zone, or column) to a column ID */
  const resolveToColumnId = useCallback(
    (id: string, layout: ColumnLayout): string | undefined => {
      if (columnIds.has(id)) return id;
      if (id.startsWith("drop:")) {
        const colId = id.slice(5);
        if (columnIds.has(colId)) return colId;
      }
      // Could be a task — find its column
      for (const [colId, ids] of layout) {
        if (ids.includes(id)) return colId;
      }
      return undefined;
    },
    [columnIds]
  );

  const handleDragOver = useCallback(
    (event: DragOverEvent) => {
      if (dragTypeRef.current === "column") {
        // Column reorder during drag
        const { active, over } = event;
        if (!over || !virtualColumnOrder) return;
        const activeId = active.id as string;
        const rawOverId = over.id as string;

        // Resolve the over target to a column ID (it might be a task or drop zone)
        const overId = resolveToColumnId(rawOverId, currentLayout);
        if (!overId || activeId === overId) return;

        const oldIndex = virtualColumnOrder.indexOf(activeId);
        const newIndex = virtualColumnOrder.indexOf(overId);
        if (oldIndex === -1 || newIndex === -1) return;

        setVirtualColumnOrder(arrayMove(virtualColumnOrder, oldIndex, newIndex));
        return;
      }

      // Task drag over
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
        const isColumnDrop = columnIds.has(overId) || overId.startsWith("drop:");
        if (isColumnDrop) {
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
    [virtualLayout, virtualColumnOrder, findColumn, columnIds, resolveToColumnId, currentLayout]
  );

  const handleDragEnd = useCallback(
    async (event: DragEndEvent) => {
      if (dragTypeRef.current === "column") {
        const colOrder = virtualColumnOrder ?? columnIdList;
        setActiveColumn(null);
        dragTypeRef.current = null;

        const { active, over } = event;
        if (!over) {
          setVirtualColumnOrder(null);
          return;
        }

        const activeId = active.id as string;

        // Use the virtual column order — it was maintained during dragOver
        const oldIndex = columnIdList.indexOf(activeId);
        const newIndex = colOrder.indexOf(activeId);

        if (oldIndex === -1 || newIndex === -1 || oldIndex === newIndex) {
          setVirtualColumnOrder(null);
          return;
        }

        const updates = reorderColumns(columnIdList, oldIndex, newIndex);
        if (updates.length === 0) {
          setVirtualColumnOrder(null);
          return;
        }

        try {
          await invoke("reorder_columns", { columns: updates });
          onTaskMoved?.();
        } catch (e) {
          console.error("Failed to reorder columns:", e);
        } finally {
          setVirtualColumnOrder(null);
        }
        return;
      }

      // Task drag end
      const { active, over } = event;
      const layout = virtualLayout ?? baseLayout;

      setActiveTask(null);
      setVirtualLayout(null);
      dragTypeRef.current = null;

      if (!over) return;

      const activeId = active.id as string;
      const overId = over.id as string;

      const draggedTask = taskMap.get(activeId);
      if (!draggedTask) return;

      // Find which column the task ended up in
      const targetColumn = findColumn(activeId, layout);
      if (!targetColumn) return;

      // Handle same-position drop (no-op)
      const targetList = layout.get(targetColumn);
      if (!targetList) return;

      // If dropped on itself with no column change
      if (activeId === overId && targetColumn === draggedTask.position.column) return;

      // Handle same-column reorder via arrayMove for correct index
      const isColumnDrop = columnIds.has(overId) || overId.startsWith("drop:");
      if (targetColumn === draggedTask.position.column && !isColumnDrop) {
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
    [virtualLayout, virtualColumnOrder, baseLayout, taskMap, findColumn, columnIds, columnIdList]
  );

  const handleRenameColumn = useCallback(
    async (columnId: string, name: string) => {
      try {
        await invoke("rename_column", { id: columnId, name });
        onTaskMoved?.();
      } catch (e) {
        console.error("Failed to rename column:", e);
      }
    },
    [onTaskMoved]
  );

  const handleAddTask = useCallback(
    async (columnId: string) => {
      const col = columnMap.get(columnId);
      const title = defaultTaskTitle(col?.name ?? "");
      try {
        await invoke("add_task", { title, column: columnId });
        onTaskMoved?.();
      } catch (e) {
        console.error("Failed to add task:", e);
      }
    },
    [columnMap, onTaskMoved]
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
        <SortableContext
          items={currentColumnOrder}
          strategy={horizontalListSortingStrategy}
        >
          {currentColumnOrder.map((colId, i) => {
            const col = columnMap.get(colId);
            if (!col) return null;
            const taskIds = currentLayout.get(col.id) ?? [];
            const colTasks = taskIds
              .map((id) => taskMap.get(id))
              .filter((t): t is Task => t !== undefined);
            return (
              <SortableColumn key={col.id} id={col.id} showSeparator={i > 0}>
                <ColumnView
                  column={col}
                  tasks={colTasks}
                  tags={board.tags}
                  blockedIds={blockedIds}
                  onTaskClick={onTaskClick}
                  onUpdateTitle={onUpdateTitle}
                  onAddTask={handleAddTask}
                  onRenameColumn={handleRenameColumn}
                  presorted
                />
              </SortableColumn>
            );
          })}
        </SortableContext>
      </div>
      <DragOverlay dropAnimation={null}>
        {activeTask ? <TaskCard task={activeTask} tags={board.tags} /> : null}
        {activeColumn ? (
          <div className="rounded-md bg-card border border-border px-4 py-2 text-sm font-medium text-muted-foreground uppercase tracking-wide shadow-lg">
            {activeColumn.name}
          </div>
        ) : null}
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
