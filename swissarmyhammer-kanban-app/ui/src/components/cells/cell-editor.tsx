import type { FieldDef } from "@/types/kanban";
import {
  resolveEditor,
  MarkdownEditor,
  SelectEditor,
  NumberEditor,
  DateEditor,
  ColorPaletteEditor,
} from "@/components/fields/editors";

interface CellEditorProps {
  field: FieldDef;
  value: unknown;
  onCommit: (value: unknown) => void;
  onCancel: () => void;
}

/**
 * Inline cell editor for grid view.
 * Dispatches on `field.editor` (via resolveEditor) to shared editor components.
 */
export function CellEditor({ field, value, onCommit, onCancel }: CellEditorProps) {
  const editor = resolveEditor(field);
  const props = { value, onCommit, onCancel, mode: "compact" as const };

  switch (editor) {
    case "select":
      return <SelectEditor {...props} field={field} />;
    case "number":
      return <NumberEditor {...props} />;
    case "date":
      return <DateEditor {...props} />;
    case "color-palette":
      return <ColorPaletteEditor {...props} />;
    case "markdown":
    default:
      return <MarkdownEditor {...props} />;
  }
}
