import { memo, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Plus } from "lucide-react";
import { DropZone } from "@/components/drop-zone";
import { computeDropZones, type DropZoneDescriptor } from "@/lib/drop-zones";
import { Field } from "@/components/fields/field";
import { DraggableTaskCard } from "@/components/sortable-task-card";
import { FocusScope } from "@/components/focus-scope";
import { Badge } from "@/components/ui/badge";
import { moniker } from "@/lib/moniker";
import { useEntityCommands } from "@/lib/entity-commands";
import { useSchema } from "@/lib/schema-context";
import { useEntityFocus, type ClaimPredicate } from "@/lib/entity-focus-context";
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
  /**
   * Task monikers in the column to the left (in order), or empty array.
   * Used to compute nav.right claimWhen predicates for cross-column navigation.
   */
  leftColumnTaskMonikers?: string[];
  /** Column header moniker for the column to the left, or null. */
  leftColumnHeaderMoniker?: string | null;
  /**
   * Task monikers in the column to the right (in order), or empty array.
   * Used to compute nav.left claimWhen predicates for cross-column navigation.
   */
  rightColumnTaskMonikers?: string[];
  /** Column header moniker for the column to the right, or null. */
  rightColumnHeaderMoniker?: string | null;
  /**
   * All task monikers on the board, across all columns.
   * Used for nav.first/nav.last predicates (any board card focused -> claim).
   */
  allBoardTaskMonikers?: Set<string>;
  /** All column header monikers on the board. Used for nav.first/nav.last. */
  allBoardHeaderMonikers?: Set<string>;
  /** Whether this column contains the overall first card on the board. */
  isFirstColumn?: boolean;
  /** Whether this column contains the overall last card on the board. */
  isLastColumn?: boolean;
}

/** Distance from container edge (px) that triggers auto-scroll during drag. */
const SCROLL_ZONE = 40;
/** Pixels per animation frame to scroll when in the edge zone. */
const SCROLL_SPEED = 6;

/**
 * Renders a single column in the board view with drag-drop, focus highlight,
 * and keyboard navigation support.
 *
 * Navigation is pull-based: each card and the column header declare claimWhen
 * predicates so broadcastNavCommand can route focus without a push-based cursor.
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
  leftColumnTaskMonikers = [],
  leftColumnHeaderMoniker = null,
  rightColumnTaskMonikers = [],
  rightColumnHeaderMoniker = null,
  allBoardTaskMonikers,
  allBoardHeaderMonikers,
  isFirstColumn = false,
  isLastColumn = false,
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

  const { setFocus } = useEntityFocus();

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

  // --- Compute claimWhen predicates for header and each card ---

  /** Monikers for tasks in this column, in display order. */
  const taskMonikers = useMemo(
    () => tasks.map((t) => moniker("task", t.id)),
    [tasks],
  );

  /**
   * Helper: returns true if the focused moniker belongs to any card or header
   * on the board. Used for nav.first/nav.last predicates.
   */
  const isBoardElement = useCallback(
    (f: string | null): boolean => {
      if (!f) return false;
      if (allBoardTaskMonikers?.has(f)) return true;
      if (allBoardHeaderMonikers?.has(f)) return true;
      return false;
    },
    [allBoardTaskMonikers, allBoardHeaderMonikers],
  );

  /** ClaimWhen predicates for the column header FocusScope. */
  const headerClaimWhen = useMemo<ClaimPredicate[]>(() => {
    const predicates: ClaimPredicate[] = [];

    // nav.up: claim when the first card in this column is focused
    if (taskMonikers.length > 0) {
      const firstCard = taskMonikers[0];
      predicates.push({
        command: "nav.up",
        when: (f) => f === firstCard,
      });
    }

    // nav.left: claim when the header of the column to the right is focused
    if (rightColumnHeaderMoniker) {
      predicates.push({
        command: "nav.left",
        when: (f) => f === rightColumnHeaderMoniker,
      });
    }

    // nav.right: claim when the header of the column to the left is focused
    if (leftColumnHeaderMoniker) {
      predicates.push({
        command: "nav.right",
        when: (f) => f === leftColumnHeaderMoniker,
      });
    }

    // Cross-column nav to an empty column: any card in the adjacent column
    // should land on this header since there are no cards to target.
    if (tasks.length === 0) {
      for (const m of rightColumnTaskMonikers) {
        predicates.push({
          command: "nav.left",
          when: (f) => f === m,
        });
      }
      for (const m of leftColumnTaskMonikers) {
        predicates.push({
          command: "nav.right",
          when: (f) => f === m,
        });
      }
    }

    // nav.first: claim if I'm the first column's header and column is empty
    // and any board element is focused (except me).
    if (isFirstColumn && tasks.length === 0) {
      predicates.push({
        command: "nav.first",
        when: (f) => isBoardElement(f) && f !== columnMoniker,
      });
    }

    // nav.last: claim if I'm the last column's header and column is empty
    // (so there's no card to be the last element).
    if (isLastColumn && tasks.length === 0) {
      predicates.push({
        command: "nav.last",
        when: (f) => isBoardElement(f) && f !== columnMoniker,
      });
    }

    return predicates;
  }, [
    taskMonikers,
    rightColumnHeaderMoniker,
    leftColumnHeaderMoniker,
    rightColumnTaskMonikers,
    leftColumnTaskMonikers,
    tasks.length,
    isFirstColumn,
    isLastColumn,
    columnMoniker,
    isBoardElement,
  ]);

  /** ClaimWhen predicates per card, indexed by position. */
  const cardClaimPredicates = useMemo<ClaimPredicate[][]>(() => {
    return taskMonikers.map((_, i) => {
      const predicates: ClaimPredicate[] = [];

      // nav.down: claim when the element above me is focused
      if (i === 0) {
        // First card claims nav.down when column header is focused
        predicates.push({
          command: "nav.down",
          when: (f) => f === columnMoniker,
        });
      } else {
        const prev = taskMonikers[i - 1];
        predicates.push({
          command: "nav.down",
          when: (f) => f === prev,
        });
      }

      // nav.up: claim when the element below me is focused
      if (i < taskMonikers.length - 1) {
        const next = taskMonikers[i + 1];
        predicates.push({
          command: "nav.up",
          when: (f) => f === next,
        });
      }

      // nav.left: claim when a card in the column to the right is focused
      // and this card is the clamped target position.
      for (let ri = 0; ri < rightColumnTaskMonikers.length; ri++) {
        const rightMoniker = rightColumnTaskMonikers[ri];
        const clampedTarget = Math.min(ri, taskMonikers.length - 1);
        if (clampedTarget === i) {
          predicates.push({
            command: "nav.left",
            when: (f) => f === rightMoniker,
          });
        }
      }
      // nav.left: also claim when right column's header is focused and
      // we should clamp to card -1 (header). But header-to-header is
      // handled above, so only handle right-header -> this card when
      // the right column has no cards (user is on empty-column header,
      // moves left to this column which has cards -> first card? No,
      // header-to-header is correct). Actually, header nav.left/right
      // always goes header-to-header. No card claims needed for that.

      // nav.right: claim when a card in the column to the left is focused
      for (let li = 0; li < leftColumnTaskMonikers.length; li++) {
        const leftMoniker = leftColumnTaskMonikers[li];
        const clampedTarget = Math.min(li, taskMonikers.length - 1);
        if (clampedTarget === i) {
          predicates.push({
            command: "nav.right",
            when: (f) => f === leftMoniker,
          });
        }
      }

      // nav.first: claim if I'm the first card of the first column
      if (isFirstColumn && i === 0) {
        predicates.push({
          command: "nav.first",
          when: (f) => isBoardElement(f) && f !== taskMonikers[0],
        });
      }

      // nav.last: claim if I'm the last card of the last column
      if (isLastColumn && i === taskMonikers.length - 1) {
        predicates.push({
          command: "nav.last",
          when: (f) =>
            isBoardElement(f) && f !== taskMonikers[taskMonikers.length - 1],
        });
      }

      return predicates;
    });
  }, [
    taskMonikers,
    columnMoniker,
    rightColumnTaskMonikers,
    leftColumnTaskMonikers,
    isFirstColumn,
    isLastColumn,
    isBoardElement,
  ]);

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
      claimWhen={headerClaimWhen}
      className="flex flex-col min-h-0 min-w-[20em] max-w-[40em] flex-1"
    >
      <div className="flex flex-col min-h-0 min-w-0 flex-1">
        <div
          className="column-header-focus px-3 py-2 flex items-center gap-2 rounded"
          onClickCapture={() => setFocus(columnMoniker)}
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
        </div>
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
                  <div className="rounded">
                    <DraggableTaskCard
                      entity={entity}
                      onDragStart={onTaskDragStart}
                      onDragEnd={onTaskDragEnd}
                      extraCommands={taskExtraCommands.get(entity.id)}
                      claimWhen={cardClaimPredicates[i]}
                    />
                  </div>
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
