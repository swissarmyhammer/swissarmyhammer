export type { DisplayProps } from "./text-display";
export { TextDisplay } from "./text-display";
export { BadgeListDisplay } from "./badge-list-display";
export { BadgeDisplay } from "./badge-display";
export { ColorSwatchDisplay } from "./color-swatch-display";
export { DateDisplay } from "./date-display";
export { NumberDisplay } from "./number-display";
export { MarkdownDisplay } from "./markdown-display";
export { AvatarDisplay } from "./avatar-display";

import type { FieldDef } from "@/types/kanban";

/** Resolve which display component to use for a field — reads directly from the YAML-configured `display` property. */
export function resolveDisplay(field: FieldDef): string {
  return field.display ?? "text";
}
