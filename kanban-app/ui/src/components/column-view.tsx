import { memo, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useDispatchCommand } from "@/lib/command-scope";
import { Plus } from "lucide-react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { invoke } from "@tauri-apps/api/core";
import { ulid } from "ulid";
import { DropZone } from "@/components/drop-zone";
import { computeDropZones, type DropZoneDescriptor } from "@/lib/drop-zones";
import { Field } from "@/components/fields/field";
import { DraggableTaskCard } from "@/components/sortable-task-card";
import { FocusScope } from "@/components/focus-scope";
import { useFocusLayerKey } from "@/components/focus-layer";
import { Badge } from "@/components/ui/badge";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { useEntityCommands } from "@/lib/entity-commands";
import { useSchema } from "@/lib/schema-context";
import { useEntityFocus } from "@/lib/entity-focus-context";
import type { CommandDef } from "@/lib/command-scope";
import type { Entity } from "@/types/kanban";
import { getStr } from "@/types/kanban";

interface ColumnViewProps {
  column: Entity;
  /** Tasks for this column, pre-sorted by the backend. */
  tasks: Entity[];
  onAddTask?: (columnId: string) => void;
  /** Called when a task drag starts in this column. */
  onTaskDragStart?: (entity: Entity) => void;
  /** Called when a task drag ends (from this column's card). */
  onTaskDragEnd?: (entity: Entity, dropEffect: string) => void;
  /** Called when a task is dropped on a zone in this column. */
  onDrop?: (descriptor: DropZoneDescriptor, taskData: string) => void;
  /** ID of the task currently being dragged (for no-op zone suppression). */
  dragTaskId?: string | null;
  /** ID of the first task in the todo column — used for "Do This Next" command. */
  firstTodoTaskId?: string | null;
  /** Ref callback for the column container — used for cross-window hit-testing. */
  containerRef?: (el: HTMLDivElement | null) => void;
}

/** Distance from container edge (px) that triggers auto-scroll during drag. */
const SCROLL_ZONE = 40;
/** Pixels per animation frame to scroll when in the edge zone. */
const SCROLL_SPEED = 6;
/** Estimated height (px) of a DropZone + Card pair for the virtualizer. */
const ESTIMATED_ITEM_HEIGHT = 80;
/** Estimated height (px) of the trailing drop zone. */
const TRAILING_ZONE_HEIGHT = 6;
/** Minimum task count to activate virtualization. Below this, all items render directly. */
const VIRTUALIZE_THRESHOLD = 25;

/**
 * Manage auto-scroll during drag near the top/bottom edges of a scroll container.
 *
 * Returns start/stop callbacks and a ref to the scroll container. Cleans up
 * the rAF loop on unmount.
 */
function useAutoScroll() {
  const containerRef = useRef<HTMLDivElement>(null);
  const scrollRafRef = useRef<number | null>(null);
  const scrollDirRef = useRef(0);

  const stopAutoScroll = useCallback(() => {
    scrollDirRef.current = 0;
    if (scrollRafRef.current !== null) {
      cancelAnimationFrame(scrollRafRef.current);
      scrollRafRef.current = null;
    }
  }, []);

  const startAutoScroll = useCallback((dir: -1 | 1) => {
    scrollDirRef.current = dir;
    if (scrollRafRef.current !== null) return;
    const tick = () => {
      if (scrollDirRef.current === 0 || !containerRef.current) {
        scrollRafRef.current = null;
        return;
      }
      containerRef.current.scrollBy({ top: scrollDirRef.current * SCROLL_SPEED });
      scrollRafRef.current = requestAnimationFrame(tick);
    };
    scrollRafRef.current = requestAnimationFrame(tick);
  }, []);

  useEffect(() => () => stopAutoScroll(), [stopAutoScroll]);

  return { containerRef, startAutoScroll, stopAutoScroll };
}

/**
 * Build per-task "Do This Next" extra-command maps and drop-zone descriptors.
 *
 * Memoizes the command map so ColumnView avoids recalculating every render.
 */
function useColumnCommands(
  tasks: Entity[],
  columnId: string,
  firstTodoTaskId: string | null | undefined,
  dispatchTaskMove: ReturnType<typeof useDispatchCommand>,
) {
  const zones = useMemo(
    () => computeDropZones(tasks.map((t) => t.id), columnId),
    [tasks, columnId],
  );

  const buildDoThisNextCommand = useCallback(
    (taskId: string): CommandDef | null => {
      if (taskId === firstTodoTaskId) return null;
      return {
        id: "task.doThisNext",
        name: "Do This Next",
        contextMenu: true,
        execute: () => {
          const args: Record<string, unknown> = { id: taskId, column: "todo" };
          if (firstTodoTaskId) args.before_id = firstTodoTaskId;
          dispatchTaskMove({ args }).catch(console.error);
        },
      };
    },
    [firstTodoTaskId, dispatchTaskMove],
  );

  const taskExtraCommands = useMemo(() => {
    const map = new Map<string, CommandDef[]>();
    for (const task of tasks) {
      const cmd = buildDoThisNextCommand(task.id);
      if (cmd) map.set(task.id, [cmd]);
    }
    return map;
  }, [tasks, buildDoThisNextCommand]);

  return { zones, taskExtraCommands };
}

/**
 * Build the dragOver handler that auto-scrolls near container edges.
 *
 * Ignores file drags (handled by FileDropProvider) and calls preventDefault
 * so child DropZones accept the drop.
 */
function useDragOverHandler(
  containerRef: React.RefObject<HTMLDivElement | null>,
  startAutoScroll: (dir: -1 | 1) => void,
  stopAutoScroll: () => void,
) {
  return useCallback(
    (e: React.DragEvent) => {
      if (e.dataTransfer.types.includes("Files")) return;
      e.preventDefault();
      e.dataTransfer.dropEffect = "move";
      if (!containerRef.current) return;
      const rect = containerRef.current.getBoundingClientRect();
      if (e.clientY < rect.top + SCROLL_ZONE) startAutoScroll(-1);
      else if (e.clientY > rect.bottom - SCROLL_ZONE) startAutoScroll(1);
      else stopAutoScroll();
    },
    [containerRef, startAutoScroll, stopAutoScroll],
  );
}

/**
 * Renders a single column in the board view with drag-drop, focus highlight,
 * and keyboard navigation support.
 *
 * Cardinal direction navigation is handled by the Rust spatial navigation
 * layer which computes focus targets from DOM rects at runtime.
 */
/**
 * Compose all column-level hooks needed by ColumnView into a single state object.
 *
 * Centralizes hook orchestration so the component body stays focused on
 * rendering. Returns everything needed for the header and card list.
 */
function useColumnViewState(
  column: Entity,
  tasks: Entity[],
  firstTodoTaskId: string | null | undefined,
  containerRefProp: ((el: HTMLDivElement | null) => void) | undefined,
  onDropProp: ((d: DropZoneDescriptor, t: string) => void) | undefined,
) {
  const dispatchTaskMove = useDispatchCommand("task.move");
  const { getFieldDef } = useSchema();
  const nameFieldDef = getFieldDef("column", "name");
  const [editingName, setEditingName] = useState(false);
  const { setFocus } = useEntityFocus();

  const { containerRef, startAutoScroll, stopAutoScroll } = useAutoScroll();
  const commands = useEntityCommands("column", column.id, column);
  const { zones, taskExtraCommands } = useColumnCommands(
    tasks, column.id, firstTodoTaskId, dispatchTaskMove,
  );

  const setContainerRef = useCallback(
    (el: HTMLDivElement | null) => {
      (containerRef as React.MutableRefObject<HTMLDivElement | null>).current = el;
      containerRefProp?.(el);
    },
    [containerRef, containerRefProp],
  );

  const handleContainerDragOver = useDragOverHandler(containerRef, startAutoScroll, stopAutoScroll);

  const handleZoneDrop = useCallback(
    (descriptor: DropZoneDescriptor, taskData: string) => { onDropProp?.(descriptor, taskData); },
    [onDropProp],
  );

  return {
    nameFieldDef, editingName, setEditingName, setFocus, commands,
    zones, taskExtraCommands, setContainerRef, handleContainerDragOver, handleZoneDrop,
  };
}

/**
 * Renders a single column in the board view with drag-drop, focus highlight,
 * and keyboard navigation support.
 *
 * Cardinal direction navigation is handled by the Rust spatial navigation
 * layer which computes focus targets from DOM rects at runtime.
 */
export const ColumnView = memo(function ColumnView({
  column, tasks, onAddTask, onTaskDragStart, onTaskDragEnd,
  onDrop: onDropProp, dragTaskId, firstTodoTaskId, containerRef: containerRefProp,
}: ColumnViewProps) {
  const {
    nameFieldDef, editingName, setEditingName, setFocus, commands,
    zones, taskExtraCommands, setContainerRef, handleContainerDragOver, handleZoneDrop,
  } = useColumnViewState(column, tasks, firstTodoTaskId, containerRefProp, onDropProp);

  return (
    <FocusScope
      moniker={column.moniker}
      commands={commands}
      className="flex flex-col min-h-0 min-w-[24em] max-w-[48em] shrink-0 flex-1"
    >
      <ColumnHeader
        column={column}
        columnMoniker={column.moniker}
        columnNameMoniker={`${column.moniker}.name`}
        nameFieldDef={nameFieldDef}
        editingName={editingName}
        setEditingName={setEditingName}
        setFocus={setFocus}
        taskCount={tasks.length}
        onAddTask={onAddTask}
      />
      <VirtualizedCardList
        tasks={tasks}
        zones={zones}
        dragTaskId={dragTaskId}
        onZoneDrop={handleZoneDrop}
        onTaskDragStart={onTaskDragStart}
        onTaskDragEnd={onTaskDragEnd}
        taskExtraCommands={taskExtraCommands}
        containerRef={setContainerRef}
        onDragOver={handleContainerDragOver}
      />
    </FocusScope>
  );
});

/** Props for the column header sub-component. */
interface ColumnHeaderProps {
  column: Entity;
  columnMoniker: string;
  columnNameMoniker: string;
  nameFieldDef: ReturnType<ReturnType<typeof useSchema>["getFieldDef"]>;
  editingName: boolean;
  setEditingName: (v: boolean) => void;
  setFocus: (moniker: string) => void;
  taskCount: number;
  onAddTask?: (columnId: string) => void;
}

/**
 * Column header with inline-editable name, badge count, and add-task button.
 *
 * Extracted from ColumnView so the main component stays under the line limit.
 */
function ColumnHeader({
  column, columnMoniker, columnNameMoniker, nameFieldDef,
  editingName, setEditingName, setFocus, taskCount, onAddTask,
}: ColumnHeaderProps) {
  return (
    <div className="flex flex-col min-h-0 min-w-0 flex-1">
      <div
        className="column-header-focus px-3 py-2 flex items-center gap-2 rounded"
        onClickCapture={() => setFocus(columnNameMoniker)}
      >
        <FocusScope moniker={columnNameMoniker} commands={[]} className="inline">
          {nameFieldDef ? (
            <Field
              fieldDef={nameFieldDef} entityType="column" entityId={column.id}
              mode="compact" editing={editingName}
              onEdit={() => setEditingName(true)}
              onDone={() => setEditingName(false)}
              onCancel={() => setEditingName(false)}
            />
          ) : (
            <span className="text-sm font-semibold text-foreground">
              {getStr(column, "name")}
            </span>
          )}
        </FocusScope>
        <Badge variant="secondary">{taskCount}</Badge>
        <div className="flex-1" />
        {onAddTask && (
          <AddTaskButton column={column} columnMoniker={columnMoniker}
            setFocus={setFocus} onAddTask={onAddTask} />
        )}
      </div>
    </div>
  );
}

/** Props for the add-task tooltip button. */
interface AddTaskButtonProps {
  column: Entity;
  columnMoniker: string;
  setFocus: (moniker: string) => void;
  onAddTask: (columnId: string) => void;
}

/**
 * Tooltip-wrapped button that creates a new task in the parent column.
 *
 * Sets focus to the column moniker before dispatching so the Rust scope
 * chain resolves the correct column entity.
 */
function AddTaskButton({ column, columnMoniker, setFocus, onAddTask }: AddTaskButtonProps) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <button
          type="button"
          aria-label={`Add task to ${getStr(column, "name")}`}
          className="p-0.5 rounded text-muted-foreground/50 hover:text-muted-foreground hover:bg-muted transition-colors"
          onClick={() => {
            setFocus(columnMoniker);
            onAddTask(column.id);
          }}
        >
          <Plus className="h-4 w-4" />
        </button>
      </TooltipTrigger>
      <TooltipContent>
        {`Add task to ${getStr(column, "name")}`}
      </TooltipContent>
    </Tooltip>
  );
}

// ---------------------------------------------------------------------------
// VirtualizedCardList — renders only visible card+zone pairs
// ---------------------------------------------------------------------------

interface VirtualizedCardListProps {
  tasks: Entity[];
  zones: DropZoneDescriptor[];
  dragTaskId?: string | null;
  onZoneDrop: (descriptor: DropZoneDescriptor, taskData: string) => void;
  onTaskDragStart?: (entity: Entity) => void;
  onTaskDragEnd?: (entity: Entity, dropEffect: string) => void;
  taskExtraCommands: Map<string, CommandDef[]>;
  containerRef: (el: HTMLDivElement | null) => void;
  onDragOver: (e: React.DragEvent) => void;
}

/** Shared CSS class for the card list scroll container. */
const CARD_LIST_CONTAINER_CLASS =
  "flex-1 overflow-y-auto [scrollbar-gutter:stable] px-2 pt-1 pb-2 m-1 rounded-lg border-2 border-transparent";

/** Shared props for card items rendered by both small and virtual lists. */
interface CardItemProps {
  zones: DropZoneDescriptor[];
  dragTaskId?: string | null;
  onZoneDrop: (descriptor: DropZoneDescriptor, taskData: string) => void;
  onTaskDragStart?: (entity: Entity) => void;
  onTaskDragEnd?: (entity: Entity, dropEffect: string) => void;
  taskExtraCommands: Map<string, CommandDef[]>;
}

/**
 * A single card + preceding drop zone pair used in the non-virtualized list.
 *
 * Extracted so the small-list map body stays one line per item.
 */
function CardWithZone({
  entity,
  index,
  zones,
  dragTaskId,
  onZoneDrop,
  onTaskDragStart,
  onTaskDragEnd,
  taskExtraCommands,
}: CardItemProps & { entity: Entity; index: number }) {
  return (
    <div>
      <DropZone descriptor={zones[index]} dragTaskId={dragTaskId} onDrop={onZoneDrop} />
      <div className="rounded">
        <DraggableTaskCard
          entity={entity}
          onDragStart={onTaskDragStart}
          onDragEnd={onTaskDragEnd}
          extraCommands={taskExtraCommands.get(entity.id)}
        />
      </div>
    </div>
  );
}

/**
 * Renders the card + drop-zone list inside a column.
 *
 * When the column is empty, renders a single empty-column drop zone.
 * For small lists (< VIRTUALIZE_THRESHOLD), renders all items directly.
 * For large lists, uses @tanstack/react-virtual to mount only visible
 * items plus overscan.
 */
const VirtualizedCardList = memo(function VirtualizedCardList({
  tasks, zones, dragTaskId, onZoneDrop, onTaskDragStart, onTaskDragEnd,
  taskExtraCommands, containerRef: containerRefProp, onDragOver,
}: VirtualizedCardListProps) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const setRef = useCallback(
    (el: HTMLDivElement | null) => {
      (scrollRef as React.MutableRefObject<HTMLDivElement | null>).current = el;
      containerRefProp(el);
    },
    [containerRefProp],
  );
  const cardProps: CardItemProps = {
    zones, dragTaskId, onZoneDrop, onTaskDragStart, onTaskDragEnd, taskExtraCommands,
  };

  if (tasks.length === 0) {
    return (
      <div ref={setRef} className={CARD_LIST_CONTAINER_CLASS} onDragOver={onDragOver}>
        <DropZone descriptor={zones[0]} dragTaskId={dragTaskId} onDrop={onZoneDrop} variant="empty-column" />
      </div>
    );
  }
  if (tasks.length < VIRTUALIZE_THRESHOLD) {
    return (
      <div ref={setRef} className={CARD_LIST_CONTAINER_CLASS} onDragOver={onDragOver}>
        {tasks.map((entity, i) => (
          <CardWithZone key={entity.id} entity={entity} index={i} {...cardProps} />
        ))}
        <DropZone descriptor={zones[zones.length - 1]} dragTaskId={dragTaskId} onDrop={onZoneDrop} />
      </div>
    );
  }
  return (
    <VirtualColumn tasks={tasks} scrollRef={scrollRef} setRef={setRef}
      onDragOver={onDragOver} {...cardProps} />
  );
});

/** Absolute-position style for a virtualized row at the given Y offset. */
function virtualRowStyle(startPx: number): React.CSSProperties {
  return { position: "absolute", top: 0, left: 0, width: "100%", transform: `translateY(${startPx}px)` };
}

/**
 * Register estimated placeholder rects for off-screen items in a virtualized
 * column so spatial navigation can target them before they mount.
 *
 * Generates one ULID spatial key per task moniker on first render, registers
 * all placeholders via `spatial_register_batch`, and unregisters them on
 * unmount via `spatial_unregister_batch`. Real FocusScope mounts overwrite
 * estimates with measured rects (register is an upsert).
 */
function usePlaceholderRegistration(
  tasks: Entity[],
  scrollRef: React.RefObject<HTMLDivElement | null>,
  layerKey: string | null,
) {
  // Stable placeholder keys: one per task, regenerated when task list identity changes.
  const keysRef = useRef<Map<string, string>>(new Map());

  const placeholderKeys = useMemo(() => {
    const prev = keysRef.current;
    const next = new Map<string, string>();
    for (const t of tasks) {
      next.set(t.id, prev.get(t.id) ?? ulid());
    }
    keysRef.current = next;
    return next;
  }, [tasks]);

  useEffect(() => {
    if (!layerKey) return;
    const el = scrollRef.current;
    if (!el) return;
    const containerRect = el.getBoundingClientRect();
    const entries = tasks.map((task, i) => ({
      key: placeholderKeys.get(task.id)!,
      moniker: task.moniker,
      x: containerRect.x,
      y: containerRect.y + i * ESTIMATED_ITEM_HEIGHT,
      w: containerRect.width,
      h: ESTIMATED_ITEM_HEIGHT,
      layer_key: layerKey,
      parent_scope: null,
      overrides: null,
    }));
    if (entries.length > 0) {
      invoke("spatial_register_batch", { entries }).catch(() => {});
    }
    const keys = entries.map((e) => e.key);
    return () => {
      if (keys.length > 0) {
        invoke("spatial_unregister_batch", { keys }).catch(() => {});
      }
    };
  }, [tasks, placeholderKeys, layerKey, scrollRef]);
}

/** Props for VirtualColumn — the large-list virtualization path. */
interface VirtualColumnProps extends CardItemProps {
  tasks: Entity[];
  scrollRef: React.RefObject<HTMLDivElement | null>;
  setRef: (el: HTMLDivElement | null) => void;
  onDragOver: (e: React.DragEvent) => void;
}

/**
 * Inner component that renders virtualized card + zone pairs.
 *
 * Calls useVirtualizer unconditionally (React hook rules) — the parent
 * only mounts this when the task count exceeds `VIRTUALIZE_THRESHOLD`.
 * Registers estimated placeholder rects for all items so spatial
 * navigation can target off-screen cards.
 */
function VirtualColumn({
  tasks, zones, dragTaskId, onZoneDrop, onTaskDragStart, onTaskDragEnd,
  taskExtraCommands, scrollRef, setRef, onDragOver,
}: VirtualColumnProps) {
  const layerKey = useFocusLayerKey();
  usePlaceholderRegistration(tasks, scrollRef, layerKey);

  const virtualizer = useVirtualizer({
    count: tasks.length + 1,
    getScrollElement: () => scrollRef.current,
    estimateSize: (i) => (i < tasks.length ? ESTIMATED_ITEM_HEIGHT : TRAILING_ZONE_HEIGHT),
    overscan: 5,
  });

  return (
    <div ref={setRef} className={CARD_LIST_CONTAINER_CLASS} onDragOver={onDragOver}>
      <div style={{ height: virtualizer.getTotalSize(), width: "100%", position: "relative" }}>
        {virtualizer.getVirtualItems().map((vRow) => (
          <VirtualRowItem
            key={vRow.index === tasks.length ? "trailing-zone" : tasks[vRow.index].id}
            vRow={vRow}
            tasks={tasks}
            zones={zones}
            dragTaskId={dragTaskId}
            onZoneDrop={onZoneDrop}
            onTaskDragStart={onTaskDragStart}
            onTaskDragEnd={onTaskDragEnd}
            taskExtraCommands={taskExtraCommands}
            measureElement={virtualizer.measureElement}
          />
        ))}
      </div>
    </div>
  );
}

/** Props for a single virtualized row item (card + zone or trailing zone). */
interface VirtualRowItemProps extends CardItemProps {
  vRow: { index: number; start: number };
  tasks: Entity[];
  measureElement: (el: HTMLElement | null) => void;
}

/**
 * Render one virtualized row — either a card + zone pair or the trailing zone.
 *
 * Extracted so the virtualizer map callback stays a single JSX expression.
 */
function VirtualRowItem({
  vRow, tasks, zones, dragTaskId, onZoneDrop, onTaskDragStart, onTaskDragEnd,
  taskExtraCommands, measureElement,
}: VirtualRowItemProps) {
  if (vRow.index === tasks.length) {
    return (
      <div data-index={vRow.index} ref={measureElement} style={virtualRowStyle(vRow.start)}>
        <DropZone descriptor={zones[zones.length - 1]} dragTaskId={dragTaskId} onDrop={onZoneDrop} />
      </div>
    );
  }
  const entity = tasks[vRow.index];
  return (
    <div data-index={vRow.index} ref={measureElement} style={virtualRowStyle(vRow.start)}>
      <DropZone descriptor={zones[vRow.index]} dragTaskId={dragTaskId} onDrop={onZoneDrop} />
      <div className="rounded">
        <DraggableTaskCard
          entity={entity}
          onDragStart={onTaskDragStart}
          onDragEnd={onTaskDragEnd}
          extraCommands={taskExtraCommands.get(entity.id)}
        />
      </div>
    </div>
  );
}
