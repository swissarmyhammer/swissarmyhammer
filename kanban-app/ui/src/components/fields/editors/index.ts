export type { EditorProps } from "./markdown-editor";
export { MarkdownEditor } from "./markdown-editor";
export { SelectEditor } from "./select-editor";
export { NumberEditor } from "./number-editor";
export { DateEditor } from "./date-editor";
export { ColorPaletteEditor } from "./color-palette-editor";
export { MultiSelectEditor } from "./multi-select-editor";

import type { FieldDef } from "@/types/kanban";

/** Resolve which editor component to use for a field — reads directly from the YAML-configured `editor` property. */
export function resolveEditor(field: FieldDef): string {
  return field.editor ?? "none";
}
