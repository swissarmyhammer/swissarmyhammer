export type { EditorProps } from "./markdown-editor";
export { MarkdownEditor } from "./markdown-editor";
export { SelectEditor } from "./select-editor";
export { NumberEditor } from "./number-editor";
export { DateEditor } from "./date-editor";
export { ColorPaletteEditor } from "./color-palette-editor";
export { MultiSelectEditor } from "./multi-select-editor";

import type { FieldDef } from "@/types/kanban";

/**
 * Resolve which editor component to use for a field.
 * Checks `field.editor` first, then falls back to `field.type.kind`.
 */
export function resolveEditor(field: FieldDef): string {
  if (field.editor) return field.editor;
  const kind = field.type.kind;
  if (kind === "markdown") return "markdown";
  if (kind === "select") return "select";
  if (kind === "color") return "color-palette";
  if (kind === "date") return "date";
  if (kind === "number" || kind === "integer") return "number";
  if (kind === "reference") return "multi-select";
  return "markdown"; // default: CM6 text editor
}
