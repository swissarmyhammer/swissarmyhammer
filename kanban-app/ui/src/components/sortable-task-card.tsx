import { useCallback, useRef, useState } from "react";
import { EntityCard } from "@/components/entity-card";
import type { Entity } from "@/types/kanban";

interface DraggableTaskCardProps {
  entity: Entity;
  isBlocked?: boolean;
  onDragStart?: (entity: Entity) => void;
  onDragEnd?: (entity: Entity, dropEffect: string) => void;
}

/**
 * HTML5 draggable task card.
 *
 * Uses a DOM clone as the OS drag ghost — visible in source window and
 * between windows. The target window's BoardView overlays a full-size
 * EntityCard on top so the shrunken OS ghost is covered.
 */
export function DraggableTaskCard({
  entity,
  isBlocked,
  onDragStart,
  onDragEnd,
}: DraggableTaskCardProps) {
  const cardRef = useRef<HTMLDivElement>(null);
  const [isDragging, setIsDragging] = useState(false);

  const handleDragStart = useCallback(
    (e: React.DragEvent) => {
      e.dataTransfer.setData(
        "application/x-swissarmyhammer-task",
        JSON.stringify(entity),
      );
      e.dataTransfer.effectAllowed = "move";

      // Clone the card DOM for the drag image
      if (cardRef.current) {
        const clone = cardRef.current.cloneNode(true) as HTMLElement;
        clone.style.position = "absolute";
        clone.style.left = "-9999px";
        clone.style.top = "0";
        clone.style.width = `${cardRef.current.offsetWidth}px`;
        clone.style.opacity = "1";
        clone.style.pointerEvents = "none";
        document.body.appendChild(clone);
        e.dataTransfer.setDragImage(clone, 20, 20);
        requestAnimationFrame(() => clone.remove());
      }

      setIsDragging(true);
      onDragStart?.(entity);
    },
    [entity, onDragStart],
  );

  const handleDragEnd = useCallback(
    (e: React.DragEvent) => {
      setIsDragging(false);
      onDragEnd?.(entity, e.dataTransfer.dropEffect);
    },
    [entity, onDragEnd],
  );

  return (
    <EntityCard
      ref={cardRef}
      entity={entity}
      isBlocked={isBlocked}
      style={{ opacity: isDragging ? 0.3 : undefined }}
      draggable
      onDragStart={handleDragStart}
      onDragEnd={handleDragEnd}
    />
  );
}
