import { useState, useCallback } from "react";
import { FieldPlaceholder } from "@/components/fields/field-placeholder";
import type { FieldDef, Entity } from "@/types/kanban";

interface EntityInspectorProps {
  entity: Entity;
  fields: FieldDef[];
  /** Optional set of field names to hide (e.g. body_field rendered elsewhere). */
  hideFields?: string[];
  onUpdateField: (fieldName: string, value: unknown) => void;
}

/**
 * Dynamic entity inspector — iterates field definitions and renders
 * a presenter or editor for each field. Only one field can be edited at a time.
 *
 * Field type components (Cards 15-21) will replace FieldPlaceholder via
 * the dispatcher below.
 */
export function EntityInspector({
  entity,
  fields,
  hideFields,
  onUpdateField,
}: EntityInspectorProps) {
  const [editingField, setEditingField] = useState<string | null>(null);

  const handleEdit = useCallback((fieldName: string) => {
    setEditingField(fieldName);
  }, []);

  const handleCommit = useCallback(
    (fieldName: string, value: unknown) => {
      onUpdateField(fieldName, value);
      setEditingField(null);
    },
    [onUpdateField],
  );

  const handleCancel = useCallback(() => {
    setEditingField(null);
  }, []);

  const visibleFields = hideFields
    ? fields.filter((f) => !hideFields.includes(f.name))
    : fields;

  // Skip computed fields with editor: "none" from the editable list
  const isEditable = (field: FieldDef) => field.type.kind !== "computed";

  return (
    <div className="space-y-3" data-testid="entity-inspector">
      {visibleFields.map((field) => (
        <FieldRow
          key={field.name}
          field={field}
          value={entity.fields[field.name]}
          editing={editingField === field.name}
          editable={isEditable(field)}
          onEdit={() => handleEdit(field.name)}
          onCommit={(value) => handleCommit(field.name, value)}
          onCancel={handleCancel}
        />
      ))}
    </div>
  );
}

interface FieldRowProps {
  field: FieldDef;
  value: unknown;
  editing: boolean;
  editable: boolean;
  onEdit: () => void;
  onCommit: (value: unknown) => void;
  onCancel: () => void;
}

function FieldRow({
  field,
  value,
  editing,
  editable,
  onEdit,
  onCommit,
  onCancel,
}: FieldRowProps) {
  return (
    <section data-testid={`field-row-${field.name}`}>
      <h3 className="text-xs font-medium text-muted-foreground uppercase tracking-wide mb-1">
        {fieldLabel(field)}
      </h3>
      <FieldDispatch
        field={field}
        value={value}
        editing={editing && editable}
        onEdit={onEdit}
        onCommit={onCommit}
        onCancel={onCancel}
      />
    </section>
  );
}

/**
 * Dispatch to the correct field component based on FieldType kind.
 *
 * Currently all types use FieldPlaceholder. As Cards 15-21 are implemented,
 * each case will be replaced with the real Presenter/Editor component.
 */
function FieldDispatch({
  field,
  value,
  editing,
  onEdit,
  onCommit,
  onCancel,
}: {
  field: FieldDef;
  value: unknown;
  editing: boolean;
  onEdit: () => void;
  onCommit: (value: unknown) => void;
  onCancel: () => void;
}) {
  // Future: switch on field.type.kind to dispatch to specific components
  // e.g. case "text": return <TextPresenter/TextEditor .../>
  //      case "markdown": return <MarkdownPresenter/MarkdownEditor .../>
  return (
    <FieldPlaceholder
      field={field}
      value={value}
      editing={editing}
      onEdit={onEdit}
      onCommit={onCommit}
      onCancel={onCancel}
    />
  );
}

/** Convert field name to a human-readable label. */
function fieldLabel(field: FieldDef): string {
  return field.name.replace(/_/g, " ");
}
