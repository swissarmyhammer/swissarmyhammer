import { useCallback, useState } from "react";
import type { FieldDef } from "@/types/kanban";
import {
  resolveEditor,
  MarkdownEditor,
  SelectEditor,
  NumberEditor,
  DateEditor,
  ColorPaletteEditor,
  MultiSelectEditor,
} from "@/components/fields/editors";
import {
  Popover,
  PopoverTrigger,
  PopoverContent,
} from "@/components/ui/popover";

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
    case "multi-select":
      return (
        <MultiSelectPopover field={field} value={value} onCommit={onCommit} onCancel={onCancel} />
      );
    case "markdown":
    default:
      return <MarkdownEditor {...props} />;
  }
}

/**
 * Wraps MultiSelectEditor in a Popover that opens automatically when the cell
 * enters edit mode. Closing the popover (Escape or click-outside) commits
 * the current selection and exits edit mode.
 */
function MultiSelectPopover({ field, value, onCommit, onCancel }: CellEditorProps & { field: FieldDef }) {
  const [open, setOpen] = useState(true);

  const handleOpenChange = useCallback(
    (nextOpen: boolean) => {
      if (!nextOpen) {
        // Popover is closing — let the editor's own commit handle persisting;
        // we just need to exit edit mode.
        onCancel();
      }
      setOpen(nextOpen);
    },
    [onCancel],
  );

  const handleCommit = useCallback(
    (val: unknown) => {
      setOpen(false);
      onCommit(val);
    },
    [onCommit],
  );

  return (
    <Popover open={open} onOpenChange={handleOpenChange}>
      <PopoverTrigger asChild>
        {/* Invisible anchor occupying the cell so the popover positions correctly */}
        <div className="w-full h-full min-h-[1.5rem]" />
      </PopoverTrigger>
      <PopoverContent
        align="start"
        sideOffset={2}
        className="w-[320px] p-0"
        onOpenAutoFocus={(e) => {
          // Let the MultiSelectEditor manage its own focus
          e.preventDefault();
        }}
      >
        <MultiSelectEditor
          field={field}
          value={value}
          onCommit={handleCommit}
          onCancel={onCancel}
          mode="compact"
        />
      </PopoverContent>
    </Popover>
  );
}
