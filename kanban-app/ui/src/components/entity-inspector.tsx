import { useState, useCallback, useMemo } from "react";
import { Tooltip, TooltipTrigger, TooltipContent } from "@/components/ui/tooltip";
import { CellDispatch } from "@/components/cells";
import {
  resolveEditor,
  MarkdownEditor,
  SelectEditor,
  NumberEditor,
  DateEditor,
  ColorPaletteEditor,
  MultiSelectEditor,
} from "@/components/fields/editors";
import { useSchema } from "@/lib/schema-context";
import type { FieldDef, Entity } from "@/types/kanban";
import { icons, HelpCircle } from "lucide-react";
import type { LucideIcon } from "lucide-react";

/** Convert kebab-case icon name (e.g. "file-text") to PascalCase key (e.g. "FileText"). */
function kebabToPascal(s: string): string {
  return s.replace(/(^|-)([a-z])/g, (_, _dash, c) => c.toUpperCase());
}

/** Resolve the lucide icon component from a field's `icon` property. */
function fieldIcon(field: FieldDef): LucideIcon {
  if (field.icon) {
    const key = kebabToPascal(field.icon);
    const Icon = icons[key as keyof typeof icons];
    if (Icon) return Icon;
  }
  return HelpCircle;
}

interface EntityInspectorProps {
  entity: Entity;
}

/**
 * Generic entity inspector — renders all fields for any entity type,
 * grouped by section (header, body, footer) in entity definition order.
 *
 * Fields with `section: "hidden"` are not rendered.
 * Fields default to "body" if no section is specified.
 *
 * Pulls everything from context:
 * - Field definitions and ordering from SchemaContext
 * - Save function from FieldUpdateContext (used internally by FieldRow)
 */
export function EntityInspector({ entity }: EntityInspectorProps) {
  const { getSchema } = useSchema();
  const schema = getSchema(entity.entity_type);
  const fields = schema?.fields ?? [];

  const sections = useMemo(() => {
    const header: FieldDef[] = [];
    const body: FieldDef[] = [];
    const footer: FieldDef[] = [];
    for (const field of fields) {
      const section = field.section ?? "body";
      if (section === "hidden") continue;
      if (section === "header") header.push(field);
      else if (section === "footer") footer.push(field);
      else body.push(field);
    }
    return { header, body, footer };
  }, [fields]);

  if (fields.length === 0) {
    return <p className="text-sm text-muted-foreground">Loading schema...</p>;
  }

  const renderField = (field: FieldDef, showLabel = true) => (
    <FieldRow
      key={field.name}
      field={field}
      entity={entity}
      showLabel={showLabel}
    />
  );

  return (
    <div data-testid="entity-inspector">
      {sections.header.length > 0 && (
        <div className="space-y-2" data-testid="inspector-header">
          {sections.header.map((f) => renderField(f, false))}
        </div>
      )}
      {sections.header.length > 0 && sections.body.length > 0 && (
        <div className="my-3 h-px bg-border" />
      )}
      {sections.body.length > 0 && (
        <div className="space-y-3" data-testid="inspector-body">
          {sections.body.map((f) => renderField(f))}
        </div>
      )}
      {sections.footer.length > 0 && (
        <>
          <div className="my-3 h-px bg-border" />
          <div className="space-y-3" data-testid="inspector-footer">
            {sections.footer.map((f) => renderField(f))}
          </div>
        </>
      )}
    </div>
  );
}

interface FieldRowProps {
  field: FieldDef;
  entity: Entity;
  showLabel?: boolean;
}

/**
 * A single field row in the inspector. Manages editing state.
 * Editors save themselves via useFieldUpdate — FieldRow only handles lifecycle.
 */
function FieldRow({
  field,
  entity,
  showLabel = true,
}: FieldRowProps) {
  const [editing, setEditing] = useState(false);

  const editable = isEditable(field);

  const handleEdit = useCallback(() => {
    if (editable) setEditing(true);
  }, [editable]);

  const handleCommit = useCallback(() => {
    setEditing(false);
  }, []);

  const handleCancel = useCallback(() => {
    setEditing(false);
  }, []);

  const content = (
    <FieldDispatch
      field={field}
      value={entity.fields[field.name]}
      entity={entity}
      editing={editing && editable}
      onEdit={handleEdit}
      onCommit={handleCommit}
      onCancel={handleCancel}
    />
  );

  const Icon = field.icon ? fieldIcon(field) : null;
  const tip = field.description || fieldLabel(field);

  if (!showLabel && !Icon) {
    return <section data-testid={`field-row-${field.name}`}>{content}</section>;
  }

  return (
    <section data-testid={`field-row-${field.name}`} className="flex items-start gap-2">
      {Icon && (
        <Tooltip>
          <TooltipTrigger asChild>
            <span className="mt-0.5 shrink-0 text-muted-foreground">
              <Icon size={14} />
            </span>
          </TooltipTrigger>
          <TooltipContent side="left" align="start">
            {tip}
          </TooltipContent>
        </Tooltip>
      )}
      <div className="flex-1 min-w-0">{content}</div>
    </section>
  );
}

/** Check if a field is editable in the inspector — driven by the field's editor property. */
function isEditable(field: FieldDef): boolean {
  return resolveEditor(field) !== "none";
}

/**
 * Dispatch to editor or display based on the field's configured `editor` and `display`
 * properties. No type-kind special cases — the YAML field definitions are the source of truth.
 */
function FieldDispatch({
  field,
  value,
  entity,
  editing,
  onEdit,
  onCommit,
  onCancel,
}: {
  field: FieldDef;
  value: unknown;
  entity: Entity;
  editing: boolean;
  onEdit: () => void;
  onCommit: (value: unknown) => void;
  onCancel: () => void;
}) {
  // Editing: dispatch to editor by field.editor
  // Editors save themselves via useFieldUpdate — entity identity is passed through.
  if (editing) {
    const editor = resolveEditor(field);
    const editorProps = {
      value,
      entityType: entity.entity_type,
      entityId: entity.id,
      fieldName: field.name,
      onCommit,
      onCancel,
      mode: "full" as const,
    };

    switch (editor) {
      case "select":
        return <SelectEditor {...editorProps} field={field} />;
      case "number":
        return <NumberEditor {...editorProps} />;
      case "date":
        return <DateEditor {...editorProps} />;
      case "color-palette":
        return <ColorPaletteEditor {...editorProps} />;
      case "multi-select":
        return <MultiSelectEditor {...editorProps} field={field} entity={entity} />;
      case "markdown":
      default:
        return (
          <MarkdownEditor
            {...editorProps}
            initialEditing
            placeholder={`Add ${field.name.replace(/_/g, " ")}...`}
          />
        );
    }
  }

  // Read-only: dispatch to display by field.display via CellDispatch
  if (isEmpty(value)) {
    return (
      <div className="text-sm cursor-text min-h-[1.25rem] text-muted-foreground/50 italic" onClick={onEdit}>
        {fieldLabel(field)}
      </div>
    );
  }
  return (
    <div className="text-sm cursor-text min-h-[1.25rem]" onClick={onEdit}>
      <CellDispatch field={field} value={value} entity={entity} mode="full" />
    </div>
  );
}

function fieldLabel(field: FieldDef): string {
  return field.name.replace(/_/g, " ");
}

function isEmpty(value: unknown): boolean {
  if (value == null) return true;
  if (value === "") return true;
  if (Array.isArray(value) && value.length === 0) return true;
  return false;
}
