import {
  useCallback,
  useContext,
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
  CommandScopeContext,
  type CommandDef,
} from "@/lib/command-scope";
import { ColumnView } from "@/components/column-view";
import { SortableColumn } from "@/components/sortable-column";
import { FocusScope } from "@/components/focus-scope";
import { useEntityFocus } from "@/lib/entity-focus-context";
import { useInspect } from "@/lib/inspect-context";
import { useBoardNav } from "@/hooks/use-board-nav";
import { BoardNavProvider } from "@/lib/board-nav-context";
/** Default title for new tasks — the Rust side also uses this as fallback. */
function defaultTaskTitle(_columnName: string): string {
  return "New task";
}
import { moniker } from "@/lib/moniker";
import { useEntityCommands } from "@/lib/entity-commands";
import { useDragSession } from "@/lib/drag-session-context";
import type { BoardData, Entity } from "@/types/kanban";
import { getStr, getNum } from "@/types/kanban";

/**
 * Renderless component that bridges the board cursor to entity focus.
 *
 * Must be rendered inside a CommandScopeProvider so it picks up the
 * correct scope (including board nav commands). Uses two separate effects:
 * one for scope registration (fires on scope changes) and one for focus
 * (fires only when the moniker changes, i.e. cursor movement).
 */
function BoardFocusBridge({ moniker: mk }: { moniker: string }) {
  const scope = useContext(CommandScopeContext);
  const { setFocus, registerScope, unregisterScope } = useEntityFocus();

  // Register scope — fires on any change to keep registry current
  useEffect(() => {
    if (scope) registerScope(mk, scope);
    return () => unregisterScope(mk);
  }, [mk, scope, registerScope, unregisterScope]);

  // Set focus — fires ONLY when the moniker changes (cursor movement)
  useEffect(() => {
    setFocus(mk);
  }, [mk, setFocus]);

  return null;
}

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

export function BoardView({ board, tasks, boardPath }: BoardViewProps) {
  const boardPathRef = useRef(boardPath);
  boardPathRef.current = boardPath;
  const { startSession, cancelSession, completeSession } = useDragSession();
  const boardMoniker = moniker("board", "board");
  const boardCommands = useEntityCommands("board", "board");
  const inspectEntity = useInspect();

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

  // --- Board keyboard navigation ---
  const cardCounts = useMemo(
    () => columns.map((col) => (baseLayout.get(col.id) ?? []).length),
    [columns, baseLayout],
  );

  const boardNav = useBoardNav({
    columnCount: columns.length,
    cardCounts,
  });
  const boardNavRef = useRef(boardNav);
  boardNavRef.current = boardNav;

  /** Resolve the task entity at the current board cursor position. */
  const currentBoardEntity = useMemo(() => {
    const { col, card } = boardNav.cursor;
    if (col < 0 || col >= columns.length || card < 0) return null;
    const colId = columns[col].id;
    const taskIds = baseLayout.get(colId) ?? [];
    if (card >= taskIds.length) return null;
    return taskMap.get(taskIds[card]) ?? null;
  }, [boardNav.cursor, columns, baseLayout, taskMap]);

  // Moniker for the focused entity, or the board moniker as fallback.
  // The board moniker fallback ensures board nav commands are always reachable
  // even when the cursor is on a column header (card=-1, no entity).
  const focusBridgeMoniker = currentBoardEntity
    ? moniker("task", currentBoardEntity.id)
    : boardMoniker;

  /** Ref for handleAddTask so boardNavCommands can reference it without circular deps. */
  const handleAddTaskRef = useRef<(columnId: string) => void>(() => {});

  // Stable colId → cursor index map
  const colIdToIndex = useMemo(() => {
    const map = new Map<string, number>();
    for (let i = 0; i < columns.length; i++) map.set(columns[i].id, i);
    return map;
  }, [columns]);

  /** Ref to the horizontal scroll container — scrolls focused column into view. */
  const scrollContainerRef = useRef<HTMLDivElement>(null);

  // Scroll the focused column into view horizontally when cursor.col changes
  useEffect(() => {
    const container = scrollContainerRef.current;
    if (
      !container ||
      boardNav.cursor.col < 0 ||
      boardNav.cursor.col >= columns.length
    )
      return;
    const focusedColId = columns[boardNav.cursor.col].id;
    const el = container.querySelector<HTMLElement>(
      `[data-moniker="column:${focusedColId}"]`,
    );
    if (el?.scrollIntoView)
      el.scrollIntoView({ inline: "nearest", block: "nearest" });
  }, [boardNav.cursor.col, columns]);

  const boardNavCommands = useMemo<CommandDef[]>(
    () => [
      {
        id: "board.moveLeft",
        name: "Move Left",
        keys: { vim: "h", cua: "ArrowLeft" },
        execute: () => boardNavRef.current.moveLeft(),
      },
      {
        id: "board.moveRight",
        name: "Move Right",
        keys: { vim: "l", cua: "ArrowRight" },
        execute: () => boardNavRef.current.moveRight(),
      },
      {
        id: "board.moveUp",
        name: "Move Up",
        keys: { vim: "k", cua: "ArrowUp" },
        execute: () => boardNavRef.current.moveUp(),
      },
      {
        id: "board.moveDown",
        name: "Move Down",
        keys: { vim: "j", cua: "ArrowDown" },
        execute: () => boardNavRef.current.moveDown(),
      },
      {
        id: "board.firstCard",
        name: "First Card",
        keys: { cua: "Home" },
        // vim: "g g" handled via SEQUENCE_TABLES in keybindings.ts
        execute: () => boardNavRef.current.moveToFirstCard(),
      },
      {
        id: "board.lastCard",
        name: "Last Card",
        // normalizeKeyEvent produces "Shift+G" for uppercase G
        keys: { vim: "Shift+G", cua: "End" },
        execute: () => boardNavRef.current.moveToLastCard(),
      },
      {
        id: "board.firstColumn",
        name: "First Column",
        keys: { vim: "0", cua: "Mod+Home" },
        execute: () => boardNavRef.current.moveToFirstColumn(),
      },
      {
        id: "board.lastColumn",
        name: "Last Column",
        // normalizeKeyEvent produces "Shift+4" → "$"
        keys: { vim: "$", cua: "Mod+End" },
        execute: () => boardNavRef.current.moveToLastColumn(),
      },
      {
        id: "board.inspect",
        name: "Inspect",
        keys: { vim: "Enter", cua: "Enter" },
        execute: () => {
          const nav = boardNavRef.current;
          const colId = columns[nav.cursor.col]?.id;
          if (!colId) return;
          if (nav.cursor.card === -1) {
            // On column header — inspect the column
            inspectEntity(moniker("column", colId));
          } else {
            // On a card — inspect the task
            const taskIds = baseLayout.get(colId) ?? [];
            const taskId = taskIds[nav.cursor.card];
            if (taskId) inspectEntity(moniker("task", taskId));
          }
        },
      },
      {
        id: "board.newTask",
        name: "New Task",
        keys: { vim: "o", cua: "Mod+Enter" },
        execute: () => {
          const nav = boardNavRef.current;
          const colId = columns[nav.cursor.col]?.id;
          if (colId) handleAddTaskRef.current(colId);
        },
      },
    ],
    [columns, baseLayout, inspectEntity],
  );

  // --- BoardNavProvider callbacks (stable via refs in the provider) ---

  const handleBoardCardClick = useCallback(
    (columnId: string, cardIndex: number) => {
      const colIdx = colIdToIndex.get(columnId) ?? 0;
      boardNavRef.current.setCursor(colIdx, cardIndex);
    },
    [colIdToIndex],
  );

  const handleBoardHeaderClick = useCallback(
    (columnId: string) => {
      const colIdx = colIdToIndex.get(columnId) ?? 0;
      boardNavRef.current.setCursor(colIdx, -1);
    },
    [colIdToIndex],
  );

  const handleBoardCardDoubleClick = useCallback(
    (columnId: string, cardIndex: number) => {
      const colIdx = colIdToIndex.get(columnId) ?? 0;
      boardNavRef.current.setCursor(colIdx, cardIndex);
      const taskIds = baseLayout.get(columnId) ?? [];
      const taskId = taskIds[cardIndex];
      if (taskId) inspectEntity(moniker("task", taskId));
    },
    [colIdToIndex, baseLayout, inspectEntity],
  );

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
      <CommandScopeProvider commands={boardNavCommands}>
        <BoardFocusBridge moniker={focusBridgeMoniker} />
        <BoardNavProvider
          onCardClick={handleBoardCardClick}
          onHeaderClick={handleBoardHeaderClick}
          onCardDoubleClick={handleBoardCardDoubleClick}
        >
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
              onClick={() => {
                boardNav.setCursor(boardNav.cursor.col, -1);
              }}
            >
              <SortableContext
                items={currentColumnOrder}
                strategy={horizontalListSortingStrategy}
              >
                {currentColumnOrder.map((colId, i) => {
                  const col = columnMap.get(colId);
                  if (!col) return null;
                  const colTasks = columnTasks.get(col.id) ?? [];
                  // Map visual index to cursor col index (columns sorted by order)
                  const cursorColIndex = colIdToIndex.get(colId) ?? -1;
                  const isFocusedCol = boardNav.cursor.col === cursorColIndex;
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
                        focusedCardIndex={
                          isFocusedCol ? boardNav.cursor.card : null
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
        </BoardNavProvider>
      </CommandScopeProvider>
    </FocusScope>
  );
}

