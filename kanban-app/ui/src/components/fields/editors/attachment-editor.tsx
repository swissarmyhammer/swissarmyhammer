/**
 * Attachment editor component — renders existing attachments with remove
 * buttons and an "Add file" button that opens a Tauri file dialog.
 *
 * The editor manages the attachment list purely through onChange/onCommit
 * callbacks. It never touches persistence directly — the entity layer
 * handles the actual file copy on save.
 */

import { useCallback, useEffect, useRef } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { X, Plus } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  AttachmentItem,
  type AttachmentMeta,
} from "@/components/fields/displays/attachment-display";
import { useFileDrop } from "@/lib/file-drop-context";
import type { FieldDef } from "@/types/kanban";
import type { EditorProps } from ".";

interface AttachmentEditorProps extends EditorProps {
  field: FieldDef;
}

/**
 * Check whether a single value is a valid attachment element.
 *
 * Valid elements are either strings (file paths) or objects with a
 * string `id` property (AttachmentMeta).
 */
function isValidElement(v: unknown): v is AttachmentMeta | string {
  if (typeof v === "string") return true;
  if (
    v != null &&
    typeof v === "object" &&
    "id" in v &&
    typeof (v as Record<string, unknown>).id === "string"
  )
    return true;
  return false;
}

/**
 * Normalize the value prop into an array of attachments/paths.
 *
 * The value can be:
 * - An array of AttachmentMeta objects (existing attachments)
 * - An array containing a mix of AttachmentMeta and string paths (new picks)
 * - null/undefined (empty)
 *
 * Invalid elements (e.g. numbers, objects without an `id`) are silently
 * filtered out so downstream code never receives unexpected shapes.
 */
function normalizeAttachments(value: unknown): Array<AttachmentMeta | string> {
  if (Array.isArray(value)) return value.filter(isValidElement);
  if (isValidElement(value)) return [value];
  return [];
}

/**
 * Returns whether the field supports multiple attachments.
 *
 * Checks the `multiple` property on `field.type` using proper type
 * narrowing instead of an unsafe cast. Defaults to true when the
 * property is absent.
 *
 * @param field - The field definition
 * @returns true if multiple attachments are allowed
 */
function isMultiple(field: FieldDef): boolean {
  const { type } = field;
  if (type && typeof type === "object" && "multiple" in type) {
    return (type as unknown as { multiple: boolean }).multiple !== false;
  }
  return true;
}

/**
 * Editor for attachment fields. Shows existing attachments with remove buttons
 * and an "Add file" button that opens the native file picker.
 */
export function AttachmentEditor({
  field,
  value,
  onChange,
}: AttachmentEditorProps) {
  const attachments = normalizeAttachments(value);
  const multiple = isMultiple(field);
  const { isDragging, registerDropTarget, unregisterDropTarget } =
    useFileDrop();

  // Keep a ref to the latest attachments+onChange so the drop callback
  // always sees current values without re-registering.
  const stateRef = useRef({ attachments, onChange });
  stateRef.current = { attachments, onChange };

  // Register as the active drop target while mounted
  useEffect(() => {
    const callback = (paths: string[]) => {
      const { attachments: current, onChange: fire } = stateRef.current;
      const next = [...current, ...paths];
      fire?.(next);
    };
    registerDropTarget(callback);
    return () => unregisterDropTarget(callback);
  }, [registerDropTarget, unregisterDropTarget]);

  const handleRemove = useCallback(
    (index: number) => {
      const next = attachments.filter((_, i) => i !== index);
      onChange?.(next);
    },
    [attachments, onChange],
  );

  const handleAdd = useCallback(async () => {
    const result = await open({ multiple, directory: false });
    if (result == null) return;

    // open() returns string | string[] | null depending on multiple flag
    const paths = Array.isArray(result) ? result : [result];
    const next = [...attachments, ...paths];
    onChange?.(next);
  }, [attachments, multiple, onChange]);

  return (
    <div
      data-file-drop-zone
      className={`flex flex-col gap-1.5 w-full rounded transition-all ${
        isDragging ? "ring-2 ring-primary bg-primary/5" : ""
      }`}
    >
      {attachments.length === 0 && (
        <span className="text-sm text-muted-foreground italic">
          No attachments
        </span>
      )}

      {attachments.map((att, index) => (
        <div
          key={typeof att === "string" ? att : att.id}
          className="flex items-center gap-1 group"
        >
          <div className="flex-1 min-w-0">
            {typeof att === "string" ? (
              <span className="text-sm truncate block">{att}</span>
            ) : (
              <AttachmentItem attachment={att} />
            )}
          </div>
          <Button
            variant="ghost"
            size="icon"
            className="h-5 w-5 shrink-0 opacity-50 hover:opacity-100"
            aria-label={`Remove ${typeof att === "string" ? att : att.name}`}
            onClick={() => handleRemove(index)}
          >
            <X size={12} />
          </Button>
        </div>
      ))}

      <Button
        variant="outline"
        size="sm"
        className="w-full text-xs gap-1"
        onClick={handleAdd}
      >
        <Plus size={12} />
        Add file
      </Button>
    </div>
  );
}
