import { useSortable } from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import { EntityCard } from "@/components/entity-card";
import type { Entity } from "@/types/kanban";

interface SortableEntityCardProps {
  entity: Entity;
  isBlocked?: boolean;
}

export function SortableEntityCard({ entity, isBlocked }: SortableEntityCardProps) {
  const {
    attributes,
    listeners,
    setNodeRef,
    transform,
    transition,
    isDragging,
  } = useSortable({ id: entity.id, data: { type: "task" } });

  const style: React.CSSProperties = {
    transform: CSS.Translate.toString(transform),
    transition,
    opacity: isDragging ? 0.3 : undefined,
  };

  return (
    <EntityCard
      ref={setNodeRef}
      style={style}
      entity={entity}
      isBlocked={isBlocked}
      dragHandleProps={{ ...listeners, ...attributes }}
    />
  );
}
