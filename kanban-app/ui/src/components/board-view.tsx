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
import { FocusZone } from "@/components/focus-zone";
import { Inspectable } from "@/components/inspectable";
import {
  useFullyQualifiedMoniker,
  useOptionalFullyQualifiedMoniker,
} from "@/components/fully-qualified-moniker-context";
import { useOptionalEnclosingLayerFq } from "@/components/layer-fq-context";
import {
  useOptionalSpatialFocusActions,
  type SpatialFocusActions,
} from "@/lib/spatial-focus-context";
import {
  asSegment,
  composeFq,
  type Direction,
  type FullyQualifiedMoniker,
  type SegmentMoniker,
} from "@/types/spatial";
import { useFocusActions, useFocusedFq } from "@/lib/entity-focus-context";
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

/**
 * Initial-focus target descriptor.
 *
 * The board zone is at FQ `<board-fq>/ui:board`. Below that:
 *   - cards live at `<board-fq>/ui:board/<columnSegment>/<cardSegment>`
 *   - column zones live at `<board-fq>/ui:board/<columnSegment>`
 *
 * `columnSegment` carries the enclosing column's segment so callers
 * inside the board zone can compose the full FQM via two `composeFq`
 * calls; `leafSegment` carries the final segment (the card segment for
 * task targets, or `null` when the target IS the column zone).
 */
interface InitialFocusTarget {
  columnSegment: SegmentMoniker;
  leafSegment: SegmentMoniker | null;
}

/**
 * Resolve the initial focus target the board should seed on mount.
 *
 * Walks the ordered columns and returns a descriptor pointing at the
 * first task it finds (preserving the in-column ordinal order
 * established by `useColumnTaskBuckets`). When no column has any
 * tasks, returns the first column's own zone. Returns `null` when
 * the board has no columns at all.
 *
 * Once focus is seeded, the spatial-nav layer drives every subsequent
 * traversal via the `<FocusZone>` graph — so a single seed call is
 * all `useInitialBoardFocus` needs.
 */
function useInitialFocusTarget(
  columns: Entity[],
  baseLayout: ColumnLayout,
  taskMap: Map<string, Entity>,
): InitialFocusTarget | null {
  return useMemo(() => {
    for (const col of columns) {
      const taskIds = baseLayout.get(col.id) ?? [];
      if (taskIds.length > 0) {
        const firstId = taskIds[0];
        const taskSegment =
          taskMap.get(firstId)?.moniker ?? `task:${firstId}`;
        return {
          columnSegment: asSegment(col.moniker),
          leafSegment: asSegment(taskSegment),
        };
      }
    }
    if (columns.length > 0) {
      return {
        columnSegment: asSegment(columns[0].moniker),
        leafSegment: null,
      };
    }
    return null;
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
  initialFocusTarget: InitialFocusTarget | null;
}

/**
 * Derive all board layout data from raw board/task props.
 *
 * Thin composer over three focused hooks: column ordering, task bucketing,
 * and initial-focus resolution. See each sub-hook for specifics.
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
  const initialFocusTarget = useInitialFocusTarget(
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
    initialFocusTarget,
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
  /**
   * Latest spatial-focus actions ref. Read at command-execute time so
   * the column-extreme commands dispatch `spatial_navigate` against the
   * focused FQM the kernel currently knows about — without re-binding
   * the command list on every focus move.
   *
   * `null` when the board is mounted in a pre-spatial-nav harness
   * (legacy unit tests that wrap only `<EntityFocusProvider>`); in that
   * case the column-extreme commands are no-ops, matching the same
   * "no kernel, nothing to navigate" fallback that
   * `app-shell.tsx::buildNavCommands` uses.
   */
  spatialActionsRef: React.RefObject<SpatialFocusActions | null>;
  dispatchEntityAddTask: ReturnType<typeof useDispatchCommand>;
  /**
   * Focus a freshly-created task by composing the FQM from the
   * known board-zone FQM, the column segment the task lives under,
   * and the task's segment.
   *
   * Resolves the column at dispatch time via the kernel's
   * `resolve_focused_column` semantics — when the task was created
   * with no `column` arg, the kernel routed it to the focused
   * column; the React side mirrors that fallback by walking the
   * `columns` list to the first one.
   */
  focusCreatedTask: (taskId: string, columnSegment: SegmentMoniker) => void;
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
      // Default placement: the kernel's `resolve_focused_column` lands
      // the new task in the lowest-order column when no focused column
      // can be resolved. Match that fallback so the React-side focus
      // dispatch composes the right FQM.
      const fallbackColumnSegment = asSegment(deps.columns[0].moniker);
      deps
        .dispatchEntityAddTask()
        .then((result) => {
          const id = (result as { id?: string } | undefined)?.id;
          if (id) deps.focusCreatedTask(id, fallbackColumnSegment);
        })
        .catch((e) => {
          toast.error(
            `Failed to add task: ${e instanceof Error ? e.message : String(e)}`,
          );
        });
    },
  };
}

/**
 * Factory for a column-extreme nav command (first / last column).
 *
 * Dispatches `spatial_navigate(focusedFq, direction)` to the Rust
 * spatial-nav kernel directly — same wire shape as the global
 * `nav.first` / `nav.last` commands defined in
 * `app-shell.tsx::buildNavCommands`. The board defines this command
 * locally only because vim `0` / `$` and cua `Mod+Home` / `Mod+End`
 * are NOT in the global `NAV_COMMAND_SPEC` (`Home` / `End` are cua
 * there, vim has only `Shift+G` for last). Both keysets resolve to
 * the same kernel call; the command exists to fill the keymap gap.
 *
 * No broadcast indirection — earlier the closure threaded the press
 * through `FocusActions.broadcastNavCommand`, which was a no-op stub
 * that always returned `false`. That pathway has been deleted (kanban
 * task `01KQJDKBQ2VNT3SE7AN3VM2KGZ`).
 *
 * Reads the latest spatial-focus actions from `deps.spatialActionsRef`
 * at execute time so the command memo stays stable across focus moves.
 * No-ops when the kernel is unavailable (pre-spatial-nav unit harnesses)
 * or when nothing is focused — there is nothing to navigate from.
 */
function makeNavCommand(
  deps: BoardActionDeps,
  id: string,
  name: string,
  keys: CommandDef["keys"],
  direction: Direction,
): CommandDef {
  return {
    id,
    name,
    keys,
    execute: async () => {
      if (deps.columns.length === 0) return;
      const actions = deps.spatialActionsRef.current;
      if (!actions) return;
      const fq = actions.focusedFq();
      if (fq === null) return;
      await actions.navigate(fq, direction);
    },
  };
}

/**
 * Board-level action commands: new task, first/last column navigation.
 *
 * The first / last column commands fill keymap gaps the global
 * `NAV_COMMAND_SPEC` (in `app-shell.tsx`) does not cover: vim `0` /
 * `$` and cua `Mod+Home` / `Mod+End`. Both pairs map to the kernel's
 * `first` / `last` directions and dispatch `spatial_navigate` against
 * the focused FQM. The global `nav.first` / `nav.last` (cua `Home` /
 * `End`, emacs `Alt+<` / `Alt+>`, vim `Shift+G`) still own those keys
 * — these commands shadow nothing.
 *
 * Inspect is no longer a board-scope concern — it lives on each
 * `<Inspectable>` wrapper (see `inspectable.tsx`), so every
 * inspectable entity carries its own scope-level Space binding
 * regardless of which layer it is rendered in (board, inspector,
 * palette result list).
 */
function useBoardActionCommands(
  columns: Entity[],
  spatialActionsRef: React.RefObject<SpatialFocusActions | null>,
  dispatchEntityAddTask: ReturnType<typeof useDispatchCommand>,
  focusCreatedTask: (taskId: string, columnSegment: SegmentMoniker) => void,
): CommandDef[] {
  return useMemo<CommandDef[]>(() => {
    const deps: BoardActionDeps = {
      columns,
      spatialActionsRef,
      dispatchEntityAddTask,
      focusCreatedTask,
    };
    return [
      makeNewTaskCommand(deps),
      makeNavCommand(
        deps,
        "board.firstColumn",
        "First Column",
        { vim: "0", cua: "Mod+Home" },
        "first",
      ),
      makeNavCommand(
        deps,
        "board.lastColumn",
        "Last Column",
        { vim: "$", cua: "Mod+End" },
        "last",
      ),
    ];
  }, [columns, dispatchEntityAddTask, spatialActionsRef, focusCreatedTask]);
}

/**
 * Scroll the focused moniker's element into view within the board strip.
 *
 * Kept as its own hook so the main BoardView body stays short.
 */
function useScrollFocusedIntoView(
  scrollContainerRef: React.RefObject<HTMLDivElement | null>,
  focusedFq: FullyQualifiedMoniker | null,
): void {
  useEffect(() => {
    const container = scrollContainerRef.current;
    if (!container || !focusedFq) return;
    const el = container.querySelector<HTMLElement>(
      `[data-moniker="${CSS.escape(focusedFq)}"]`,
    );
    if (el?.scrollIntoView)
      el.scrollIntoView({ inline: "nearest", block: "nearest" });
  }, [scrollContainerRef, focusedFq]);
}

/**
 * Seed the spatial navigator's selection exactly once on mount.
 *
 * The spatial-nav layer ( `<FocusZone>` graph ) owns every subsequent focus
 * move once a moniker is selected, but it has no opinion about which entity
 * starts focused on a fresh mount. This hook fires that initial `setFocus`
 * call — pointing at the first task on the board, or the first column when
 * the board is empty — and then stays out of the way.
 *
 * `initialMoniker` is resolved by `useInitialFocusMoniker` and is `null`
 * only when the board has no columns at all (in which case there is nothing
 * to focus).
 */
function useInitialBoardFocus(
  initialTarget: InitialFocusTarget | null,
  boardZoneFq: FullyQualifiedMoniker,
  setFocus: (fq: FullyQualifiedMoniker | null) => void,
): void {
  const initialFocusDone = useRef(false);
  useEffect(() => {
    if (initialFocusDone.current) return;
    if (!initialTarget) return;
    initialFocusDone.current = true;
    const columnFq = composeFq(boardZoneFq, initialTarget.columnSegment);
    const targetFq =
      initialTarget.leafSegment === null
        ? columnFq
        : composeFq(columnFq, initialTarget.leafSegment);
    setFocus(targetFq);
  }, [initialTarget, boardZoneFq, setFocus]);
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
  columnMap: Map<string, Entity>,
  focusCreatedTask: (taskId: string, columnSegment: SegmentMoniker) => void,
): (columnId: string) => Promise<void> {
  const dispatch = useDispatchCommand();
  return useCallback(
    async (columnId: string) => {
      try {
        const result = (await dispatch("entity.add:task", {
          args: { column: columnId },
        })) as { id?: string } | undefined;
        if (result?.id) {
          const column = columnMap.get(columnId);
          if (column) {
            focusCreatedTask(result.id, asSegment(column.moniker));
          }
        }
      } catch (e) {
        toast.error(
          `Failed to add task: ${e instanceof Error ? e.message : String(e)}`,
        );
      }
    },
    [columnMap, focusCreatedTask, dispatch],
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
 * Render one sortable column.
 *
 * Cross-column keyboard navigation now lives in the spatial-nav layer (each
 * column is its own `<FocusZone>`), so this component no longer threads
 * neighbor moniker lists or header monikers down to `ColumnView`. Only the
 * structural / drag-drop wiring stays.
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
  const { columnTasks } = layout;
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
  spatialActionsRef: React.RefObject<SpatialFocusActions | null>;
}

/**
 * Allocate and keep up-to-date the refs used by board action commands.
 *
 * The spatial-focus actions come from `useOptionalSpatialFocusActions()`,
 * which can be `null` when `<BoardView>` is mounted outside the
 * spatial-nav stack (older unit tests). We wrap the latest value in a
 * ref so the column-extreme command factories receive a mutable handle
 * — keeping the command list memo stable across BoardView renders. The
 * commands themselves no-op when the ref is null at execute time.
 */
function useBoardCommandRefs(
  spatialActions: SpatialFocusActions | null,
): BoardCommandRefs {
  const spatialActionsRef = useRef<SpatialFocusActions | null>(spatialActions);
  spatialActionsRef.current = spatialActions;
  return { spatialActionsRef };
}

/**
 * Board view that renders columns and cards.
 *
 * Navigation flows through the spatial-nav `<FocusZone>` graph: this view
 * registers a single `ui:board` zone at its root and each column / card
 * mounts its own zone underneath. Direction keys (nav.up/down/left/right
 * and friends) are routed by the spatial navigator against that zone tree
 * — there are no claimWhen predicates and no document-level keydown
 * listeners on the board. `useInitialBoardFocus` only seeds the initial
 * selection; every subsequent move belongs to the navigator.
 */
export function BoardView({ board, tasks, groupValue }: BoardViewProps) {
  const layout = useBoardLayout(board, tasks, groupValue);
  const dragDrop = useBoardDragDrop(
    layout.columnIdList,
    layout.columnMap,
    layout.taskMap,
  );
  const scrollContainerRef = useRef<HTMLDivElement>(null);

  // The outer wrapper carries the real `board:<id>` entity moniker.
  // The `<Inspectable>` wrapper owns inspector dispatch on double-click;
  // the spatial primitive `<FocusZone>` stays pure-spatial. The inner
  // `ui:board` chrome zone (in `BoardSpatialZone`) is NOT wrapped in
  // `<Inspectable>` — only this entity wrapper is — so a double-click
  // on the board surface inspects the board.
  //
  // The outer wrapper registers as a `<FocusZone>` because its React
  // subtree contains `<BoardSpatialZone>` (a `<FocusZone>`) plus every
  // column zone, card zone, and field zone inside the board. A
  // `<FocusScope>` here would violate the kernel's path-prefix
  // scope-is-leaf invariant (every descendant FQM begins with the board
  // moniker's FQM) — see
  // `swissarmyhammer-focus/tests/scope_is_leaf.rs`. `showFocusBar={false}`
  // because `<BoardSpatialZone>` already owns the visible board chrome
  // and a focus rectangle around the entire viewport would be visual
  // noise.
  //
  // Action commands and focus dispatch live INSIDE `BoardSpatialZone`
  // because the FQM composition for `card:<id>` targets requires the
  // board zone's FQM (`<board-fq>/ui:board`) — only descendants of the
  // `<FocusZone moniker="ui:board">` see that FQM via
  // `useFullyQualifiedMoniker()`.
  return (
    <Inspectable moniker={asSegment(board.board.moniker)}>
      <FocusZone
        moniker={asSegment(board.board.moniker)}
        showFocusBar={false}
        className="flex flex-col flex-1 min-h-0 relative"
      >
        <BoardSpatialZone>
          <BoardSpatialBody
            layout={layout}
            dragDrop={dragDrop}
            scrollContainerRef={scrollContainerRef}
          />
        </BoardSpatialZone>
      </FocusZone>
    </Inspectable>
  );
}

/** Props for the spatial-zone-aware board body. */
interface BoardSpatialBodyProps {
  layout: BoardLayoutResult;
  dragDrop: BoardDragDropResult;
  scrollContainerRef: React.RefObject<HTMLDivElement | null>;
}

/**
 * Render the board content inside the `ui:board` zone.
 *
 * Mounts the action-command provider, seeds initial focus, and wires
 * `useAddTaskHandler` against the board zone FQM. Lives one level
 * deeper than `BoardView` so its hooks read the board zone FQM via
 * `useFullyQualifiedMoniker()` — which is the FQ context at this
 * depth (the ancestor `<FocusZone moniker="ui:board">` provides it).
 *
 * Production trees always mount inside the spatial-nav stack, so the
 * board zone FQM is guaranteed to be present. Pre-spatial-nav unit
 * tests that mount only `<EntityFocusProvider>` will throw from
 * `useFullyQualifiedMoniker()` — which is correct: those tests never
 * exercise the board action commands or initial-focus seeding.
 */
function BoardSpatialBody({
  layout,
  dragDrop,
  scrollContainerRef,
}: BoardSpatialBodyProps) {
  const dispatchEntityAddTask = useDispatchCommand("entity.add:task");
  const { setFocus } = useFocusActions();
  const focusedFq = useFocusedFq();
  const boardZoneFq = useFullyQualifiedMoniker();
  // The column-extreme commands (`board.firstColumn` /
  // `board.lastColumn`) dispatch `spatial_navigate` against the
  // spatial-nav kernel — same wire shape as `app-shell.tsx`'s global
  // nav commands. The actions are read through a ref so the command
  // memo stays stable across focus moves.
  const spatialActions = useOptionalSpatialFocusActions();
  const { spatialActionsRef } = useBoardCommandRefs(spatialActions);

  const focusCreatedTask = useCallback(
    (taskId: string, columnSegment: SegmentMoniker) => {
      const columnFq = composeFq(boardZoneFq, columnSegment);
      const cardFq = composeFq(columnFq, asSegment(`task:${taskId}`));
      setFocus(cardFq);
    },
    [boardZoneFq, setFocus],
  );

  const boardActionCommands = useBoardActionCommands(
    layout.columns,
    spatialActionsRef,
    dispatchEntityAddTask,
    focusCreatedTask,
  );

  useScrollFocusedIntoView(scrollContainerRef, focusedFq);
  useInitialBoardFocus(layout.initialFocusTarget, boardZoneFq, setFocus);

  const handleAddTask = useAddTaskHandler(layout.columnMap, focusCreatedTask);

  return (
    <CommandScopeProvider commands={boardActionCommands}>
      <BoardDndWrapper
        scrollContainerRef={scrollContainerRef}
        dragDrop={dragDrop}
        layout={layout}
        handleAddTask={handleAddTask}
      />
    </CommandScopeProvider>
  );
}

/** Props for `BoardSpatialZone`. */
interface BoardSpatialZoneProps {
  children: React.ReactNode;
}

/**
 * Wrap the board content in a `<FocusZone moniker={asSegment("ui:board")}>`
 * when the surrounding tree mounts the spatial-nav stack.
 *
 * `<FocusZone>` itself tolerates a missing `<FocusLayer>` ancestor by
 * falling back to a plain `<div>` (post-architecture-fix behaviour), but
 * that fallback still emits a `data-moniker="ui:board"` attribute. The
 * board's pre-spatial-nav unit tests assert there is no `ui:board`
 * marker on the DOM when they mount only `<EntityFocusProvider>` etc.,
 * so we shortcut the wrap entirely when the provider stack is absent —
 * existing tests stay green, and production (which always mounts the
 * providers) takes the zone-emitting branch unchanged.
 */
function BoardSpatialZone({ children }: BoardSpatialZoneProps) {
  const layerFq = useOptionalEnclosingLayerFq();
  const actions = useOptionalSpatialFocusActions();
  const parentFq = useOptionalFullyQualifiedMoniker();
  if (!layerFq || !actions || !parentFq) {
    return <>{children}</>;
  }
  // The board fills the viewport — drawing a focus rectangle around the
  // entire board body would be visually noisy, so `showFocusBar={false}`
  // suppresses the visible `<FocusIndicator>` here. The zone still
  // registers, still flips its `data-focused` attribute on focus claim,
  // and still owns drill-in/out + click-to-focus through the spatial-nav
  // graph — only the visible bar is muted. Sized container zones
  // (column, card, field row) keep `showFocusBar={true}` because they
  // are bounded boxes whose users need a visible "here is focus" hint;
  // viewport-sized chrome zones (board, perspective, navbar) suppress
  // it for the same reason. See
  // `kanban-app/ui/src/components/perspective-view.spatial.test.tsx`
  // for the matching contract on the perspective zone.
  return (
    <FocusZone
      moniker={asSegment("ui:board")}
      showFocusBar={false}
      className="flex flex-1 min-h-0"
    >
      {children}
    </FocusZone>
  );
}
