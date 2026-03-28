import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
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
import type { DropZoneDescriptor } from "@/lib/drop-zones";
import {
  CommandScopeProvider,
  type CommandDef,
} from "@/lib/command-scope";
import { ColumnView } from "@/components/column-view";
import { SortableColumn } from "@/components/sortable-column";
import { FocusScope } from "@/components/focus-scope";
import { useInspect } from "@/lib/inspect-context";
import { useEntityFocus } from "@/lib/entity-focus-context";
/** Default title for new tasks — the Rust side also uses this as fallback. */
function defaultTaskTitle(_columnName: string): string {
  return "New task";
}
import { moniker, fieldMoniker } from "@/lib/moniker";
import { useEntityCommands } from "@/lib/entity-commands";
import { useDragSession } from "@/lib/drag-session-context";
import type { BoardData, Entity } from "@/types/kanban";
import { getStr, getNum } from "@/types/kanban";

interface BoardViewProps {
  board: BoardData;
  tasks: Entity[];
  boardPath?: string;
}

type ColumnLayout = Map<string, string[]>;

interface TaskDragState {
  sourceTaskId: string;
  sourceColumn: string;
}

/**
 * Board view that renders columns and cards.
 *
 * Navigation is pull-based: each card and column header FocusScope declares
 * claimWhen predicates. The global KeybindingHandler broadcasts nav.up/down/
 * left/right/first/last, and each predicate evaluates whether it should claim
 * focus. No push-based cursor state is needed.
 */
export function BoardView({ board, tasks, boardPath }: BoardViewProps) {
  const boardPathRef = useRef(boardPath);
  boardPathRef.current = boardPath;
  const { startSession, cancelSession, completeSession } = useDragSession();
  const boardMoniker = moniker("board", "board");
  const boardCommands = useEntityCommands("board", "board");
  const inspectEntity = useInspect();
  const { focusedMoniker, broadcastNavCommand, setFocus } = useEntityFocus();
  const broadcastRef = useRef(broadcastNavCommand);
  broadcastRef.current = broadcastNavCommand;
  const focusedMonikerRef = useRef(focusedMoniker);
  focusedMonikerRef.current = focusedMoniker;

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

  // Pre-resolved task entity arrays per column — memoized so that React.memo
  // on ColumnView sees stable references and skips re-renders on cursor moves.
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

  // The first task in the todo (first) column — used for "Do This Next" placement
  const firstTodoTaskId = useMemo(() => {
    if (columns.length === 0) return null;
    const todoColId = columns[0].id;
    const todoTaskIds = baseLayout.get(todoColId);
    return todoTaskIds && todoTaskIds.length > 0 ? todoTaskIds[0] : null;
  }, [columns, baseLayout]);

  // --- Cross-column moniker tables for claimWhen ---

  /** Task monikers per column (in display order), indexed by column ID. */
  const columnTaskMonikers = useMemo(() => {
    const map = new Map<string, string[]>();
    for (const col of columns) {
      const taskIds = baseLayout.get(col.id) ?? [];
      map.set(
        col.id,
        taskIds.map((id) => moniker("task", id)),
      );
    }
    return map;
  }, [columns, baseLayout]);

  /** All task monikers on the board — used for nav.first/nav.last. */
  const allBoardTaskMonikers = useMemo(() => {
    const set = new Set<string>();
    for (const monikers of columnTaskMonikers.values()) {
      for (const m of monikers) set.add(m);
    }
    return set;
  }, [columnTaskMonikers]);

  /** All column header monikers (name field level). */
  const allBoardHeaderMonikers = useMemo(() => {
    const set = new Set<string>();
    for (const col of columns) {
      set.add(moniker("column", col.id));
      set.add(fieldMoniker("column", col.id, "name"));
    }
    return set;
  }, [columns]);

  /** Ref for handleAddTask so boardCommands can reference it without circular deps. */
  const handleAddTaskRef = useRef<(columnId: string) => void>(() => {});

  /** Ref to the horizontal scroll container — scrolls focused column into view. */
  const scrollContainerRef = useRef<HTMLDivElement>(null);

  // Scroll the focused column into view horizontally when focus changes
  useEffect(() => {
    const container = scrollContainerRef.current;
    if (!container || !focusedMoniker) return;
    // Find the DOM element with the focused moniker and scroll it into view
    const el = container.querySelector<HTMLElement>(
      `[data-moniker="${focusedMoniker}"]`,
    );
    if (el?.scrollIntoView)
      el.scrollIntoView({ inline: "nearest", block: "nearest" });
  }, [focusedMoniker]);

  // Focus the first card (or first column header) on mount so the board
  // starts with an active focus target for keyboard navigation.
  const initialFocusDone = useRef(false);
  useEffect(() => {
    if (initialFocusDone.current) return;
    initialFocusDone.current = true;
    // Find the first non-empty column's first task, or the first column header
    for (const col of columns) {
      const monikers = columnTaskMonikers.get(col.id) ?? [];
      if (monikers.length > 0) {
        setFocus(monikers[0]);
        return;
      }
    }
    // All columns empty — focus first column header
    if (columns.length > 0) {
      setFocus(moniker("column", columns[0].id));
    }
  }, [columns, columnTaskMonikers, setFocus]);

  /**
   * Board-level commands that don't need cursor state.
   * Inspect uses focusedMoniker directly; newTask finds the column from focusedMoniker.
   */
  const boardActionCommands = useMemo<CommandDef[]>(() => {
    /**
     * Determine which column the current focus is in.
     * Walks the focused moniker to find a column: either the focused element
     * IS a column, or it's a task whose position_column we can look up.
     */
    const findFocusedColumnId = (): string | null => {
      const fm = focusedMonikerRef.current;
      if (!fm) return columns[0]?.id ?? null;
      // Check if it's a column header
      if (fm.startsWith("column:")) return fm.slice("column:".length);
      // Check if it's a task — look up its column
      if (fm.startsWith("task:")) {
        const taskId = fm.slice("task:".length);
        const entity = taskMap.get(taskId);
        if (entity) return getStr(entity, "position_column") || (columns[0]?.id ?? null);
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
          if (fm) inspectEntity(fm);
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
          // Move to the first column's header
          if (columns.length > 0) {
            broadcastRef.current("nav.first");
          }
        },
      },
      {
        id: "board.lastColumn",
        name: "Last Column",
        keys: { vim: "$", cua: "Mod+End" },
        execute: () => {
          if (columns.length > 0) {
            broadcastRef.current("nav.last");
          }
        },
      },
    ];
  }, [columns, taskMap, inspectEntity]);

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
          ...(boardPathRef.current ? { boardPath: boardPathRef.current } : {}),
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
    async (descriptor: DropZoneDescriptor, taskId: string, entity: Entity) => {
      try {
        const args: Record<string, unknown> = {
          id: taskId,
          column: descriptor.columnId,
          swimlane: getStr(entity, "position_swimlane") || null,
        };
        if (descriptor.beforeId) args.before_id = descriptor.beforeId;
        if (descriptor.afterId) args.after_id = descriptor.afterId;
        const boardPath = descriptor.boardPath || boardPathRef.current;
        await invoke("dispatch_command", {
          cmd: "task.move",
          args,
          scopeChain: [`task:${taskId}`],
          ...(boardPath ? { boardPath } : {}),
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

  const handleAddTask = useCallback(
    async (columnId: string) => {
      const col = columnMap.get(columnId);
      const title = defaultTaskTitle(col ? getStr(col, "name") : "");
      try {
        await invoke("dispatch_command", {
          cmd: "task.add",
          args: { title, column: columnId },
          ...(boardPathRef.current ? { boardPath: boardPathRef.current } : {}),
        });
      } catch (e) {
        console.error("Failed to add task:", e);
      }
    },
    [columnMap],
  );
  handleAddTaskRef.current = handleAddTask;

  return (
    <FocusScope
      moniker={boardMoniker}
      commands={boardCommands}
      className="flex flex-col flex-1 min-h-0 relative"
    >
      <CommandScopeProvider commands={boardActionCommands}>
        {/* @dnd-kit context for column reordering only */}
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
                const colTasks = columnTasks.get(col.id) ?? [];

                // Compute adjacent column monikers for cross-column nav
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
                      tasks={colTasks}
                      onAddTask={i === 0 ? handleAddTask : undefined}
                      onTaskDragStart={handleTaskDragStart}
                      onTaskDragEnd={handleTaskDragEnd}
                      onDrop={handleZoneDrop}
                      dragTaskId={taskDrag?.sourceTaskId ?? null}
                      boardPath={boardPath}
                      firstTodoTaskId={firstTodoTaskId}
                      leftColumnTaskMonikers={
                        prevColId
                          ? columnTaskMonikers.get(prevColId) ?? []
                          : []
                      }
                      leftColumnHeaderMoniker={
                        prevColId ? fieldMoniker("column", prevColId, "name") : null
                      }
                      rightColumnTaskMonikers={
                        nextColId
                          ? columnTaskMonikers.get(nextColId) ?? []
                          : []
                      }
                      rightColumnHeaderMoniker={
                        nextColId ? fieldMoniker("column", nextColId, "name") : null
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
          <DragOverlay dropAnimation={null}>
            {activeColumn ? (
              <div className="rounded-md bg-card border border-border px-4 py-2 text-sm font-medium text-muted-foreground uppercase tracking-wide shadow-lg">
                {getStr(activeColumn, "name")}
              </div>
            ) : null}
          </DragOverlay>
        </DndContext>
      </CommandScopeProvider>
    </FocusScope>
  );
}
