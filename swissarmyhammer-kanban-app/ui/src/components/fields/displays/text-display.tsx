import type { FieldDef, Entity } from "@/types/kanban";

export interface DisplayProps {
  field: FieldDef;
  value: unknown;
  entity: Entity;
  mode: "compact" | "full";
}

/** Plain text display — truncates in compact mode. */
export function TextDisplay({ value, mode }: DisplayProps) {
  const text = typeof value === "string" ? value : value != null ? String(value) : "";
  if (!text) return <span className="text-muted-foreground/50">-</span>;
  if (mode === "compact") return <span className="truncate block">{text}</span>;
  return <span className="text-sm">{text}</span>;
}
