import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  DndContext,
  DragOverlay,
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
import { emit } from "@tauri-apps/api/event";
import { useActiveBoardPath } from "@/lib/command-scope";
import { ColumnView } from "@/components/column-view";
import { SortableColumn } from "@/components/sortable-column";
import { FocusScope } from "@/components/focus-scope";
import { useEntityFocus } from "@/lib/entity-focus-context";
/** Default title for new tasks — the Rust side also uses this as fallback. */
function defaultTaskTitle(_columnName: string): string {
  return "New task";
}
import { useFieldUpdate } from "@/lib/field-update-context";
import { moniker } from "@/lib/moniker";
import { useInspect } from "@/lib/inspect-context";
import { useDragSession } from "@/lib/drag-session-context";
import type { BoardData, Entity } from "@/types/kanban";
import { getStr, getNum } from "@/types/kanban";

interface BoardViewProps {
  board: BoardData;
  tasks: Entity[];
}

type ColumnLayout = Map<string, string[]>;

interface TaskDragState {
  sourceTaskId: string;
  sourceColumn: string;
  targetColumn: string | null;
  insertIndex: number | null;
}

export function BoardView({ board, tasks }: BoardViewProps) {
  const boardPath = useActiveBoardPath();
  const boardPathRef = useRef(boardPath);
  boardPathRef.current = boardPath;
  const { setFocus } = useEntityFocus();
  const inspectEntity = useInspect();
  const { startSession, cancelSession, completeSession } = useDragSession();
  const boardMoniker = moniker("board", "board");
  const boardCommands = useMemo(
    () => [
      {
        id: "entity.inspect",
        name: "Inspect board",
        target: boardMoniker,
        contextMenu: true,
        execute: () => inspectEntity(boardMoniker),
      },
    ],
    [boardMoniker, inspectEntity],
  );

  const columns = useMemo(
    () =>
      [...board.columns].sort(
        (a, b) => getNum(a, "order") - getNum(b, "order"),
      ),
    [board.columns],
  );

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
          getStr(tb, "position_ordinal", "a0"),
        );
      });
    }
    return map;
  }, [columns, tasks, taskMap]);

  // --- Column drag state (managed by @dnd-kit) ---
  const [activeColumn, setActiveColumn] = useState<Entity | null>(null);
  const [virtualColumnOrder, setVirtualColumnOrder] = useState<string[] | null>(
    null,
  );
  const currentColumnOrder = virtualColumnOrder ?? columnIdList;

  // --- HTML5 task drag state ---
  const [taskDrag, setTaskDrag] = useState<TaskDragState | null>(null);

  // Cancel backend drag session on Escape during an active task drag
  useEffect(() => {
    if (!taskDrag) return;
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        cancelSession();
        setTaskDrag(null);
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [taskDrag, cancelSession]);

  // @dnd-kit sensors — columns only
  const sensors = useSensors(
    useSensor(PointerSensor, {
      activationConstraint: { distance: 5 },
    }),
  );

  // --- Column drag handlers (@dnd-kit) ---
  const handleColumnDragStart = useCallback(
    (event: DragStartEvent) => {
      const id = event.active.id as string;
      setActiveColumn(columnMap.get(id) ?? null);
      setVirtualColumnOrder([...columnIdList]);
    },
    [columnMap, columnIdList],
  );

  const handleColumnDragOver = useCallback(
    (event: DragOverEvent) => {
      const { active, over } = event;
      if (!over || !virtualColumnOrder) return;
      const activeId = active.id as string;
      const overId = over.id as string;
      if (activeId === overId) return;

      const oldIndex = virtualColumnOrder.indexOf(activeId);
      const newIndex = virtualColumnOrder.indexOf(overId);
      if (oldIndex === -1 || newIndex === -1) return;

      setVirtualColumnOrder(arrayMove(virtualColumnOrder, oldIndex, newIndex));
    },
    [virtualColumnOrder],
  );

  const handleColumnDragEnd = useCallback(
    async (event: DragEndEvent) => {
      const colOrder = virtualColumnOrder ?? columnIdList;
      setActiveColumn(null);

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
          ...(boardPathRef.current
            ? { boardPath: boardPathRef.current }
            : {}),
        });
      } catch (e) {
        console.error("Failed to reorder columns:", e);
      } finally {
        setVirtualColumnOrder(null);
      }
    },
    [virtualColumnOrder, columnIdList],
  );

  // --- HTML5 task drag handlers ---
  const persistMove = useCallback(
    async (
      taskId: string,
      column: string,
      entity: Entity,
      placement: { before?: string; after?: string },
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
          ...(boardPathRef.current
            ? { boardPath: boardPathRef.current }
            : {}),
        });
      } catch (e) {
        console.error("Failed to move task:", e);
      }
    },
    [],
  );

  const handleTaskDragStart = useCallback(
    (entity: Entity) => {
      const sourceColumn = getStr(entity, "position_column");
      setTaskDrag({
        sourceTaskId: entity.id,
        sourceColumn,
        targetColumn: null,
        insertIndex: null,
      });
      startSession(entity.id, entity.fields, false);
    },
    [startSession],
  );

  const handleTaskDragEnd = useCallback(
    (_entity: Entity, dropEffect: string) => {
      setTaskDrag(null);
      emit("drag-ended", {});
      // Only cancel the backend session if the drop was rejected (no valid target).
      // Successful drops are handled by handleTaskDrop which calls persistMove
      // or completeSession directly.
      if (dropEffect === "none") {
        cancelSession();
      }
    },
    [cancelSession],
  );

  const handleColumnDragOverHTML5 = useCallback(
    (columnId: string, insertIndex: number) => {
      setTaskDrag((prev) => {
        if (!prev) return prev;
        return { ...prev, targetColumn: columnId, insertIndex };
      });
    },
    [],
  );

  const handleColumnDragEnter = useCallback((columnId: string) => {
    setTaskDrag((prev) => {
      if (!prev) return prev;
      return { ...prev, targetColumn: columnId };
    });
  }, []);

  const handleColumnDragLeave = useCallback((_columnId: string) => {
    setTaskDrag((prev) => {
      if (!prev) return prev;
      return { ...prev, targetColumn: null, insertIndex: null };
    });
  }, []);

  const handleTaskDrop = useCallback(
    (columnId: string, taskData: string, insertIndex: number) => {
      setTaskDrag(null);
      let entity: Entity | null = null;
      if (taskData) {
        try {
          entity = JSON.parse(taskData);
        } catch {
          // ignore
        }
      }

      if (!entity) {
        cancelSession();
        return;
      }

      const taskId = entity.id;
      const isLocalTask = taskMap.has(taskId);

      if (isLocalTask) {
        // Same-board drop — task exists locally, move it directly
        cancelSession();
        const colTasks = baseLayout.get(columnId) ?? [];

        if (colTasks.length === 0 || insertIndex >= colTasks.length) {
          const lastId =
            colTasks.length > 0 ? colTasks[colTasks.length - 1] : undefined;
          persistMove(taskId, columnId, entity, lastId ? { after: lastId } : {});
        } else {
          const beforeId = colTasks[insertIndex];
          const sourceColumn = getStr(entity, "position_column");
          const sourceIndex = (baseLayout.get(sourceColumn) ?? []).indexOf(
            taskId,
          );
          if (columnId === sourceColumn && sourceIndex < insertIndex) {
            persistMove(taskId, columnId, entity, { after: beforeId });
          } else {
            persistMove(taskId, columnId, entity, { before: beforeId });
          }
        }
      } else {
        // Cross-board drop — task doesn't exist here yet, complete via session
        // The backend handles creating/moving the task to this board
        const colTasks = baseLayout.get(columnId) ?? [];
        const beforeId = insertIndex < colTasks.length ? colTasks[insertIndex] : undefined;
        const afterId = !beforeId && colTasks.length > 0 ? colTasks[colTasks.length - 1] : undefined;
        completeSession(columnId, {
          dropIndex: insertIndex,
          beforeId,
          afterId,
        });
      }
    },
    [taskMap, baseLayout, persistMove, cancelSession, completeSession],
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
    [updateField],
  );

  const handleAddTask = useCallback(
    async (columnId: string) => {
      const col = columnMap.get(columnId);
      const title = defaultTaskTitle(col ? getStr(col, "name") : "");
      try {
        await invoke("dispatch_command", {
          cmd: "task.add",
          args: { title, column: columnId },
          ...(boardPathRef.current
            ? { boardPath: boardPathRef.current }
            : {}),
        });
      } catch (e) {
        console.error("Failed to add task:", e);
      }
    },
    [columnMap],
  );

  return (
    <FocusScope
      moniker={boardMoniker}
      commands={boardCommands}
      className="flex flex-col flex-1 min-h-0 relative"
    >
      {/* @dnd-kit context for column reordering only */}
      <DndContext
        sensors={sensors}
        onDragStart={handleColumnDragStart}
        onDragOver={handleColumnDragOver}
        onDragEnd={handleColumnDragEnd}
      >
        <div
          className="flex flex-1 min-h-0 overflow-x-auto"
          onClick={() => setFocus(null)}
        >
          <SortableContext
            items={currentColumnOrder}
            strategy={horizontalListSortingStrategy}
          >
            {currentColumnOrder.map((colId, i) => {
              const col = columnMap.get(colId);
              if (!col) return null;
              const taskIds = baseLayout.get(col.id) ?? [];
              const colTasks = taskIds
                .map((id) => taskMap.get(id))
                .filter((t): t is Entity => t !== undefined);
              return (
                <SortableColumn
                  key={col.id}
                  id={col.id}
                  showSeparator={i > 0}
                >
                  <ColumnView
                    column={col}
                    tasks={colTasks}
                    onAddTask={i === 0 ? handleAddTask : undefined}
                    onRenameColumn={handleRenameColumn}
                    onTaskDragStart={handleTaskDragStart}
                    onTaskDragEnd={handleTaskDragEnd}
                    onDragOver={handleColumnDragOverHTML5}
                    onDrop={handleTaskDrop}
                    onDragEnter={handleColumnDragEnter}
                    onDragLeave={handleColumnDragLeave}
                    insertAtIndex={
                      taskDrag?.targetColumn === col.id
                        ? taskDrag.insertIndex
                        : null
                    }
                  />
                </SortableColumn>
              );
            })}
          </SortableContext>
        </div>
        <DragOverlay dropAnimation={null}>
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
