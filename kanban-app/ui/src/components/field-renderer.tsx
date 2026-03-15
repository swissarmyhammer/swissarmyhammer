import { useCallback, useState } from "react";
import { FieldPlaceholder } from "@/components/fields/field-placeholder";
import { Tooltip, TooltipTrigger, TooltipContent } from "@/components/ui/tooltip";
import { useFieldUpdate } from "@/lib/field-update-context";
import type { Entity, FieldDef } from "@/types/kanban";

interface FieldRendererProps {
  entity: Entity;
  field: FieldDef;
  /** Render inline (no label). Default: false */
  inline?: boolean;
  className?: string;
}

/**
 * Generic field renderer — the building block for all views (board, grid, inspector).
 *
 * Renders the field value using `fieldDef.display` for read mode and
 * `fieldDef.editor` for edit mode. Persists changes via `useFieldUpdate`
 * internally — no `onCommit` wiring needed.
 *
 * Usage:
 * ```tsx
 * <FieldRenderer entity={task} field={titleField} />
 * ```
 */
export function FieldRenderer({
  entity,
  field,
  inline,
  className,
}: FieldRendererProps) {
  const [editing, setEditing] = useState(false);
  const { updateField } = useFieldUpdate();
  const value = entity.fields[field.name];
  const editable = field.editor !== "none" && field.type.kind !== "computed";

  const handleEdit = useCallback(() => {
    if (editable) setEditing(true);
  }, [editable]);

  const handleCommit = useCallback(
    (v: unknown) => {
      setEditing(false);
      updateField(entity.entity_type, entity.id, field.name, v).catch(() => {});
    },
    [updateField, entity.entity_type, entity.id, field.name],
  );

  const handleCancel = useCallback(() => {
    setEditing(false);
  }, []);

  const content = (
    <FieldPlaceholder
      field={field}
      value={value}
      editing={editing}
      onEdit={handleEdit}
      onCommit={handleCommit}
      onCancel={handleCancel}
    />
  );

  if (inline) {
    return <div className={className}>{content}</div>;
  }

  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <section className={className} data-testid={`field-renderer-${field.name}`}>
          {content}
        </section>
      </TooltipTrigger>
      <TooltipContent side="top" align="start">
        {fieldLabel(field)}
      </TooltipContent>
    </Tooltip>
  );
}

/** Convert field name to a human-readable label. */
function fieldLabel(field: FieldDef): string {
  return field.name.replace(/_/g, " ");
}
