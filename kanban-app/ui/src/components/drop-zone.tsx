import { useCallback, useState } from "react";
import { Inbox } from "lucide-react";
import type { DropZoneDescriptor } from "@/lib/drop-zones";

const DRAG_MIME = "application/x-swissarmyhammer-task";

interface DropZoneProps {
  /** The pre-computed descriptor carrying placement data. */
  descriptor: DropZoneDescriptor;
  /** ID of the task currently being dragged (for no-op suppression). */
  dragTaskId?: string | null;
  /** Called when a task is dropped on this zone. */
  onDrop: (descriptor: DropZoneDescriptor, taskData: string) => void;
  /** Visual variant: thin bar between cards, or full placeholder for empty columns. */
  variant?: "between" | "empty-column";
}

/**
 * A single drop target zone rendered between (or around) task cards.
 *
 * Each zone carries its own DropZoneDescriptor with pre-computed placement
 * data, so the drop handler needs no runtime midpoint computation.
 */
export function DropZone({
  descriptor,
  dragTaskId,
  onDrop,
  variant = "between",
}: DropZoneProps) {
  const [isOver, setIsOver] = useState(false);

  const handleDragOver = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    e.dataTransfer.dropEffect = "move";
    setIsOver(true);
  }, []);

  const handleDragEnter = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    setIsOver(true);
  }, []);

  const handleDragLeave = useCallback((e: React.DragEvent) => {
    e.stopPropagation();
    // Only clear if we actually left this element (not entering a child).
    if (!e.currentTarget.contains(e.relatedTarget as Node)) {
      setIsOver(false);
    }
  }, []);

  const handleDrop = useCallback(
    (e: React.DragEvent) => {
      e.preventDefault();
      e.stopPropagation();
      setIsOver(false);
      const taskData = e.dataTransfer.getData(DRAG_MIME);
      if (taskData) {
        onDrop(descriptor, taskData);
      }
    },
    [descriptor, onDrop],
  );

  // No-op: if the dragged task is the one this zone references, render as
  // an inert spacer (same height, no drop handling) to keep layout stable.
  const isNoOp =
    !!dragTaskId &&
    (dragTaskId === descriptor.beforeId || dragTaskId === descriptor.afterId);

  if (isNoOp && variant !== "empty-column") {
    return (
      <div
        data-drop-zone
        {...(descriptor.beforeId
          ? { "data-drop-before": descriptor.beforeId }
          : {})}
        {...(descriptor.afterId
          ? { "data-drop-after": descriptor.afterId }
          : {})}
        style={{ height: 6 }}
      />
    );
  }

  if (variant === "empty-column") {
    return (
      <div
        data-drop-zone
        data-drop-empty
        className={`flex flex-col items-center justify-center flex-1 min-h-[120px] rounded-lg border-2 border-dashed transition-colors duration-150 ${
          isOver ? "border-primary/60 bg-primary/5" : "border-transparent"
        }`}
        onDragOver={handleDragOver}
        onDragEnter={handleDragEnter}
        onDragLeave={handleDragLeave}
        onDrop={handleDrop}
      >
        <div className="flex flex-col items-center text-muted-foreground opacity-40">
          <Inbox className="h-8 w-8 mb-2" />
          <p className="text-xs">No tasks</p>
        </div>
      </div>
    );
  }

  // "between" variant — thin line that expands when a drag is active
  return (
    <div
      data-drop-zone
      {...(descriptor.beforeId
        ? { "data-drop-before": descriptor.beforeId }
        : {})}
      {...(descriptor.afterId ? { "data-drop-after": descriptor.afterId } : {})}
      className="relative transition-all duration-150"
      style={{ height: isOver ? 24 : 6 }}
      onDragOver={handleDragOver}
      onDragEnter={handleDragEnter}
      onDragLeave={handleDragLeave}
      onDrop={handleDrop}
    >
      {isOver && (
        <div className="absolute inset-x-1 top-1/2 -translate-y-1/2 h-1 bg-primary rounded-full shadow-sm shadow-primary/50" />
      )}
    </div>
  );
}
