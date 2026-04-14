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
import { emit } from "@tauri-apps/api/event";
import { toast } from "sonner";
import type { DropZoneDescriptor } from "@/lib/drop-zones";
import {
  CommandScopeProvider,
  useDispatchCommand,
  type CommandDef,
} from "@/lib/command-scope";
import { ColumnView } from "@/components/column-view";
import { SortableColumn } from "@/components/sortable-column";
import { FocusScope } from "@/components/focus-scope";
import { useEntityFocus } from "@/lib/entity-focus-context";
/** Default title for new tasks — the Rust side also uses this as fallback. */
function defaultTaskTitle(_columnName: string): string {
  return "New task";
}
import { useEntityCommands } from "@/lib/entity-commands";
import { useDragSession } from "@/lib/drag-session-context";
import { useActivePerspective } from "@/components/perspective-container";
import type { BoardData, Entity } from "@/types/kanban";
import { getStr, getNum } from "@/types/kanban";

interface BoardViewProps {
  board: BoardData;
  tasks: Entity[];
  /** When rendered inside a GroupSection, the group value for this slice. */
  groupValue?: string;
}

type ColumnLayout = Map<string, string[]>;

/**
 * Compare two tasks for column ordering.
 *
 * When a group field is active (and we're not inside a group section), clusters
 * by group value first. Within each group (or without grouping), sorts by ordinal.
 */
function compareTaskOrder(
  ta: Entity,
  tb: Entity,
  groupField: string | undefined,
  groupValue: string | undefined,
): number {
  if (groupField && groupValue === undefined) {
    const ga = String(ta.fields[groupField] ?? "");
    const gb = String(tb.fields[groupField] ?? "");
    const groupCmp = ga.localeCompare(gb);
    if (groupCmp !== 0) return groupCmp;
  }
  return getStr(ta, "position_ordinal", "a0").localeCompare(
    getStr(tb, "position_ordinal", "a0"),
  );
}

interface TaskDragState {
  sourceTaskId: string;
  sourceColumn: string;
}

/** Return value from useBoardLayout — all derived board data needed for rendering. */
interface BoardLayoutResult {
  columns: Entity[];
  columnIdList: string[];
  filteredTasks: Entity[];
  taskMap: Map<string, Entity>;
  columnMap: Map<string, Entity>;
  baseLayout: ColumnLayout;
  columnTasks: Map<string, Entity[]>;
  firstTodoTaskId: string | null;
  columnTaskMonikers: Map<string, string[]>;
  allBoardTaskMonikers: Set<string>;
  allBoardHeaderMonikers: Set<string>;
}

/**
 * Derive all board layout data from raw board/task props.
 *
 * Handles column sorting, task bucketing into columns, moniker tables for
 * cross-column keyboard navigation, and group-aware ordering.
 */
function useBoardLayout(
  board: BoardData,
  tasks: Entity[],
  groupValue: string | undefined,
): BoardLayoutResult {
  const { groupField } = useActivePerspective();

  const columns = useMemo(
    () =>
      [...board.columns].sort(
        (a, b) => getNum(a, "order") - getNum(b, "order"),
      ),
    [board.columns],
  );

  const columnIdList = useMemo(() => columns.map((c) => c.id), [columns]);

  // Filtering is server-side; alias for clarity.
  const filteredTasks = tasks;

  const taskMap = useMemo(() => {
    const map = new Map<string, Entity>();
    for (const task of filteredTasks) map.set(task.id, task);
    return map;
  }, [filteredTasks]);

  const columnMap = useMemo(() => {
    const map = new Map<string, Entity>();
    for (const col of columns) map.set(col.id, col);
    return map;
  }, [columns]);

  const baseLayout = useMemo<ColumnLayout>(() => {
    const map: ColumnLayout = new Map();
    for (const col of columns) map.set(col.id, []);
    for (const task of filteredTasks) {
      const col = getStr(task, "position_column");
      map.get(col)?.push(task.id);
    }
    for (const ids of map.values()) {
      ids.sort((a, b) =>
        compareTaskOrder(
          taskMap.get(a)!,
          taskMap.get(b)!,
          groupField,
          groupValue,
        ),
      );
    }
    return map;
  }, [columns, filteredTasks, taskMap, groupField, groupValue]);

  const columnTasks = useMemo(() => {
    const map = new Map<string, Entity[]>();
    for (const col of columns) {
      const taskIds = baseLayout.get(col.id) ?? [];
      const entities = taskIds
        .map((id) => taskMap.get(id))
        .filter((t): t is Entity => t !== undefined);
      map.set(col.id, entities);
    }
    return map;
  }, [columns, baseLayout, taskMap]);

  const firstTodoTaskId = useMemo(() => {
    if (columns.length === 0) return null;
    const todoColId = columns[0].id;
    const todoTaskIds = baseLayout.get(todoColId);
    return todoTaskIds && todoTaskIds.length > 0 ? todoTaskIds[0] : null;
  }, [columns, baseLayout]);

  const columnTaskMonikers = useMemo(() => {
    const map = new Map<string, string[]>();
    for (const col of columns) {
      const taskIds = baseLayout.get(col.id) ?? [];
      map.set(
        col.id,
        taskIds.map((id) => taskMap.get(id)?.moniker ?? `task:${id}`),
      );
    }
    return map;
  }, [columns, baseLayout, taskMap]);

  const allBoardTaskMonikers = useMemo(() => {
    const set = new Set<string>();
    for (const monikers of columnTaskMonikers.values()) {
      for (const m of monikers) set.add(m);
    }
    return set;
  }, [columnTaskMonikers]);

  const allBoardHeaderMonikers = useMemo(() => {
    const set = new Set<string>();
    for (const col of columns) {
      set.add(col.moniker);
      set.add(`${col.moniker}.name`);
    }
    return set;
  }, [columns]);

  return {
    columns,
    columnIdList,
    filteredTasks,
    taskMap,
    columnMap,
    baseLayout,
    columnTasks,
    firstTodoTaskId,
    columnTaskMonikers,
    allBoardTaskMonikers,
    allBoardHeaderMonikers,
  };
}

/** Return value from useBoardDragDrop — drag state and all event handlers. */
interface BoardDragDropResult {
  activeColumn: Entity | null;
  currentColumnOrder: string[];
  taskDrag: TaskDragState | null;
  sensors: ReturnType<typeof useSensors>;
  handleColumnDragStart: (event: DragStartEvent) => void;
  handleColumnDragOver: (event: DragOverEvent) => void;
  handleColumnDragEnd: (event: DragEndEvent) => void;
  handleTaskDragStart: (entity: Entity) => void;
  handleTaskDragEnd: (entity: Entity, dropEffect: string) => void;
  handleZoneDrop: (descriptor: DropZoneDescriptor, taskData: string) => void;
}

/**
 * Manage all drag-and-drop state for the board.
 *
 * Handles column reordering (via @dnd-kit) and task dragging (via HTML5
 * drag), including optimistic column ordering, Escape cancellation, and
 * cross-board drop support.
 */
function useBoardDragDrop(
  columnIdList: string[],
  columnMap: Map<string, Entity>,
  taskMap: Map<string, Entity>,
): BoardDragDropResult {
  const { startSession, cancelSession, completeSession } = useDragSession();
  const dispatch = useDispatchCommand();

  // --- Column drag state (managed by @dnd-kit) ---
  const [activeColumn, setActiveColumn] = useState<Entity | null>(null);
  const [virtualColumnOrder, setVirtualColumnOrder] = useState<string[] | null>(
    null,
  );
  const currentColumnOrder = virtualColumnOrder ?? columnIdList;

  // Clear virtual column order when real data catches up from the backend.
  useEffect(() => {
    setVirtualColumnOrder(null);
  }, [columnIdList]);

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
        await dispatch("column.reorder", {
          args: { id: activeId, target_index: newIndex },
        });
        // Keep virtualColumnOrder alive — columns stay in dragged position
        // until the entity store refresh arrives and columnIdList updates.
      } catch (e) {
        console.error("Failed to reorder columns:", e);
        setVirtualColumnOrder(null);
      }
    },
    [virtualColumnOrder, columnIdList],
  );

  // --- HTML5 task drag handlers ---
  const persistMove = useCallback(
    async (descriptor: DropZoneDescriptor, taskId: string, _entity: Entity) => {
      try {
        const args: Record<string, unknown> = {
          id: taskId,
          column: descriptor.columnId,
        };
        if (descriptor.beforeId) args.before_id = descriptor.beforeId;
        if (descriptor.afterId) args.after_id = descriptor.afterId;
        // Board identity is resolved from the scope chain by useDispatchCommand —
        // no explicit boardPath needed.
        await dispatch("task.move", {
          args,
          target: `task:${taskId}`,
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
      // Successful drops are handled by handleZoneDrop which calls persistMove
      // or completeSession directly.
      if (dropEffect === "none") {
        cancelSession();
      }
    },
    [cancelSession],
  );

  const handleZoneDrop = useCallback(
    (descriptor: DropZoneDescriptor, taskData: string) => {
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
        // Same-board drop — descriptor carries all placement params
        cancelSession();
        persistMove(descriptor, taskId, entity);
      } else {
        // Cross-board drop — pass zone's placement to the session
        completeSession(descriptor.columnId, {
          beforeId: descriptor.beforeId,
          afterId: descriptor.afterId,
        });
      }
    },
    [taskMap, persistMove, cancelSession, completeSession],
  );

  return {
    activeColumn,
    currentColumnOrder,
    taskDrag,
    sensors,
    handleColumnDragStart,
    handleColumnDragOver,
    handleColumnDragEnd,
    handleTaskDragStart,
    handleTaskDragEnd,
    handleZoneDrop,
  };
}

/** Props for the drag overlay sub-component. */
interface BoardDragOverlayProps {
  activeColumn: Entity | null;
}

/**
 * Render the drag overlay shown while a column is being reordered.
 *
 * Displays a floating pill with the dragged column's name, or nothing when no
 * column drag is active.
 */
function BoardDragOverlay({ activeColumn }: BoardDragOverlayProps) {
  return (
    <DragOverlay dropAnimation={null}>
      {activeColumn ? (
        <div className="rounded-md bg-card border border-border px-4 py-2 text-sm font-medium text-muted-foreground uppercase tracking-wide shadow-lg">
          {getStr(activeColumn, "name")}
        </div>
      ) : null}
    </DragOverlay>
  );
}

/**
 * Board view that renders columns and cards.
 *
 * Navigation is pull-based: each card and column header FocusScope declares
 * claimWhen predicates. The global KeybindingHandler broadcasts nav.up/down/
 * left/right/first/last, and each predicate evaluates whether it should claim
 * focus. No push-based cursor state is needed.
 */
/**
 * Board-level action commands: inspect, new task, first/last column navigation.
 *
 * Uses refs for focused moniker and add-task callback to avoid circular
 * dependency between commands and the handlers that depend on them.
 */
function useBoardActionCommands(
  columns: Entity[],
  taskMap: Map<string, Entity>,
  focusedMonikerRef: React.RefObject<string | null>,
  broadcastRef: React.RefObject<(cmd: string) => void>,
  handleAddTaskRef: React.RefObject<(columnId: string) => void>,
  dispatchInspect: ReturnType<typeof useDispatchCommand>,
): CommandDef[] {
  return useMemo<CommandDef[]>(() => {
    const findFocusedColumnId = (): string | null => {
      const fm = focusedMonikerRef.current;
      if (!fm) return columns[0]?.id ?? null;
      if (fm.startsWith("column:")) return fm.slice("column:".length);
      if (fm.startsWith("task:")) {
        const entity = taskMap.get(fm.slice("task:".length));
        if (entity)
          return getStr(entity, "position_column") || (columns[0]?.id ?? null);
      }
      return columns[0]?.id ?? null;
    };

    return [
      {
        id: "board.inspect",
        name: "Inspect",
        keys: { vim: "Enter", cua: "Enter" },
        execute: () => {
          const fm = focusedMonikerRef.current;
          if (fm) dispatchInspect({ target: fm }).catch(console.error);
        },
      },
      {
        id: "board.newTask",
        name: "New Task",
        keys: { vim: "o", cua: "Mod+Enter" },
        execute: () => {
          const colId = findFocusedColumnId();
          if (colId) handleAddTaskRef.current(colId);
        },
      },
      {
        id: "board.firstColumn",
        name: "First Column",
        keys: { vim: "0", cua: "Mod+Home" },
        execute: () => {
          if (columns.length > 0) broadcastRef.current("nav.first");
        },
      },
      {
        id: "board.lastColumn",
        name: "Last Column",
        keys: { vim: "$", cua: "Mod+End" },
        execute: () => {
          if (columns.length > 0) broadcastRef.current("nav.last");
        },
      },
    ];
  }, [columns, taskMap, dispatchInspect]);
}

/**
 * Board view that renders columns and cards.
 *
 * Navigation is pull-based: each card and column header FocusScope declares
 * claimWhen predicates. The global KeybindingHandler broadcasts nav.up/down/
 * left/right/first/last, and each predicate evaluates whether it should claim
 * focus. No push-based cursor state is needed.
 */
export function BoardView({ board, tasks, groupValue }: BoardViewProps) {
  const boardMoniker = board.board.moniker;
  const boardCommands = useEntityCommands("board", "board");
  const dispatch = useDispatchCommand();
  const dispatchInspect = useDispatchCommand("ui.inspect");
  const { focusedMoniker, broadcastNavCommand, setFocus } = useEntityFocus();
  const broadcastRef = useRef(broadcastNavCommand);
  broadcastRef.current = broadcastNavCommand;
  const focusedMonikerRef = useRef(focusedMoniker);
  focusedMonikerRef.current = focusedMoniker;
  const handleAddTaskRef = useRef<(columnId: string) => void>(() => {});
  const scrollContainerRef = useRef<HTMLDivElement>(null);

  const layout = useBoardLayout(board, tasks, groupValue);
  const {
    columns,
    columnIdList,
    taskMap,
    columnMap,
    columnTasks,
    firstTodoTaskId,
    columnTaskMonikers,
    allBoardTaskMonikers,
    allBoardHeaderMonikers,
  } = layout;
  const dragDrop = useBoardDragDrop(columnIdList, columnMap, taskMap);
  const {
    activeColumn,
    currentColumnOrder,
    taskDrag,
    sensors,
    handleColumnDragStart,
    handleColumnDragOver,
    handleColumnDragEnd,
    handleTaskDragStart,
    handleTaskDragEnd,
    handleZoneDrop,
  } = dragDrop;

  const boardActionCommands = useBoardActionCommands(
    columns,
    taskMap,
    focusedMonikerRef,
    broadcastRef,
    handleAddTaskRef,
    dispatchInspect,
  );

  useEffect(() => {
    const container = scrollContainerRef.current;
    if (!container || !focusedMoniker) return;
    const el = container.querySelector<HTMLElement>(
      `[data-moniker="${focusedMoniker}"]`,
    );
    if (el?.scrollIntoView)
      el.scrollIntoView({ inline: "nearest", block: "nearest" });
  }, [focusedMoniker]);

  const initialFocusDone = useRef(false);
  useEffect(() => {
    if (initialFocusDone.current) return;
    initialFocusDone.current = true;
    for (const col of columns) {
      const monikers = columnTaskMonikers.get(col.id) ?? [];
      if (monikers.length > 0) {
        setFocus(monikers[0]);
        return;
      }
    }
    if (columns.length > 0) setFocus(columns[0].moniker);
  }, [columns, columnTaskMonikers, setFocus]);

  const handleAddTask = useCallback(
    async (columnId: string) => {
      const col = columnMap.get(columnId);
      const title = defaultTaskTitle(col ? getStr(col, "name") : "");
      try {
        const result = (await dispatch("task.add", {
          args: { title, column: columnId },
        })) as { id?: string } | undefined;
        if (result?.id) setFocus(`task:${result.id}`);
      } catch (e) {
        toast.error(
          `Failed to add task: ${e instanceof Error ? e.message : String(e)}`,
        );
      }
    },
    [columnMap, setFocus],
  );
  handleAddTaskRef.current = handleAddTask;

  return (
    <FocusScope
      moniker={boardMoniker}
      commands={boardCommands}
      className="flex flex-col flex-1 min-h-0 relative"
    >
      <CommandScopeProvider commands={boardActionCommands}>
        <DndContext
          sensors={sensors}
          onDragStart={handleColumnDragStart}
          onDragOver={handleColumnDragOver}
          onDragEnd={handleColumnDragEnd}
        >
          <div
            ref={scrollContainerRef}
            className="flex flex-1 min-h-0 overflow-x-auto pl-2"
          >
            <SortableContext
              items={currentColumnOrder}
              strategy={horizontalListSortingStrategy}
            >
              {currentColumnOrder.map((colId, i) => {
                const col = columnMap.get(colId);
                if (!col) return null;
                const prevColId = i > 0 ? currentColumnOrder[i - 1] : null;
                const nextColId =
                  i < currentColumnOrder.length - 1
                    ? currentColumnOrder[i + 1]
                    : null;
                return (
                  <SortableColumn
                    key={col.id}
                    id={col.id}
                    showSeparator={i > 0}
                  >
                    <ColumnView
                      column={col}
                      tasks={columnTasks.get(col.id) ?? []}
                      onAddTask={i === 0 ? handleAddTask : undefined}
                      onTaskDragStart={handleTaskDragStart}
                      onTaskDragEnd={handleTaskDragEnd}
                      onDrop={handleZoneDrop}
                      dragTaskId={taskDrag?.sourceTaskId ?? null}
                      firstTodoTaskId={firstTodoTaskId}
                      leftColumnTaskMonikers={
                        prevColId
                          ? (columnTaskMonikers.get(prevColId) ?? [])
                          : []
                      }
                      leftColumnHeaderMoniker={
                        prevColId
                          ? `${columnMap.get(prevColId)?.moniker ?? `column:${prevColId}`}.name`
                          : null
                      }
                      rightColumnTaskMonikers={
                        nextColId
                          ? (columnTaskMonikers.get(nextColId) ?? [])
                          : []
                      }
                      rightColumnHeaderMoniker={
                        nextColId
                          ? `${columnMap.get(nextColId)?.moniker ?? `column:${nextColId}`}.name`
                          : null
                      }
                      allBoardTaskMonikers={allBoardTaskMonikers}
                      allBoardHeaderMonikers={allBoardHeaderMonikers}
                      isFirstColumn={i === 0}
                      isLastColumn={i === currentColumnOrder.length - 1}
                    />
                  </SortableColumn>
                );
              })}
            </SortableContext>
          </div>
          <BoardDragOverlay activeColumn={activeColumn} />
        </DndContext>
      </CommandScopeProvider>
    </FocusScope>
  );
}
