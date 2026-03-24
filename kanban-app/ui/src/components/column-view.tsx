import { useCallback, useEffect, useRef, useState } from "react";
import { Inbox, Plus } from "lucide-react";
import { Field } from "@/components/fields/field";
import { DraggableTaskCard } from "@/components/sortable-task-card";
import { FocusScope } from "@/components/focus-scope";
import { Badge } from "@/components/ui/badge";
import { moniker } from "@/lib/moniker";
import { useEntityCommands } from "@/lib/entity-commands";
import { useSchema } from "@/lib/schema-context";
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
  /** Called during dragover with the computed insert index. */
  onDragOver?: (columnId: string, insertIndex: number) => void;
  /** Called when a task is dropped on this column. */
  onDrop?: (columnId: string, taskData: string, insertIndex: number) => void;
  /** Called when drag enters this column. */
  onDragEnter?: (columnId: string) => void;
  /** Called when drag leaves this column. */
  onDragLeave?: (columnId: string) => void;
  /** Externally controlled insert marker position. */
  insertAtIndex?: number | null;
  /** Whether this column is the target of an active drag (intra or cross-window). */
  isDragTarget?: boolean;
  /** Ref callback for the column container — used for cross-window hit-testing. */
  containerRef?: (el: HTMLDivElement | null) => void;
}

/** Distance from container edge (px) that triggers auto-scroll during drag. */
const SCROLL_ZONE = 40;
/** Pixels per animation frame to scroll when in the edge zone. */
const SCROLL_SPEED = 6;

/** Compute the insert index by comparing dragover Y to card midpoints. */
function computeInsertIndex(container: HTMLElement, clientY: number): number {
  const cards = container.querySelectorAll<HTMLElement>("[data-entity-card]");
  for (let i = 0; i < cards.length; i++) {
    const rect = cards[i].getBoundingClientRect();
    const midY = rect.top + rect.height / 2;
    if (clientY < midY) return i;
  }
  return cards.length;
}

export function ColumnView({
  column,
  tasks,
  onAddTask,
  onTaskDragStart,
  onTaskDragEnd,
  onDragOver: onDragOverProp,
  onDrop: onDropProp,
  onDragEnter,
  onDragLeave,
  insertAtIndex,
  isDragTarget: isDragTargetProp,
  containerRef: containerRefProp,
}: ColumnViewProps) {
  const columnMoniker = moniker("column", column.id);
  const { getFieldDef } = useSchema();
  const nameFieldDef = getFieldDef("column", "name");
  const [editingName, setEditingName] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);
  const [localInsert, setLocalInsert] = useState<number | null>(null);
  const [isDragOver, setIsDragOver] = useState(false);
  /** rAF handle for edge-scroll loop during drag. */
  const scrollRafRef = useRef<number | null>(null);
  /** Current scroll direction: -1 (up), 0 (none), 1 (down). */
  const scrollDirRef = useRef(0);

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

  const insertIndex = insertAtIndex ?? localInsert;
  const showDashes = isDragOver || isDragTargetProp;

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

  const clearDragState = useCallback(() => {
    setIsDragOver(false);
    setLocalInsert(null);
  }, []);

  const handleDragOver = useCallback(
    (e: React.DragEvent) => {
      e.preventDefault();
      e.dataTransfer.dropEffect = "move";

      if (!isDragOver) {
        setIsDragOver(true);
        onDragEnter?.(column.id);
      }

      if (containerRef.current) {
        const idx = computeInsertIndex(containerRef.current, e.clientY);
        setLocalInsert(idx);
        onDragOverProp?.(column.id, idx);

        // Auto-scroll when cursor is near the container's top or bottom edge
        const rect = containerRef.current.getBoundingClientRect();
        if (e.clientY < rect.top + SCROLL_ZONE) {
          startAutoScroll(-1);
        } else if (e.clientY > rect.bottom - SCROLL_ZONE) {
          startAutoScroll(1);
        } else {
          stopAutoScroll();
        }
      }
    },
    [
      column.id,
      isDragOver,
      onDragOverProp,
      onDragEnter,
      startAutoScroll,
      stopAutoScroll,
    ],
  );

  /** Clear drag visuals when the cursor leaves this column's container. */
  const handleDragLeave = useCallback(
    (e: React.DragEvent) => {
      // Ignore spurious leave events from entering child elements —
      // only clear when the cursor actually leaves the container.
      if (
        containerRef.current &&
        e.relatedTarget instanceof Node &&
        containerRef.current.contains(e.relatedTarget)
      ) {
        return;
      }
      stopAutoScroll();
      clearDragState();
      onDragLeave?.(column.id);
    },
    [column.id, onDragLeave, clearDragState, stopAutoScroll],
  );

  const handleDrop = useCallback(
    (e: React.DragEvent) => {
      e.preventDefault();
      stopAutoScroll();
      clearDragState();
      const taskData = e.dataTransfer.getData(
        "application/x-swissarmyhammer-task",
      );
      const idx = containerRef.current
        ? computeInsertIndex(containerRef.current, e.clientY)
        : tasks.length;
      onDropProp?.(column.id, taskData, idx);
    },
    [column.id, tasks.length, onDropProp, clearDragState, stopAutoScroll],
  );

  return (
    <FocusScope
      moniker={columnMoniker}
      commands={commands}
      className="flex flex-col min-h-0 min-w-[20em] max-w-[40em] flex-1"
    >
      <div className="flex flex-col min-h-0 min-w-0 flex-1">
        <div className="px-3 py-2 flex items-center gap-2">
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
        </div>
        <div
          ref={setContainerRef}
          className={`flex-1 overflow-y-auto px-2 pt-1 pb-2 space-y-1.5 m-1 rounded-lg border-2 transition-colors duration-150 ${
            showDashes
              ? "border-dashed border-primary/60"
              : "border-transparent"
          }`}
          onDragOver={handleDragOver}
          onDragLeave={handleDragLeave}
          onDrop={handleDrop}
        >
          {tasks.length === 0 && insertIndex == null ? (
            <div className="flex flex-col items-center justify-center h-full text-muted-foreground opacity-40">
              <Inbox className="h-8 w-8 mb-2" />
              <p className="text-xs">No tasks</p>
            </div>
          ) : (
            tasks.map((entity, i) => (
              <div key={entity.id}>
                {insertIndex === i && (
                  <div className="h-1 bg-primary rounded-full mx-1 my-1.5 shadow-sm shadow-primary/50" />
                )}
                <DraggableTaskCard
                  entity={entity}
                  onDragStart={onTaskDragStart}
                  onDragEnd={onTaskDragEnd}
                />
              </div>
            ))
          )}
          {/* Insertion marker at the end */}
          {insertIndex != null && insertIndex >= tasks.length && (
            <div className="h-1 bg-primary rounded-full mx-1 my-1.5 shadow-sm shadow-primary/50" />
          )}
        </div>
      </div>
    </FocusScope>
  );
}
