import { useState, useCallback, useMemo } from "react";
import { Tooltip, TooltipTrigger, TooltipContent } from "@/components/ui/tooltip";
import { resolveEditor } from "@/components/fields/editors";
import { Field } from "@/components/fields/field";
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
 * Field handles data binding, save, and display/editor dispatch.
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

  const handleDone = useCallback(() => {
    setEditing(false);
  }, []);

  const handleCancel = useCallback(() => {
    setEditing(false);
  }, []);

  const content = (
    <Field
      fieldDef={field}
      entityType={entity.entity_type}
      entityId={entity.id}
      mode="full"
      editing={editing && editable}
      onEdit={handleEdit}
      onDone={handleDone}
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

function fieldLabel(field: FieldDef): string {
  return field.name.replace(/_/g, " ");
}
