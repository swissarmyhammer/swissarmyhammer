import { useSortable } from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import { GripHorizontal } from "lucide-react";
import type { ReactNode } from "react";

interface SortableColumnProps {
  id: string;
  children: ReactNode;
  showSeparator: boolean;
}

export function SortableColumn({ id, children, showSeparator }: SortableColumnProps) {
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
      className="flex flex-1 min-w-[20em] max-w-[60em] relative"
    >
      {showSeparator && <div className="w-px bg-border shrink-0 my-3" />}
      <div className="flex flex-col min-h-0 flex-1">
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
