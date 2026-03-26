import { memo, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Plus } from "lucide-react";
import { DropZone } from "@/components/drop-zone";
import { computeDropZones, type DropZoneDescriptor } from "@/lib/drop-zones";
import { Field } from "@/components/fields/field";
import { DraggableTaskCard } from "@/components/sortable-task-card";
import { FocusScope } from "@/components/focus-scope";
import { FocusHighlight } from "@/components/ui/focus-highlight";
import { Badge } from "@/components/ui/badge";
import { moniker } from "@/lib/moniker";
import { useEntityCommands } from "@/lib/entity-commands";
import { useSchema } from "@/lib/schema-context";
import { useBoardNavActions } from "@/lib/board-nav-context";
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
  /** Board path — passed explicitly from BoardView, not pulled from context. */
  boardPath?: string;
  /** Ref callback for the column container — used for cross-window hit-testing. */
  containerRef?: (el: HTMLDivElement | null) => void;
  /** ID of the first task in the todo column — used for "Do This Next" command. */
  firstTodoTaskId?: string | null;
  /** Which card has keyboard focus: -1 = column header, 0..n = card index, null = not focused. */
  focusedCardIndex?: number | null;
}

/** Distance from container edge (px) that triggers auto-scroll during drag. */
const SCROLL_ZONE = 40;
/** Pixels per animation frame to scroll when in the edge zone. */
const SCROLL_SPEED = 6;

/**
 * Renders a single column in the board view with drag-drop, focus highlight,
 * and keyboard navigation support.
 *
 * Click handling uses BoardNavContext (stable, ref-backed) instead of callback
 * props so that React.memo can prevent re-renders when only other columns' focus changes.
 */
export const ColumnView = memo(function ColumnView({
  column,
  tasks,
  onAddTask,
  onTaskDragStart,
  onTaskDragEnd,
  onDrop: onDropProp,
  dragTaskId,
  boardPath,
  containerRef: containerRefProp,
  firstTodoTaskId,
  focusedCardIndex,
}: ColumnViewProps) {
  const columnMoniker = moniker("column", column.id);
  const { getFieldDef } = useSchema();
  const nameFieldDef = getFieldDef("column", "name");
  const [editingName, setEditingName] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);
  /** rAF handle for edge-scroll loop during drag. */
  const scrollRafRef = useRef<number | null>(null);
  /** Current scroll direction: -1 (up), 0 (none), 1 (down). */
  const scrollDirRef = useRef(0);

  // Board nav actions from context — stable, never triggers re-render
  const navActions = useBoardNavActions();

  /** Stop the auto-scroll loop. */
  const stopAutoScroll = useCallback(() => {
    scrollDirRef.current = 0;
    if (scrollRafRef.current !== null) {
      cancelAnimationFrame(scrollRafRef.current);
      scrollRafRef.current = null;
    }
  }, []);

  /** Start or update the auto-scroll loop for the given direction. */
  const startAutoScroll = useCallback((dir: -1 | 1) => {
    scrollDirRef.current = dir;
    if (scrollRafRef.current !== null) return; // already running
    const tick = () => {
      if (scrollDirRef.current === 0 || !containerRef.current) {
        scrollRafRef.current = null;
        return;
      }
      containerRef.current.scrollBy({
        top: scrollDirRef.current * SCROLL_SPEED,
      });
      scrollRafRef.current = requestAnimationFrame(tick);
    };
    scrollRafRef.current = requestAnimationFrame(tick);
  }, []);

  // Clean up rAF on unmount
  useEffect(() => () => stopAutoScroll(), [stopAutoScroll]);

  /** Set both internal ref and parent's ref for cross-window hit-testing. */
  const setContainerRef = useCallback(
    (el: HTMLDivElement | null) => {
      (containerRef as React.MutableRefObject<HTMLDivElement | null>).current =
        el;
      containerRefProp?.(el);
    },
    [containerRefProp],
  );

  const commands = useEntityCommands("column", column.id, column);
  // Compute drop zones at render time — each zone carries preconfigured placement data
  const zones = useMemo(
    () =>
      computeDropZones(
        tasks.map((t) => t.id),
        column.id,
        boardPath ?? "",
      ),
    [tasks, column.id, boardPath],
  );

  /** Build a "Do This Next" command for a task, or null if the task is already first in todo. */
  const buildDoThisNextCommand = useCallback(
    (taskId: string): CommandDef | null => {
      // Don't show if task is already the first item in todo
      if (taskId === firstTodoTaskId) return null;
      return {
        id: "task.doThisNext",
        name: "Do This Next",
        contextMenu: true,
        execute: () => {
          const args: Record<string, unknown> = { id: taskId, column: "todo" };
          if (firstTodoTaskId) args.before_id = firstTodoTaskId;
          invoke("dispatch_command", {
            cmd: "task.move",
            args,
            ...(boardPath ? { boardPath } : {}),
          }).catch(console.error);
        },
      };
    },
    [firstTodoTaskId, boardPath],
  );

  /** Memoized extra commands per task — includes "Do This Next" when applicable. */
  const taskExtraCommands = useMemo(() => {
    const map = new Map<string, CommandDef[]>();
    for (const task of tasks) {
      const cmd = buildDoThisNextCommand(task.id);
      if (cmd) map.set(task.id, [cmd]);
    }
    return map;
  }, [tasks, buildDoThisNextCommand]);

  // Scroll the card container to top when cursor jumps to column header.
  // The header FocusHighlight is outside the scrollable container, so its
  // scrollIntoView won't reset the card list's scroll position.
  useEffect(() => {
    if (focusedCardIndex === -1 && containerRef.current) {
      containerRef.current.scrollTop = 0;
    }
  }, [focusedCardIndex]);

  /** Allow drops in the column + auto-scroll near edges. */
  const handleContainerDragOver = useCallback(
    (e: React.DragEvent) => {
      // preventDefault is REQUIRED — without it the browser rejects drops
      // on child DropZones inside this container.
      e.preventDefault();
      e.dataTransfer.dropEffect = "move";
      if (!containerRef.current) return;
      const rect = containerRef.current.getBoundingClientRect();
      if (e.clientY < rect.top + SCROLL_ZONE) {
        startAutoScroll(-1);
      } else if (e.clientY > rect.bottom - SCROLL_ZONE) {
        startAutoScroll(1);
      } else {
        stopAutoScroll();
      }
    },
    [startAutoScroll, stopAutoScroll],
  );

  /** Forward zone drops to parent. */
  const handleZoneDrop = useCallback(
    (descriptor: DropZoneDescriptor, taskData: string) => {
      onDropProp?.(descriptor, taskData);
    },
    [onDropProp],
  );

  return (
    <FocusScope
      moniker={columnMoniker}
      commands={commands}
      className="flex flex-col min-h-0 min-w-[20em] max-w-[40em] flex-1"
    >
      <div className="flex flex-col min-h-0 min-w-0 flex-1">
        <FocusHighlight
          focused={focusedCardIndex === -1}
          className="column-header-focus px-3 py-2 flex items-center gap-2 rounded"
          onClickCapture={() => navActions?.onHeaderClick(column.id)}
        >
          {nameFieldDef ? (
            <Field
              fieldDef={nameFieldDef}
              entityType="column"
              entityId={column.id}
              mode="compact"
              editing={editingName}
              onEdit={() => setEditingName(true)}
              onDone={() => setEditingName(false)}
              onCancel={() => setEditingName(false)}
            />
          ) : (
            <span className="text-sm font-semibold text-foreground">
              {getStr(column, "name")}
            </span>
          )}
          <Badge variant="secondary">{tasks.length}</Badge>
          <div className="flex-1" />
          {onAddTask && (
            <button
              type="button"
              className="p-0.5 rounded text-muted-foreground/50 hover:text-muted-foreground hover:bg-muted transition-colors"
              onClick={() => onAddTask(column.id)}
              title={`Add task to ${getStr(column, "name")}`}
            >
              <Plus className="h-4 w-4" />
            </button>
          )}
        </FocusHighlight>
        <div
          ref={setContainerRef}
          className="flex-1 overflow-y-auto [scrollbar-gutter:stable] px-2 pt-1 pb-2 m-1 rounded-lg border-2 border-transparent"
          onDragOver={handleContainerDragOver}
        >
          {tasks.length === 0 ? (
            <DropZone
              descriptor={zones[0]}
              dragTaskId={dragTaskId}
              onDrop={handleZoneDrop}
              variant="empty-column"
            />
          ) : (
            <>
              {tasks.map((entity, i) => (
                <div key={entity.id}>
                  <DropZone
                    descriptor={zones[i]}
                    dragTaskId={dragTaskId}
                    onDrop={handleZoneDrop}
                  />
                  <FocusHighlight
                    focused={focusedCardIndex === i}
                    className="rounded"
                    onClickCapture={() => navActions?.onCardClick(column.id, i)}
                    onDoubleClickCapture={() =>
                      navActions?.onCardDoubleClick(column.id, i)
                    }
                  >
                    <DraggableTaskCard
                      entity={entity}
                      onDragStart={onTaskDragStart}
                      onDragEnd={onTaskDragEnd}
                      extraCommands={taskExtraCommands.get(entity.id)}
                    />
                  </FocusHighlight>
                </div>
              ))}
              {/* Final zone after the last card */}
              <DropZone
                descriptor={zones[zones.length - 1]}
                dragTaskId={dragTaskId}
                onDrop={handleZoneDrop}
              />
            </>
          )}
        </div>
      </div>
    </FocusScope>
  );
});
