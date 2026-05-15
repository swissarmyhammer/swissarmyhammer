import { memo, useCallback, useRef, useState } from "react";
import { EntityCard } from "@/components/entity-card";
import type { CommandDef } from "@/lib/command-scope";
import type { Entity } from "@/types/kanban";

interface DraggableTaskCardProps {
  entity: Entity;
  onDragStart?: (entity: Entity) => void;
  onDragEnd?: (entity: Entity, dropEffect: string) => void;
  /** Additional commands to pass through to EntityCard's context menu. */
  extraCommands?: CommandDef[];
}

/**
 * HTML5 draggable task card.
 *
 * Uses a DOM clone as the OS drag ghost — visible in source window and
 * between windows. The target window's BoardView overlays a full-size
 * EntityCard on top so the shrunken OS ghost is covered.
 *
 * Wrapped in React.memo so cards whose entity reference hasn't changed
 * skip re-rendering when the parent column re-renders.
 */
export const DraggableTaskCard = memo(function DraggableTaskCard({
  entity,
  onDragStart,
  onDragEnd,
  extraCommands,
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
        clone.style.position = "fixed";
        clone.style.left = "-9999px";
        clone.style.top = "-9999px";
        clone.style.width = `${cardRef.current.offsetWidth}px`;
        clone.style.height = `${cardRef.current.offsetHeight}px`;
        clone.style.transform = "none";
        clone.style.zoom = "1";
        clone.style.opacity = "1";
        clone.style.pointerEvents = "none";
        // The OS drag image is built from this clone (outside React).
        // Strip focus indicators so the ghost doesn't show the bar.
        clone.removeAttribute("data-focused");
        clone.removeAttribute("data-focus-depth");
        for (const el of clone.querySelectorAll("[data-focused]")) {
          el.removeAttribute("data-focused");
          el.removeAttribute("data-focus-depth");
        }
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
      style={{ opacity: isDragging ? 0.3 : undefined }}
      draggable
      onDragStart={handleDragStart}
      onDragEnd={handleDragEnd}
      extraCommands={extraCommands}
    />
  );
});
