export type { DisplayProps } from "./text-display";
export { TextDisplay } from "./text-display";
export { BadgeListDisplay } from "./badge-list-display";
export { BadgeDisplay } from "./badge-display";
export { ColorSwatchDisplay } from "./color-swatch-display";
export { DateDisplay } from "./date-display";
export { NumberDisplay } from "./number-display";
export { MarkdownDisplay } from "./markdown-display";

import type { FieldDef } from "@/types/kanban";

/**
 * Resolve which display component to use for a field.
 * Checks `field.display` first, then falls back to `field.type.kind`.
 */
export function resolveDisplay(field: FieldDef): string {
  if (field.display) return field.display;
  const kind = field.type.kind;
  if (kind === "markdown") return "markdown";
  if (kind === "select") return "badge";
  if (kind === "color") return "color-swatch";
  if (kind === "date") return "date";
  if (kind === "number" || kind === "integer") return "number";
  return "text";
}
