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
import {
  useFocusActions,
  useFocusedMoniker,
  useFocusedMonikerRef,
} from "@/lib/entity-focus-context";
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

/** Tasks bucketed into their target columns. */
interface ColumnTaskBuckets {
  taskMap: Map<string, Entity>;
  baseLayout: ColumnLayout;
  columnTasks: Map<string, Entity[]>;
}

/**
 * Bucket tasks into their target columns and pre-sort each bucket.
 *
 * Output order within each bucket honors the active grouping (when any) and
 * then ordinal.
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

  const baseLayout = useMemo<ColumnLayout>(() => {
    const map: ColumnLayout = new Map();
    for (const col of columns) map.set(col.id, []);
    for (const task of tasks) {
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
  }, [columns, tasks, taskMap, groupField, groupValue]);

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

  return { taskMap, baseLayout, columnTasks };
}

/** Moniker tables needed for cross-column keyboard navigation. */
interface BoardMonikers {
  columnTaskMonikers: Map<string, string[]>;
  allBoardTaskMonikers: Set<string>;
  allBoardHeaderMonikers: Set<string>;
}

/**
 * Build the moniker tables that drive cross-column nav predicates.
 *
 * - `columnTaskMonikers` preserves per-column task moniker order so each
 *   column can expose its left/right neighbor's moniker list.
 * - The two `allBoard*` sets drive nav.first / nav.last claim predicates.
 */
function useBoardMonikers(
  columns: Entity[],
  baseLayout: ColumnLayout,
  taskMap: Map<string, Entity>,
): BoardMonikers {
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

  return { columnTaskMonikers, allBoardTaskMonikers, allBoardHeaderMonikers };
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
  columnTaskMonikers: Map<string, string[]>;
  allBoardTaskMonikers: Set<string>;
  allBoardHeaderMonikers: Set<string>;
}

/**
 * Derive all board layout data from raw board/task props.
 *
 * Thin composer over three focused hooks: column ordering, task bucketing,
 * and moniker table construction. See each sub-hook for specifics.
 */
function useBoardLayout(
  board: BoardData,
  tasks: Entity[],
  groupValue: string | undefined,
): BoardLayoutResult {
  const { groupField } = useActivePerspective();
  const { columns, columnIdList, columnMap } = useColumnOrdering(board);
  const { taskMap, baseLayout, columnTasks } = useColumnTaskBuckets(
    columns,
    tasks,
    groupField,
    groupValue,
  );
  const { columnTaskMonikers, allBoardTaskMonikers, allBoardHeaderMonikers } =
    useBoardMonikers(columns, baseLayout, taskMap);

  return {
    columns,
    columnIdList,
    // Filtering is server-side; alias for clarity with downstream consumers.
    filteredTasks: tasks,
    taskMap,
    columnMap,
    baseLayout,
    columnTasks,
    columnTaskMonikers,
    allBoardTaskMonikers,
    allBoardHeaderMonikers,
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
      // Only cancel the backend session if the drop was rejected (no valid target).
      // Successful drops are handled by handleZoneDrop which calls persistMove
      // or completeSession directly.
      if (dropEffect === "none") cancelSession();
    },
    [cancelSession],
  );

  const handleZoneDrop = useCallback(
    (descriptor: DropZoneDescriptor, taskData: string) => {
      setTaskDrag(null);
      const entity = parseTaskDropPayload(taskData);
      if (!entity) {
        cancelSession();
        return;
      }
      if (taskMap.has(entity.id)) {
        // Same-board drop — descriptor carries all placement params
        cancelSession();
        persistMove(descriptor, entity.id);
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

/** Shared dependencies passed to each board-action command factory. */
interface BoardActionDeps {
  columns: Entity[];
  focusedMonikerRef: React.RefObject<string | null>;
  broadcastRef: React.RefObject<(cmd: string) => void>;
  dispatchInspect: ReturnType<typeof useDispatchCommand>;
  dispatchEntityAddTask: ReturnType<typeof useDispatchCommand>;
  setFocus: (moniker: string) => void;
}

/** Factory for the "inspect focused entity" command. */
function makeInspectCommand(deps: BoardActionDeps): CommandDef {
  return {
    id: "board.inspect",
    name: "Inspect",
    keys: { vim: "Enter", cua: "Enter" },
    execute: () => {
      const fm = deps.focusedMonikerRef.current;
      if (fm) deps.dispatchInspect({ target: fm }).catch(console.error);
    },
  };
}

/** Factory for the "create task in focused column" command.
 *
 * Dispatches the unified `entity.add:task` with no `column` arg. The
 * backend resolves the target column from the scope chain — which the
 * dispatcher already carries — via
 * `swissarmyhammer_kanban::focus::resolve_focused_column` inside
 * `AddEntityCmd`. That matches the React flow that used to live here as
 * `resolveFocusedColumnId`: a focused `column:<id>` moniker routes the
 * new task into that column; a focused `task:<id>` moniker routes it
 * into the focused task's home column; anything else falls through to
 * the lowest-order column in `AddEntity::apply_position`.
 *
 * Per PR #40 review — column resolution is business logic, not
 * presentation; it belongs in headless Rust (see
 * `swissarmyhammer-kanban/src/focus.rs`).
 */
function makeNewTaskCommand(deps: BoardActionDeps): CommandDef {
  return {
    id: "board.newTask",
    name: "New Task",
    keys: { vim: "o", cua: "Mod+Enter" },
    execute: () => {
      if (deps.columns.length === 0) return;
      deps
        .dispatchEntityAddTask()
        .then((result) => {
          const id = (result as { id?: string } | undefined)?.id;
          if (id) deps.setFocus(`task:${id}`);
        })
        .catch((e) => {
          toast.error(
            `Failed to add task: ${e instanceof Error ? e.message : String(e)}`,
          );
        });
    },
  };
}

/** Factory for a nav-broadcast command (first/last column). */
function makeNavBroadcastCommand(
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
      if (deps.columns.length > 0) deps.broadcastRef.current(navCmd);
    },
  };
}

/**
 * Board-level action commands: inspect, new task, first/last column navigation.
 *
 * Uses refs for focused moniker and broadcast callback to avoid rebuilding
 * the command list on every focus change.
 */
function useBoardActionCommands(
  columns: Entity[],
  focusedMonikerRef: React.RefObject<string | null>,
  broadcastRef: React.RefObject<(cmd: string) => void>,
  dispatchInspect: ReturnType<typeof useDispatchCommand>,
  dispatchEntityAddTask: ReturnType<typeof useDispatchCommand>,
  setFocus: (moniker: string) => void,
): CommandDef[] {
  return useMemo<CommandDef[]>(() => {
    const deps: BoardActionDeps = {
      columns,
      focusedMonikerRef,
      broadcastRef,
      dispatchInspect,
      dispatchEntityAddTask,
      setFocus,
    };
    return [
      makeInspectCommand(deps),
      makeNewTaskCommand(deps),
      makeNavBroadcastCommand(
        deps,
        "board.firstColumn",
        "First Column",
        { vim: "0", cua: "Mod+Home" },
        "nav.first",
      ),
      makeNavBroadcastCommand(
        deps,
        "board.lastColumn",
        "Last Column",
        { vim: "$", cua: "Mod+End" },
        "nav.last",
      ),
    ];
  }, [
    columns,
    dispatchInspect,
    dispatchEntityAddTask,
    focusedMonikerRef,
    broadcastRef,
    setFocus,
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
 * Subsequent focus is driven by pull-based claimWhen predicates — we only
 * need to seed the initial selection.
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
 * Dispatches the unified `entity.add:task` command that the grid view and the
 * palette also route through — the backend `AddEntity` operation honours the
 * `column` override, so a single creation path serves every UI entry point.
 *
 * On success, focus moves to the newly-created task. On failure, surfaces
 * the error via a toast.
 */
function useAddTaskHandler(
  setFocus: (moniker: string) => void,
): (columnId: string) => Promise<void> {
  const dispatch = useDispatchCommand();
  return useCallback(
    async (columnId: string) => {
      try {
        const result = (await dispatch("entity.add:task", {
          args: { column: columnId },
        })) as { id?: string } | undefined;
        if (result?.id) setFocus(`task:${result.id}`);
      } catch (e) {
        toast.error(
          `Failed to add task: ${e instanceof Error ? e.message : String(e)}`,
        );
      }
    },
    [setFocus, dispatch],
  );
}

/**
 * Build the header-moniker string for a neighbor column, or null.
 *
 * Neighbor columns expose a `<moniker>.name` target that `ColumnView` uses
 * to wire its cross-column nav predicates; callers pass `null` when there
 * is no neighbor on that side.
 */
function neighborHeaderMoniker(
  neighborId: string | null,
  columnMap: Map<string, Entity>,
): string | null {
  if (!neighborId) return null;
  const col = columnMap.get(neighborId);
  return `${col?.moniker ?? `column:${neighborId}`}.name`;
}

/** Props for a single positioned column inside the strip. */
interface BoardColumnItemProps {
  col: Entity;
  index: number;
  total: number;
  prevColId: string | null;
  nextColId: string | null;
  layout: BoardLayoutResult;
  taskDrag: TaskDragState | null;
  handleAddTask: (columnId: string) => void;
  handleTaskDragStart: (entity: Entity) => void;
  handleTaskDragEnd: (entity: Entity, dropEffect: string) => void;
  handleZoneDrop: (descriptor: DropZoneDescriptor, taskData: string) => void;
}

/**
 * Render one sortable column with its neighbor moniker wiring.
 *
 * Kept as its own component so the strip map body stays tiny — only
 * `BoardColumnStrip` knows about neighbor indices; this component takes
 * the resolved ids as props.
 */
function BoardColumnItem({
  col,
  index,
  total,
  prevColId,
  nextColId,
  layout,
  taskDrag,
  handleAddTask,
  handleTaskDragStart,
  handleTaskDragEnd,
  handleZoneDrop,
}: BoardColumnItemProps) {
  const {
    columnMap,
    columnTasks,
    columnTaskMonikers,
    allBoardTaskMonikers,
    allBoardHeaderMonikers,
  } = layout;
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
        leftColumnTaskMonikers={
          prevColId ? (columnTaskMonikers.get(prevColId) ?? []) : []
        }
        leftColumnHeaderMoniker={neighborHeaderMoniker(prevColId, columnMap)}
        rightColumnTaskMonikers={
          nextColId ? (columnTaskMonikers.get(nextColId) ?? []) : []
        }
        rightColumnHeaderMoniker={neighborHeaderMoniker(nextColId, columnMap)}
        allBoardTaskMonikers={allBoardTaskMonikers}
        allBoardHeaderMonikers={allBoardHeaderMonikers}
        isFirstColumn={index === 0}
        isLastColumn={index === total - 1}
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
        const prevColId = i > 0 ? currentColumnOrder[i - 1] : null;
        const nextColId =
          i < currentColumnOrder.length - 1 ? currentColumnOrder[i + 1] : null;
        return (
          <BoardColumnItem
            key={col.id}
            col={col}
            index={i}
            total={currentColumnOrder.length}
            prevColId={prevColId}
            nextColId={nextColId}
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
  broadcastRef: React.RefObject<(cmd: string) => void>;
}

/**
 * Allocate and keep up-to-date the refs used by board action commands.
 *
 * Focus is read through `useFocusedMonikerRef` — a subscribeAll-backed ref —
 * so BoardView does not re-render on every focus move just to keep this
 * ref current. The broadcast callback comes from the stable actions bag,
 * but we wrap it in a ref too for API symmetry with action-command
 * factories downstream that expect a mutable ref.
 */
function useBoardCommandRefs(
  broadcastNavCommand: (cmd: string) => void,
): BoardCommandRefs {
  const focusedMonikerRef = useFocusedMonikerRef();
  const broadcastRef = useRef(broadcastNavCommand);
  broadcastRef.current = broadcastNavCommand;
  return { focusedMonikerRef, broadcastRef };
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
  const dispatchInspect = useDispatchCommand("ui.inspect");
  const dispatchEntityAddTask = useDispatchCommand("entity.add:task");
  const { broadcastNavCommand, setFocus } = useFocusActions();
  const focusedMoniker = useFocusedMoniker();
  const scrollContainerRef = useRef<HTMLDivElement>(null);
  const { focusedMonikerRef, broadcastRef } =
    useBoardCommandRefs(broadcastNavCommand);

  const layout = useBoardLayout(board, tasks, groupValue);
  const dragDrop = useBoardDragDrop(
    layout.columnIdList,
    layout.columnMap,
    layout.taskMap,
  );

  const boardActionCommands = useBoardActionCommands(
    layout.columns,
    focusedMonikerRef,
    broadcastRef,
    dispatchInspect,
    dispatchEntityAddTask,
    setFocus,
  );

  useScrollFocusedIntoView(scrollContainerRef, focusedMoniker);
  useInitialBoardFocus(layout.columns, layout.columnTaskMonikers, setFocus);

  const handleAddTask = useAddTaskHandler(setFocus);

  return (
    <FocusScope
      moniker={board.board.moniker}
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
