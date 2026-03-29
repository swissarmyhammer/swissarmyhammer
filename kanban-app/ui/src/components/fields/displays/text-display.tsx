import type { FieldDisplayProps } from "../field";
import type { Entity } from "@/types/kanban";

/**
 * Display props with entity guaranteed present.
 *
 * Derived from FieldDisplayProps (the registry contract) with `entity`
 * narrowed to non-optional. Field always passes the entity, so display
 * components can rely on it without null checks.
 */
export type DisplayProps = Omit<FieldDisplayProps, "entity"> & {
  entity: Entity;
};

/** Plain text display — truncates in compact mode. */
export function TextDisplay({ value, mode }: DisplayProps) {
  const text =
    typeof value === "string" ? value : value != null ? String(value) : "";
  if (!text) return <span className="text-muted-foreground/50">-</span>;
  if (mode === "compact") return <span className="truncate block">{text}</span>;
  return <span className="text-sm">{text}</span>;
}
