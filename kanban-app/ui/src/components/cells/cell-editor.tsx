import { useCallback, useRef, useState } from "react";
import type { FieldDef, Entity } from "@/types/kanban";
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
  entity: Entity;
  /** Entity type for direct save by editors. */
  entityType?: string;
  /** Entity ID for direct save by editors. */
  entityId?: string;
  /** Field name for direct save by editors. */
  fieldName?: string;
  onCommit: (value: unknown) => void;
  onCancel: () => void;
}

/**
 * Inline cell editor for grid view.
 * Dispatches on `field.editor` (via resolveEditor) to shared editor components.
 * Passes entity identity through so editors can save themselves.
 */
export function CellEditor({ field, value, entity, entityType, entityId, fieldName, onCommit, onCancel }: CellEditorProps) {
  const editor = resolveEditor(field);
  const props = { value, entityType, entityId, fieldName, onCommit, onCancel, mode: "compact" as const };

  switch (editor) {
    case "none":
      // Non-editable field — exit immediately
      onCancel();
      return null;
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
        <MultiSelectPopover field={field} entity={entity} value={value} entityType={entityType} entityId={entityId} fieldName={fieldName} onCommit={onCommit} onCancel={onCancel} />
      );
    case "markdown":
    default:
      // Pass onCommit as onSubmit so Enter commits + exits edit mode
      return <MarkdownEditor {...props} onSubmit={onCommit} />;
  }
}

/**
 * Wraps MultiSelectEditor in a Popover that opens automatically when the cell
 * enters edit mode. Closing the popover (Escape or click-outside) commits
 * the current selection and exits edit mode.
 */
function MultiSelectPopover({ field, entity, value, entityType, entityId, fieldName, onCommit, onCancel }: CellEditorProps & { field: FieldDef }) {
  const [open, setOpen] = useState(true);
  const committedRef = useRef(false);

  const handleOpenChange = useCallback(
    (nextOpen: boolean) => {
      if (!nextOpen && !committedRef.current) {
        // Popover closed without an explicit commit (e.g. click-outside).
        // Exit edit mode — the MultiSelectEditor's commit will have already
        // fired via its blur/Escape handler if data was changed.
        onCancel();
      }
      setOpen(nextOpen);
    },
    [onCancel],
  );

  const handleCommit = useCallback(
    (val: unknown) => {
      committedRef.current = true;
      setOpen(false);
      onCommit(val);
    },
    [onCommit],
  );

  const handleCancel = useCallback(() => {
    committedRef.current = true;
    setOpen(false);
    onCancel();
  }, [onCancel]);

  return (
    <Popover open={open} onOpenChange={handleOpenChange}>
      <PopoverTrigger asChild>
        <div className="w-full h-full min-h-[1.5rem]" />
      </PopoverTrigger>
      <PopoverContent
        align="start"
        sideOffset={2}
        className="w-[320px] p-0"
        onOpenAutoFocus={(e) => {
          e.preventDefault();
        }}
        onEscapeKeyDown={(e) => {
          // Prevent Radix from closing the popover on Escape —
          // let the CM6 keymap handle it via commit() first.
          e.preventDefault();
        }}
      >
        <MultiSelectEditor
          field={field}
          entity={entity}
          value={value}
          entityType={entityType}
          entityId={entityId}
          fieldName={fieldName}
          onCommit={handleCommit}
          onCancel={handleCancel}
          mode="compact"
        />
      </PopoverContent>
    </Popover>
  );
}
