/**
 * Cross-window drop overlay.
 *
 * When a drag session is active, renders transparent drop zones over each
 * column with a floating ghost card that follows the mouse cursor.
 *
 * Column highlighting is driven by mouse position (not mouseenter/leave)
 * for reliable tracking when the pointer enters from outside the window.
 *
 * Detects Alt/Option key for copy mode — holding Alt during drop
 * copies the task instead of moving it.
 */

import { useCallback, useEffect, useRef, useState } from "react";
import { useDragSession } from "@/lib/drag-session-context";
import type { Entity } from "@/types/kanban";
import { getStr } from "@/types/kanban";

interface CrossWindowDropOverlayProps {
  columns: Entity[];
  /** Task IDs per column, in display order. */
  tasksByColumn: Map<string, string[]>;
}

export function CrossWindowDropOverlay({
  columns,
  tasksByColumn,
}: CrossWindowDropOverlayProps) {
  const { session, completeSession, isSource } = useDragSession();
  const [hoveredColumn, setHoveredColumn] = useState<string | null>(null);
  const [altHeld, setAltHeld] = useState(false);
  const [mousePos, setMousePos] = useState<{ x: number; y: number } | null>(
    null,
  );
  /** Refs for each column zone to hit-test mouse position. */
  const columnRefs = useRef<Map<string, HTMLDivElement>>(new Map());

  // Track Alt/Option key state for copy mode
  useEffect(() => {
    if (!session) return;
    const handleKey = (e: KeyboardEvent) => setAltHeld(e.altKey);
    window.addEventListener("keydown", handleKey);
    window.addEventListener("keyup", handleKey);
    return () => {
      window.removeEventListener("keydown", handleKey);
      window.removeEventListener("keyup", handleKey);
      setAltHeld(false);
    };
  }, [session]);

  // Track mouse position and determine which column is hovered via hit-testing.
  // This is more reliable than mouseenter/leave when the pointer enters from
  // outside the window (cross-window drag).
  useEffect(() => {
    if (!session) return;
    const handleMove = (e: MouseEvent) => {
      if (!isSource) {
        setMousePos({ x: e.clientX, y: e.clientY });
      }

      // Hit-test column zones by checking which element rect contains the cursor
      let found: string | null = null;
      for (const [colId, el] of columnRefs.current) {
        const rect = el.getBoundingClientRect();
        if (
          e.clientX >= rect.left &&
          e.clientX <= rect.right &&
          e.clientY >= rect.top &&
          e.clientY <= rect.bottom
        ) {
          found = colId;
          break;
        }
      }
      setHoveredColumn(found);
    };
    window.addEventListener("mousemove", handleMove);
    return () => {
      window.removeEventListener("mousemove", handleMove);
      setMousePos(null);
      setHoveredColumn(null);
    };
  }, [session, isSource]);

  const handleMouseUp = useCallback(
    (columnId: string, e: React.MouseEvent) => {
      if (!session) return;
      const tasks = tasksByColumn.get(columnId) ?? [];
      completeSession(columnId, {
        dropIndex: tasks.length,
        copyMode: e.altKey || altHeld,
      });
      setHoveredColumn(null);
    },
    [session, completeSession, tasksByColumn, altHeld],
  );

  // Only show when there's an active drag session
  if (!session) return null;

  const isCopyMode = session.copy_mode || altHeld;
  const taskTitle = (session.task_fields.title as string) ?? "Task";

  // In the source window, show the overlay visually but don't capture pointer
  // events — @dnd-kit is still handling the drag there. In target windows,
  // capture events so mouseup triggers the drop.
  const capturePointer = !isSource;

  /** Store ref for a column zone element. */
  const setColumnRef = (colId: string) => (el: HTMLDivElement | null) => {
    if (el) {
      columnRefs.current.set(colId, el);
    } else {
      columnRefs.current.delete(colId);
    }
  };

  return (
    <div
      className="absolute inset-0 z-50 flex"
      style={{ pointerEvents: capturePointer ? "auto" : "none" }}
    >
      {columns.map((col) => (
        <div
          key={col.id}
          ref={setColumnRef(col.id)}
          className={`flex-1 flex flex-col items-center justify-center border-2 border-dashed transition-colors duration-150 m-1 rounded-lg ${
            hoveredColumn === col.id
              ? "border-primary/80 bg-primary/10 shadow-inner"
              : "border-muted-foreground/20 bg-background/40"
          }`}
          onMouseUp={capturePointer ? (e) => handleMouseUp(col.id, e) : undefined}
        >
          <div className="text-sm font-medium text-muted-foreground pointer-events-none">
            {getStr(col, "name")}
          </div>
          {isCopyMode && hoveredColumn === col.id && (
            <div className="mt-1 text-xs text-muted-foreground/70 pointer-events-none">
              Copy
            </div>
          )}
        </div>
      ))}

      {/* Floating ghost card following the cursor (target windows only) */}
      {mousePos && (
        <div
          className="fixed pointer-events-none z-[60]"
          style={{
            left: mousePos.x + 12,
            top: mousePos.y - 8,
          }}
        >
          <div className="px-3 py-2 rounded-md bg-card border border-border shadow-lg text-sm text-foreground max-w-56 truncate opacity-90">
            {isCopyMode && (
              <span className="mr-1.5 text-primary font-bold">+</span>
            )}
            {taskTitle}
          </div>
        </div>
      )}
    </div>
  );
}
