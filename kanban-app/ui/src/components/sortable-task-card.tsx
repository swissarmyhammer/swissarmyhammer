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

/** Create a DOM clone of the card for the OS drag ghost image. */
function setDragGhostImage(e: React.DragEvent, source: HTMLElement) {
  const clone = source.cloneNode(true) as HTMLElement;
  Object.assign(clone.style, {
    position: "fixed", left: "-9999px", top: "-9999px",
    width: `${source.offsetWidth}px`, height: `${source.offsetHeight}px`,
    transform: "none", zoom: "1", opacity: "1", pointerEvents: "none",
  });
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

/**
 * HTML5 draggable task card with DOM clone ghost.
 *
 * Wrapped in React.memo so cards whose entity reference hasn't changed
 * skip re-rendering when the parent column re-renders.
 */
export const DraggableTaskCard = memo(function DraggableTaskCard({
  entity, onDragStart, onDragEnd, extraCommands,
}: DraggableTaskCardProps) {
  const cardRef = useRef<HTMLDivElement>(null);
  const [isDragging, setIsDragging] = useState(false);

  const handleDragStart = useCallback(
    (e: React.DragEvent) => {
      e.dataTransfer.setData("application/x-swissarmyhammer-task", JSON.stringify(entity));
      e.dataTransfer.effectAllowed = "move";
      if (cardRef.current) setDragGhostImage(e, cardRef.current);
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
