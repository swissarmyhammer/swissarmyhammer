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
  type DispatchOptions,
} from "@/lib/command-scope";
import { ColumnView } from "@/components/column-view";
import { SortableColumn } from "@/components/sortable-column";
import { FocusScope } from "@/components/focus-scope";
import { useEntityFocus, useFocusedMoniker } from "@/lib/entity-focus-context";
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

/** Ordered column entities and lookup maps derived from the board. */
interface ColumnOrdering {
  columns: Entity[];
  columnIdList: string[];
  columnMap: Map<string, Entity>;
}

/**
 * Sort columns by their `order` field and build id-keyed lookup tables.
 *
 * @param board Raw board data whose `columns` list may arrive unsorted.
 */
function useColumnOrdering(board: BoardData): ColumnOrdering {
  const columns = useMemo(
    () =>
      [...board.columns].sort(
        (a, b) => getNum(a, "order") - getNum(b, "order"),
      ),
    [board.columns],
  );

  const columnIdList = useMemo(() => columns.map((c) => c.id), [columns]);

  const columnMap = useMemo(() => {
    const map = new Map<string, Entity>();
    for (const col of columns) map.set(col.id, col);
    return map;
  }, [columns]);

  return { columns, columnIdList, columnMap };
}

/** Tasks bucketed into their target columns plus the "first todo" pointer. */
interface ColumnTaskBuckets {
  taskMap: Map<string, Entity>;
  baseLayout: ColumnLayout;
  columnTasks: Map<string, Entity[]>;
  firstTodoTaskId: string | null;
}

/**
 * Build a column-keyed map of sorted task-id arrays.
 *
 * Each column gets a bucket of task ids sorted by group (when applicable)
 * then by ordinal. Pure function — suitable for `useMemo`.
 */
function buildBaseLayout(
  columns: Entity[],
  tasks: Entity[],
  taskMap: Map<string, Entity>,
  groupField: string | undefined,
  groupValue: string | undefined,
): ColumnLayout {
  const map: ColumnLayout = new Map();
  for (const col of columns) map.set(col.id, []);
  for (const task of tasks)
    map.get(getStr(task, "position_column"))?.push(task.id);
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
}

/**
 * Bucket tasks into their target columns and pre-sort each bucket.
 *
 * Output order within each bucket honors the active grouping (when any) and
 * then ordinal. Also exposes the first task in the todo column for the
 * "do this next" helper.
 */
function useColumnTaskBuckets(
  columns: Entity[],
  tasks: Entity[],
  groupField: string | undefined,
  groupValue: string | undefined,
): ColumnTaskBuckets {
  const taskMap = useMemo(() => {
    const map = new Map<string, Entity>();
    for (const task of tasks) map.set(task.id, task);
    return map;
  }, [tasks]);

  const baseLayout = useMemo(
    () => buildBaseLayout(columns, tasks, taskMap, groupField, groupValue),
    [columns, tasks, taskMap, groupField, groupValue],
  );

  const columnTasks = useMemo(() => {
    const map = new Map<string, Entity[]>();
    for (const col of columns) {
      const ids = baseLayout.get(col.id) ?? [];
      map.set(
        col.id,
        ids
          .map((id) => taskMap.get(id))
          .filter((t): t is Entity => t !== undefined),
      );
    }
    return map;
  }, [columns, baseLayout, taskMap]);

  const firstTodoTaskId = useMemo(() => {
    if (columns.length === 0) return null;
    const ids = baseLayout.get(columns[0].id);
    return ids && ids.length > 0 ? ids[0] : null;
  }, [columns, baseLayout]);

  return { taskMap, baseLayout, columnTasks, firstTodoTaskId };
}

/**
 * Build per-column task moniker lists for initial focus seeding.
 *
 * Preserves display order within each column so `useInitialBoardFocus` can
 * select the first card of the first non-empty column.
 */
function useColumnTaskMonikers(
  columns: Entity[],
  baseLayout: ColumnLayout,
  taskMap: Map<string, Entity>,
): Map<string, string[]> {
  return useMemo(() => {
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
}

/**
 * Derive all board layout data from raw board/task props.
 *
 * Thin composer over column ordering, task bucketing, and moniker
 * construction. See each sub-hook for specifics.
 */
function useBoardLayout(
  board: BoardData,
  tasks: Entity[],
  groupValue: string | undefined,
): BoardLayoutResult {
  const { groupField } = useActivePerspective();
  const { columns, columnIdList, columnMap } = useColumnOrdering(board);
  const { taskMap, baseLayout, columnTasks, firstTodoTaskId } =
    useColumnTaskBuckets(columns, tasks, groupField, groupValue);
  const columnTaskMonikers = useColumnTaskMonikers(
    columns,
    baseLayout,
    taskMap,
  );

  return {
    columns,
    columnIdList,
    // Filtering is server-side; alias for clarity with downstream consumers.
    filteredTasks: tasks,
    taskMap,
    columnMap,
    baseLayout,
    columnTasks,
    firstTodoTaskId,
    columnTaskMonikers,
  };
}

/** @dnd-kit column-drag state and the three drag lifecycle handlers. */
interface ColumnDragHandlers {
  activeColumn: Entity | null;
  currentColumnOrder: string[];
  handleColumnDragStart: (event: DragStartEvent) => void;
  handleColumnDragOver: (event: DragOverEvent) => void;
  handleColumnDragEnd: (event: DragEndEvent) => void;
}

/**
 * Manage @dnd-kit-driven column reordering with optimistic ordering.
 *
 * While a drag is in flight, `currentColumnOrder` reflects the optimistic
 * position. After `drag end` the hook dispatches `column.reorder` and keeps
 * the optimistic order visible until the backend refresh updates
 * `columnIdList`, at which point the `useEffect` below clears it.
 */
/**
 * Compute the next virtual column order in response to a drag-over event.
 *
 * Returns the current order unchanged when the drag is self-targeting or
 * either id is missing — callers use referential equality to decide whether
 * to commit the update.
 */
function computeDragOverOrder(event: DragOverEvent, order: string[]): string[] {
  const { active, over } = event;
  if (!over) return order;
  const activeId = active.id as string;
  const overId = over.id as string;
  if (activeId === overId) return order;
  const oldIndex = order.indexOf(activeId);
  const newIndex = order.indexOf(overId);
  if (oldIndex === -1 || newIndex === -1) return order;
  return arrayMove(order, oldIndex, newIndex);
}

/**
 * Build the async `onDragEnd` handler for a column reorder.
 *
 * Factored out so `useColumnDragHandlers` stays short — the drag-end logic
 * is ~20 lines of validation + a dispatch call + optimistic-state cleanup.
 */
function useColumnDragEndHandler(
  columnIdList: string[],
  virtualColumnOrder: string[] | null,
  setVirtualColumnOrder: (v: string[] | null) => void,
  setActiveColumn: (c: Entity | null) => void,
): (event: DragEndEvent) => Promise<void> {
  const dispatch = useDispatchCommand();
  return useCallback(
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
    [
      virtualColumnOrder,
      columnIdList,
      dispatch,
      setVirtualColumnOrder,
      setActiveColumn,
    ],
  );
}

/**
 * Manage @dnd-kit-driven column reordering with optimistic ordering.
 *
 * While a drag is in flight, `currentColumnOrder` reflects the optimistic
 * position. After drag-end the hook dispatches `column.reorder` and keeps
 * the optimistic order visible until the backend refresh updates
 * `columnIdList`.
 */
function useColumnDragHandlers(
  columnIdList: string[],
  columnMap: Map<string, Entity>,
): ColumnDragHandlers {
  const [activeColumn, setActiveColumn] = useState<Entity | null>(null);
  const [virtualColumnOrder, setVirtualColumnOrder] = useState<string[] | null>(
    null,
  );
  const currentColumnOrder = virtualColumnOrder ?? columnIdList;

  // Clear virtual column order when real data catches up from the backend.
  useEffect(() => {
    setVirtualColumnOrder(null);
  }, [columnIdList]);

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
      if (!virtualColumnOrder) return;
      const next = computeDragOverOrder(event, virtualColumnOrder);
      if (next !== virtualColumnOrder) setVirtualColumnOrder(next);
    },
    [virtualColumnOrder],
  );

  const handleColumnDragEnd = useColumnDragEndHandler(
    columnIdList,
    virtualColumnOrder,
    setVirtualColumnOrder,
    setActiveColumn,
  );

  return {
    activeColumn,
    currentColumnOrder,
    handleColumnDragStart,
    handleColumnDragOver,
    handleColumnDragEnd,
  };
}

/** HTML5 task-drag state and its three lifecycle handlers. */
interface TaskDragHandlers {
  taskDrag: TaskDragState | null;
  handleTaskDragStart: (entity: Entity) => void;
  handleTaskDragEnd: (entity: Entity, dropEffect: string) => void;
  handleZoneDrop: (descriptor: DropZoneDescriptor, taskData: string) => void;
}

/**
 * Bind a window-level Escape handler that cancels an active task drag.
 *
 * The backend drag session is separate from the HTML5 drag; Escape must
 * cancel both so downstream listeners don't think a drag is still in flight.
 */
function useTaskDragEscapeCancel(
  taskDrag: TaskDragState | null,
  cancelSession: () => void,
  setTaskDrag: (v: TaskDragState | null) => void,
): void {
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
  }, [taskDrag, cancelSession, setTaskDrag]);
}

/**
 * Build the `persistMove` callback that dispatches `task.move` for a drop.
 *
 * Board identity is resolved from the scope chain by `useDispatchCommand` —
 * callers only need to pass the drop descriptor and task id.
 */
function usePersistTaskMove(): (
  descriptor: DropZoneDescriptor,
  taskId: string,
) => Promise<void> {
  const dispatch = useDispatchCommand();
  return useCallback(
    async (descriptor: DropZoneDescriptor, taskId: string) => {
      try {
        const args: Record<string, unknown> = {
          id: taskId,
          column: descriptor.columnId,
        };
        if (descriptor.beforeId) args.before_id = descriptor.beforeId;
        if (descriptor.afterId) args.after_id = descriptor.afterId;
        await dispatch("task.move", { args, target: `task:${taskId}` });
      } catch (e) {
        console.error("Failed to move task:", e);
      }
    },
    [dispatch],
  );
}

/**
 * Build the zone-drop handler that routes same-board vs cross-board drops.
 *
 * Same-board drops dispatch `task.move` directly. Cross-board drops forward
 * placement to the drag session for the source window to handle.
 */
function useZoneDropHandler(
  taskMap: Map<string, Entity>,
  setTaskDrag: (v: TaskDragState | null) => void,
  persistMove: (d: DropZoneDescriptor, id: string) => Promise<void>,
  cancelSession: () => void,
  completeSession: (
    col: string,
    placement: { beforeId?: string; afterId?: string },
  ) => void,
) {
  return useCallback(
    (descriptor: DropZoneDescriptor, taskData: string) => {
      setTaskDrag(null);
      const entity = parseTaskDropPayload(taskData);
      if (!entity) {
        cancelSession();
        return;
      }
      if (taskMap.has(entity.id)) {
        cancelSession();
        persistMove(descriptor, entity.id);
      } else {
        completeSession(descriptor.columnId, {
          beforeId: descriptor.beforeId,
          afterId: descriptor.afterId,
        });
      }
    },
    [taskMap, persistMove, cancelSession, completeSession, setTaskDrag],
  );
}

/**
 * Manage HTML5-drag task state: start, end, and the drop-zone handler.
 *
 * Delegates Escape cancellation and `task.move` dispatch to dedicated hooks
 * so this composer stays compact. Cross-board drops are forwarded to the
 * drag session; same-board drops persist directly.
 */
function useTaskDragHandlers(taskMap: Map<string, Entity>): TaskDragHandlers {
  const { startSession, cancelSession, completeSession } = useDragSession();
  const [taskDrag, setTaskDrag] = useState<TaskDragState | null>(null);
  useTaskDragEscapeCancel(taskDrag, cancelSession, setTaskDrag);
  const persistMove = usePersistTaskMove();

  const handleTaskDragStart = useCallback(
    (entity: Entity) => {
      const sourceColumn = getStr(entity, "position_column");
      setTaskDrag({ sourceTaskId: entity.id, sourceColumn });
      startSession(entity.id, entity.fields, false);
    },
    [startSession],
  );

  const handleTaskDragEnd = useCallback(
    (_entity: Entity, dropEffect: string) => {
      setTaskDrag(null);
      emit("drag-ended", {});
      if (dropEffect === "none") cancelSession();
    },
    [cancelSession],
  );

  const handleZoneDrop = useZoneDropHandler(
    taskMap,
    setTaskDrag,
    persistMove,
    cancelSession,
    completeSession,
  );

  return { taskDrag, handleTaskDragStart, handleTaskDragEnd, handleZoneDrop };
}

/**
 * Parse the JSON string carried on an HTML5 drag's dataTransfer.
 *
 * Returns `null` when the payload is empty or malformed — callers treat that
 * as "rejected drop" and cancel the session.
 */
function parseTaskDropPayload(taskData: string): Entity | null {
  if (!taskData) return null;
  try {
    return JSON.parse(taskData) as Entity;
  } catch {
    return null;
  }
}

/** Return value from useBoardDragDrop — drag state and all event handlers. */
interface BoardDragDropResult extends ColumnDragHandlers, TaskDragHandlers {
  sensors: ReturnType<typeof useSensors>;
}

/**
 * Compose column and task drag handlers and expose the @dnd-kit sensors.
 *
 * The two concerns (column reordering via @dnd-kit vs task dragging via
 * HTML5 drag) live in their own hooks — this hook is just wiring.
 */
function useBoardDragDrop(
  columnIdList: string[],
  columnMap: Map<string, Entity>,
  taskMap: Map<string, Entity>,
): BoardDragDropResult {
  const columnDrag = useColumnDragHandlers(columnIdList, columnMap);
  const taskDragHandlers = useTaskDragHandlers(taskMap);

  // @dnd-kit sensors — columns only
  const sensors = useSensors(
    useSensor(PointerSensor, {
      activationConstraint: { distance: 5 },
    }),
  );

  return { ...columnDrag, ...taskDragHandlers, sensors };
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
 * Resolve the column id the user is focused on (or the leftmost as fallback).
 *
 * A column moniker (`column:<id>`) resolves directly. A task moniker
 * (`task:<id>`) resolves to its home column via `taskMap`. Any other or
 * missing moniker falls back to the first column.
 */
function resolveFocusedColumnId(
  focusedMoniker: string | null,
  columns: Entity[],
  taskMap: Map<string, Entity>,
): string | null {
  const fallback = columns[0]?.id ?? null;
  if (!focusedMoniker) return fallback;
  if (focusedMoniker.startsWith("column:"))
    return focusedMoniker.slice("column:".length);
  if (focusedMoniker.startsWith("task:")) {
    const entity = taskMap.get(focusedMoniker.slice("task:".length));
    if (entity) return getStr(entity, "position_column") || fallback;
  }
  return fallback;
}

/** Shared dependencies passed to each board-action command factory. */
interface BoardActionDeps {
  columns: Entity[];
  taskMap: Map<string, Entity>;
  focusedMonikerRef: React.RefObject<string | null>;
  /**
   * Ambient dispatcher used by navigation aliases like
   * `board.firstColumn`/`board.lastColumn`. These dispatch `nav.first`/
   * `nav.last` through the unified pipeline to the Rust NavigateCmd impl
   * — no local side-channel.
   */
  dispatchRef: React.RefObject<
    (cmd: string, opts?: DispatchOptions) => Promise<unknown>
  >;
  handleAddTaskRef: React.RefObject<(columnId: string) => void>;
  dispatchInspect: ReturnType<typeof useDispatchCommand>;
}

/**
 * Factory for the "inspect focused entity" command.
 *
 * Bound to Space (the universal "inspect / peek" key across the app);
 * Enter is reserved for "activate / drill into" verbs on a given scope.
 */
function makeInspectCommand(deps: BoardActionDeps): CommandDef {
  return {
    id: "board.inspect",
    name: "Inspect",
    keys: { vim: "Space", cua: "Space" },
    execute: () => {
      const fm = deps.focusedMonikerRef.current;
      if (fm) deps.dispatchInspect({ target: fm }).catch(console.error);
    },
  };
}

/** Factory for the "create task in focused column" command. */
function makeNewTaskCommand(deps: BoardActionDeps): CommandDef {
  return {
    id: "board.newTask",
    name: "New Task",
    keys: { vim: "o", cua: "Mod+Enter" },
    execute: () => {
      const colId = resolveFocusedColumnId(
        deps.focusedMonikerRef.current,
        deps.columns,
        deps.taskMap,
      );
      if (colId) deps.handleAddTaskRef.current(colId);
    },
  };
}

/** Factory for a nav-dispatch command (first/last column). */
function makeNavDispatchCommand(
  deps: BoardActionDeps,
  id: string,
  name: string,
  keys: CommandDef["keys"],
  navCmd: string,
): CommandDef {
  return {
    id,
    name,
    keys,
    execute: () => {
      if (deps.columns.length > 0) {
        deps.dispatchRef
          .current(navCmd)
          .catch((e) => console.error(`${navCmd} failed:`, e));
      }
    },
  };
}

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
  dispatchRef: React.RefObject<
    (cmd: string, opts?: DispatchOptions) => Promise<unknown>
  >,
  handleAddTaskRef: React.RefObject<(columnId: string) => void>,
  dispatchInspect: ReturnType<typeof useDispatchCommand>,
): CommandDef[] {
  return useMemo<CommandDef[]>(() => {
    const deps: BoardActionDeps = {
      columns,
      taskMap,
      focusedMonikerRef,
      dispatchRef,
      handleAddTaskRef,
      dispatchInspect,
    };
    return [
      makeInspectCommand(deps),
      makeNewTaskCommand(deps),
      makeNavDispatchCommand(
        deps,
        "board.firstColumn",
        "First Column",
        { vim: "0", cua: "Mod+Home" },
        "nav.first",
      ),
      makeNavDispatchCommand(
        deps,
        "board.lastColumn",
        "Last Column",
        { vim: "$", cua: "Mod+End" },
        "nav.last",
      ),
    ];
  }, [
    columns,
    taskMap,
    dispatchInspect,
    focusedMonikerRef,
    dispatchRef,
    handleAddTaskRef,
  ]);
}

/**
 * Scroll the focused moniker's element into view within the board strip.
 *
 * Kept as its own hook so the main BoardView body stays short.
 */
function useScrollFocusedIntoView(
  scrollContainerRef: React.RefObject<HTMLDivElement | null>,
  focusedMoniker: string | null,
): void {
  useEffect(() => {
    const container = scrollContainerRef.current;
    if (!container || !focusedMoniker) return;
    const el = container.querySelector<HTMLElement>(
      `[data-moniker="${CSS.escape(focusedMoniker)}"]`,
    );
    if (el?.scrollIntoView)
      el.scrollIntoView({ inline: "nearest", block: "nearest" });
  }, [scrollContainerRef, focusedMoniker]);
}

/**
 * Focus the first task (or first column header) exactly once on mount.
 *
 * Subsequent focus changes are driven by the Rust spatial navigation layer.
 */
function useInitialBoardFocus(
  columns: Entity[],
  columnTaskMonikers: Map<string, string[]>,
  setFocus: (moniker: string) => void,
): void {
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
}

/**
 * Build the `onAddTask` callback that creates a task in the given column.
 *
 * On success, focus moves to the newly-created task. On failure, surfaces
 * the error via a toast.
 */
function useAddTaskHandler(
  columnMap: Map<string, Entity>,
  setFocus: (moniker: string) => void,
): (columnId: string) => Promise<void> {
  const dispatch = useDispatchCommand();
  return useCallback(
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
    [columnMap, setFocus, dispatch],
  );
}

/** Props for a single positioned column inside the strip. */
interface BoardColumnItemProps {
  col: Entity;
  index: number;
  layout: BoardLayoutResult;
  taskDrag: TaskDragState | null;
  handleAddTask: (columnId: string) => void;
  handleTaskDragStart: (entity: Entity) => void;
  handleTaskDragEnd: (entity: Entity, dropEffect: string) => void;
  handleZoneDrop: (descriptor: DropZoneDescriptor, taskData: string) => void;
}

/**
 * Render one sortable column inside the board strip.
 *
 * Kept as its own component so the strip map body stays tiny.
 */
function BoardColumnItem({
  col,
  index,
  layout,
  taskDrag,
  handleAddTask,
  handleTaskDragStart,
  handleTaskDragEnd,
  handleZoneDrop,
}: BoardColumnItemProps) {
  const { columnTasks, firstTodoTaskId } = layout;
  return (
    <SortableColumn id={col.id} showSeparator={index > 0}>
      <ColumnView
        column={col}
        tasks={columnTasks.get(col.id) ?? []}
        onAddTask={index === 0 ? handleAddTask : undefined}
        onTaskDragStart={handleTaskDragStart}
        onTaskDragEnd={handleTaskDragEnd}
        onDrop={handleZoneDrop}
        dragTaskId={taskDrag?.sourceTaskId ?? null}
        firstTodoTaskId={firstTodoTaskId}
      />
    </SortableColumn>
  );
}

/** Props for the column strip sub-component. */
interface BoardColumnStripProps {
  layout: BoardLayoutResult;
  currentColumnOrder: string[];
  taskDrag: TaskDragState | null;
  handleAddTask: (columnId: string) => void;
  handleTaskDragStart: (entity: Entity) => void;
  handleTaskDragEnd: (entity: Entity, dropEffect: string) => void;
  handleZoneDrop: (descriptor: DropZoneDescriptor, taskData: string) => void;
}

/**
 * Render the horizontal strip of sortable columns inside the scroll container.
 *
 * Wraps the column list in a @dnd-kit SortableContext and delegates each
 * slot to `BoardColumnItem`.
 */
function BoardColumnStrip({
  layout,
  currentColumnOrder,
  taskDrag,
  handleAddTask,
  handleTaskDragStart,
  handleTaskDragEnd,
  handleZoneDrop,
}: BoardColumnStripProps) {
  const { columnMap } = layout;
  return (
    <SortableContext
      items={currentColumnOrder}
      strategy={horizontalListSortingStrategy}
    >
      {currentColumnOrder.map((colId, i) => {
        const col = columnMap.get(colId);
        if (!col) return null;
        return (
          <BoardColumnItem
            key={col.id}
            col={col}
            index={i}
            layout={layout}
            taskDrag={taskDrag}
            handleAddTask={handleAddTask}
            handleTaskDragStart={handleTaskDragStart}
            handleTaskDragEnd={handleTaskDragEnd}
            handleZoneDrop={handleZoneDrop}
          />
        );
      })}
    </SortableContext>
  );
}

/** Props for the inner DnD-wrapped board body. */
interface BoardDndWrapperProps {
  scrollContainerRef: React.RefObject<HTMLDivElement | null>;
  dragDrop: BoardDragDropResult;
  layout: BoardLayoutResult;
  handleAddTask: (columnId: string) => void;
}

/**
 * Wrap the column strip in a DndContext plus the `min-w-0` scroll container
 * that owns horizontal overflow. Kept separate so `BoardView` stays focused
 * on composing scope/command providers.
 *
 * The `min-w-0 overflow-x-auto` classes on the inner div are load-bearing —
 * they keep the column strip from propagating its intrinsic width up through
 * flex ancestors and scrolling the app chrome.
 */
function BoardDndWrapper({
  scrollContainerRef,
  dragDrop,
  layout,
  handleAddTask,
}: BoardDndWrapperProps) {
  return (
    <DndContext
      sensors={dragDrop.sensors}
      onDragStart={dragDrop.handleColumnDragStart}
      onDragOver={dragDrop.handleColumnDragOver}
      onDragEnd={dragDrop.handleColumnDragEnd}
    >
      <div
        ref={scrollContainerRef}
        className="flex flex-1 min-h-0 min-w-0 overflow-x-auto pl-2"
      >
        <BoardColumnStrip
          layout={layout}
          currentColumnOrder={dragDrop.currentColumnOrder}
          taskDrag={dragDrop.taskDrag}
          handleAddTask={handleAddTask}
          handleTaskDragStart={dragDrop.handleTaskDragStart}
          handleTaskDragEnd={dragDrop.handleTaskDragEnd}
          handleZoneDrop={dragDrop.handleZoneDrop}
        />
      </div>
      <BoardDragOverlay activeColumn={dragDrop.activeColumn} />
    </DndContext>
  );
}

/** Mutable refs BoardView threads into its action-command factories. */
interface BoardCommandRefs {
  focusedMonikerRef: React.RefObject<string | null>;
  dispatchRef: React.RefObject<
    (cmd: string, opts?: DispatchOptions) => Promise<unknown>
  >;
  handleAddTaskRef: React.RefObject<(columnId: string) => void>;
}

/**
 * Allocate and keep up-to-date the refs used by board action commands.
 *
 * The commands are memoized but need to see the latest focused moniker and
 * add-task callback; refs avoid rebuilding the command list on every render.
 */
function useBoardCommandRefs(
  focusedMoniker: string | null,
  dispatch: (cmd: string, opts?: DispatchOptions) => Promise<unknown>,
): BoardCommandRefs {
  const focusedMonikerRef = useRef(focusedMoniker);
  focusedMonikerRef.current = focusedMoniker;
  const dispatchRef = useRef(dispatch);
  dispatchRef.current = dispatch;
  const handleAddTaskRef = useRef<(columnId: string) => void>(() => {});
  return { focusedMonikerRef, dispatchRef, handleAddTaskRef };
}

/**
 * Board view that renders columns and cards.
 *
 * Cardinal direction navigation is handled by the Rust spatial navigation
 * layer which computes focus targets from DOM rects at runtime.
 */
export function BoardView({ board, tasks, groupValue }: BoardViewProps) {
  const boardCommands = useEntityCommands("board", "board");
  const dispatchInspect = useDispatchCommand("ui.inspect");
  const dispatch = useDispatchCommand();
  const { setFocus } = useEntityFocus();
  const focusedMoniker = useFocusedMoniker();
  const scrollContainerRef = useRef<HTMLDivElement>(null);
  const { focusedMonikerRef, dispatchRef, handleAddTaskRef } =
    useBoardCommandRefs(focusedMoniker, dispatch);

  const layout = useBoardLayout(board, tasks, groupValue);
  const dragDrop = useBoardDragDrop(
    layout.columnIdList,
    layout.columnMap,
    layout.taskMap,
  );

  const boardActionCommands = useBoardActionCommands(
    layout.columns,
    layout.taskMap,
    focusedMonikerRef,
    dispatchRef,
    handleAddTaskRef,
    dispatchInspect,
  );

  useScrollFocusedIntoView(scrollContainerRef, focusedMoniker);
  useInitialBoardFocus(layout.columns, layout.columnTaskMonikers, setFocus);

  const handleAddTask = useAddTaskHandler(layout.columnMap, setFocus);
  handleAddTaskRef.current = handleAddTask;

  return (
    <FocusScope
      moniker={board.board.moniker}
      commands={boardCommands}
      className="flex flex-col flex-1 min-h-0 relative"
    >
      <CommandScopeProvider commands={boardActionCommands}>
        <BoardDndWrapper
          scrollContainerRef={scrollContainerRef}
          dragDrop={dragDrop}
          layout={layout}
          handleAddTask={handleAddTask}
        />
      </CommandScopeProvider>
    </FocusScope>
  );
}
