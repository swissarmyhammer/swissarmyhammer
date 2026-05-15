import { useSortable } from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import { GripHorizontal } from "lucide-react";
import type { ReactNode } from "react";

interface SortableColumnProps {
  id: string;
  children: ReactNode;
  showSeparator: boolean;
}

export function SortableColumn({
  id,
  children,
  showSeparator,
}: SortableColumnProps) {
  const {
    attributes,
    listeners,
    setNodeRef,
    transform,
    transition,
    isDragging,
  } = useSortable({ id, data: { type: "column" } });

  const style: React.CSSProperties = {
    transform: CSS.Translate.toString(transform),
    transition,
    opacity: isDragging ? 0.3 : undefined,
  };

  return (
    <div
      ref={setNodeRef}
      style={style}
      // `shrink-0` is load-bearing: without it, this flex item shrinks under the
      // parent strip's pressure and the inner ColumnView (which itself carries
      // `min-w-[24em] shrink-0`) overflows its slot, visually overlapping the
      // next column. With `shrink-0` the strip's `overflow-x-auto` scroll
      // container takes over once the columns no longer fit, exactly as the
      // user expects.
      //
      // `min-w-[24em]` matches the inner ColumnView's minimum so the slot
      // never reports a smaller width than its content. `max-w-[60em]` stays
      // wider than the inner `max-w-[48em]` to leave headroom for the
      // separator + grip + paddings.
      className="flex flex-1 shrink-0 min-w-[24em] max-w-[60em] relative"
    >
      {showSeparator && <div className="w-px bg-border shrink-0 my-3" />}
      <div className="flex flex-col min-h-0 min-w-0 flex-1">
        <div className="flex items-center justify-center">
          <button
            type="button"
            className="p-1 text-muted-foreground/30 hover:text-muted-foreground cursor-grab active:cursor-grabbing touch-none"
            {...listeners}
            {...attributes}
          >
            <GripHorizontal className="h-3 w-3" />
          </button>
        </div>
        {children}
      </div>
    </div>
  );
}
