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
import { useActiveBoardPath } from "@/lib/command-scope";
import { ColumnView } from "@/components/column-view";
import { SortableColumn } from "@/components/sortable-column";
import { EntityCard } from "@/components/entity-card";
import { FocusScope } from "@/components/focus-scope";
import { useEntityFocus } from "@/lib/entity-focus-context";
/** Default title for new tasks — the Rust side also uses this as fallback. */
function defaultTaskTitle(_columnName: string): string {
  return "New task";
}
import { useFieldUpdate } from "@/lib/field-update-context";
import { moniker } from "@/lib/moniker";
import { useInspect } from "@/lib/inspect-context";
import type { BoardData, Entity } from "@/types/kanban";
import { getStr, getNum } from "@/types/kanban";

interface BoardViewProps {
  board: BoardData;
  tasks: Entity[];
}

/**
 * Virtual column layout: maps column id → ordered array of task ids.
 * This is the "live" arrangement shown during a drag, which may differ
 * from the persisted state.
 */
type ColumnLayout = Map<string, string[]>;

type DragType = "task" | "column";

export function BoardView({ board, tasks }: BoardViewProps) {
  const boardPath = useActiveBoardPath();
  const boardPathRef = useRef(boardPath);
  boardPathRef.current = boardPath;
  const { setFocus } = useEntityFocus();
  const inspectEntity = useInspect();
  const boardMoniker = moniker("board", "board");
  const boardCommands = useMemo(() => [
    {
      id: "entity.inspect",
      name: "Inspect board",
      target: boardMoniker,
      contextMenu: true,
      execute: () => inspectEntity(boardMoniker),
    },
  ], [boardMoniker, inspectEntity]);

  const columns = useMemo(
    () => [...board.columns].sort((a, b) =>
      getNum(a, "order") - getNum(b, "order")
    ),
    [board.columns]
  );

  const columnIds = useMemo(() => new Set(columns.map((c) => c.id)), [columns]);
  const columnIdList = useMemo(() => columns.map((c) => c.id), [columns]);

  const taskMap = useMemo(() => {
    const map = new Map<string, Entity>();
    for (const task of tasks) map.set(task.id, task);
    return map;
  }, [tasks]);

  const columnMap = useMemo(() => {
    const map = new Map<string, Entity>();
    for (const col of columns) map.set(col.id, col);
    return map;
  }, [columns]);

  // Group tasks by column and sort each column by ordinal.
  // The backend sorts on initial load, but incremental entity-field-changed
  // events patch the tasks array in-place without re-sorting, so the frontend
  // must sort to maintain correct visual order after moves.
  const baseLayout = useMemo<ColumnLayout>(() => {
    const map: ColumnLayout = new Map();
    for (const col of columns) map.set(col.id, []);
    for (const task of tasks) {
      const col = getStr(task, "position_column");
      const list = map.get(col);
      if (list) list.push(task.id);
    }
    for (const ids of map.values()) {
      ids.sort((a, b) => {
        const ta = taskMap.get(a)!;
        const tb = taskMap.get(b)!;
        return getStr(ta, "position_ordinal", "a0").localeCompare(
          getStr(tb, "position_ordinal", "a0")
        );
      });
    }
    return map;
  }, [columns, tasks, taskMap]);

  // Virtual layout tracks live arrangement during drag
  const [virtualLayout, setVirtualLayout] = useState<ColumnLayout | null>(null);
  const [activeTask, setActiveTask] = useState<Entity | null>(null);
  const [activeColumn, setActiveColumn] = useState<Entity | null>(null);
  const [virtualColumnOrder, setVirtualColumnOrder] = useState<string[] | null>(null);
  const activeColumnRef = useRef<string | null>(null);
  const dragTypeRef = useRef<DragType | null>(null);

  const currentLayout = virtualLayout ?? baseLayout;
  const currentColumnOrder = virtualColumnOrder ?? columnIdList;


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
        activeColumnRef.current = task ? getStr(task, "position_column") || null : null;
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

      // Cross-column move: transfer task between columns in the virtual layout
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
          toList.push(activeId);
        } else {
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

  /**
   * Tell the backend: put taskId in column, before or after a reference task.
   * The backend computes the ordinal. Only ONE entity is touched.
   */
  const persistMove = useCallback(
    async (
      taskId: string,
      column: string,
      entity: Entity,
      placement: { before?: string; after?: string }
    ) => {
      try {
        const args: Record<string, unknown> = {
          id: taskId,
          column,
          swimlane: getStr(entity, "position_swimlane") || null,
        };
        if (placement.before) args.before_id = placement.before;
        if (placement.after) args.after_id = placement.after;
        await invoke("dispatch_command", {
          cmd: "task.move",
          args,
          ...(boardPathRef.current ? { boardPath: boardPathRef.current } : {}),
        });
      } catch (e) {
        console.error("Failed to move task:", e);
      }
    },
    []
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
        const oldIndex = columnIdList.indexOf(activeId);
        const newIndex = colOrder.indexOf(activeId);

        if (oldIndex === -1 || newIndex === -1 || oldIndex === newIndex) {
          setVirtualColumnOrder(null);
          return;
        }

        try {
          await invoke("dispatch_command", {
            cmd: "column.reorder",
            args: { id: activeId, target_index: newIndex },
            ...(boardPathRef.current ? { boardPath: boardPathRef.current } : {}),
          });
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

      const targetColumn = findColumn(activeId, layout);
      if (!targetColumn) return;

      const draggedColumn = getStr(draggedTask, "position_column");

      // No-op: dropped on itself, same column
      if (activeId === overId && targetColumn === draggedColumn) return;

      // Resolve overId: is it a column/drop-zone, or a task?
      const isColumnDrop = columnIds.has(overId) || overId.startsWith("drop:");

      if (isColumnDrop && targetColumn !== draggedColumn) {
        // Cross-column drop onto column zone — append at end (no before/after)
        await persistMove(activeId, targetColumn, draggedTask, {});
        return;
      }

      if (isColumnDrop && targetColumn === draggedColumn) {
        // Same-column drop onto column zone — no-op (can't determine intent)
        return;
      }

      // Dropped onto a specific task (overId is a task ID).
      // Simple rule: "put me before overId".
      // The backend reads overId's ordinal and computes a new ordinal
      // that sorts just before it. One entity touched.
      const oldIndex = (baseLayout.get(draggedColumn) ?? []).indexOf(activeId);
      const overIndex = (layout.get(targetColumn) ?? []).indexOf(overId);

      // If dragging downward within same column, place AFTER the target instead
      if (targetColumn === draggedColumn && oldIndex < overIndex) {
        await persistMove(activeId, targetColumn, draggedTask, { after: overId });
      } else {
        await persistMove(activeId, targetColumn, draggedTask, { before: overId });
      }
    },
    [virtualLayout, virtualColumnOrder, baseLayout, taskMap, findColumn, columnIds, columnIdList, persistMove]
  );

  const { updateField } = useFieldUpdate();

  const handleRenameColumn = useCallback(
    async (columnId: string, name: string) => {
      try {
        await updateField("column", columnId, "name", name);
      } catch {
        // updateField already logs errors
      }
    },
    [updateField]
  );

  const handleAddTask = useCallback(
    async (columnId: string) => {
      const col = columnMap.get(columnId);
      const title = defaultTaskTitle(col ? getStr(col, "name") : "");
      try {
        await invoke("dispatch_command", { cmd: "task.add", args: { title, column: columnId }, ...(boardPathRef.current ? { boardPath: boardPathRef.current } : {}) });
      } catch (e) {
        console.error("Failed to add task:", e);
      }
    },
    [columnMap]
  );

  return (
    <FocusScope moniker={boardMoniker} commands={boardCommands} className="flex flex-col flex-1 min-h-0">
      <DndContext
        sensors={sensors}
        collisionDetection={closestCorners}
        onDragStart={handleDragStart}
        onDragOver={handleDragOver}
        onDragEnd={handleDragEnd}
      >
        <div className="flex flex-1 min-h-0 overflow-x-auto" onClick={() => setFocus(null)}>
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
                .filter((t): t is Entity => t !== undefined);
              return (
                <SortableColumn key={col.id} id={col.id} showSeparator={i > 0}>
                  <ColumnView
                    column={col}
                    tasks={colTasks}
                    // Only the first column gets the + button — new tasks should
                    // always enter at the first workflow stage.
                    onAddTask={i === 0 ? handleAddTask : undefined}
                    onRenameColumn={handleRenameColumn}
                  />
                </SortableColumn>
              );
            })}
          </SortableContext>
        </div>
        <DragOverlay dropAnimation={null}>
          {activeTask ? <EntityCard entity={activeTask} /> : null}
          {activeColumn ? (
            <div className="rounded-md bg-card border border-border px-4 py-2 text-sm font-medium text-muted-foreground uppercase tracking-wide shadow-lg">
              {getStr(activeColumn, "name")}
            </div>
          ) : null}
        </DragOverlay>
      </DndContext>
    </FocusScope>
  );
}
