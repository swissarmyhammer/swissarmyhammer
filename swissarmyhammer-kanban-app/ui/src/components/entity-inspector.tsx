import { useState, useCallback, useMemo, useRef } from "react";
import { HexColorPicker } from "react-colorful";
import { Popover, PopoverTrigger, PopoverContent } from "@/components/ui/popover";
import { EditableMarkdown } from "@/components/editable-markdown";
import { FieldPlaceholder } from "@/components/fields/field-placeholder";
import { SubtaskProgress } from "@/components/subtask-progress";
import { TagPill } from "@/components/tag-pill";
import { useSchema } from "@/lib/schema-context";
import { useEntityStore } from "@/lib/entity-store-context";
import { useFieldUpdate } from "@/lib/field-update-context";
import type { FieldDef, Entity } from "@/types/kanban";
import { getStr } from "@/types/kanban";

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
 * - Tag entities from EntityStoreContext (for body_field markdown decorations)
 * - Save function from FieldUpdateContext
 */
export function EntityInspector({ entity }: EntityInspectorProps) {
  const [editingField, setEditingField] = useState<string | null>(null);
  const { getSchema } = useSchema();
  const { getEntities } = useEntityStore();
  const { updateField } = useFieldUpdate();
  const tags = getEntities("tag");
  const schema = getSchema(entity.entity_type);
  const bodyFieldName = schema?.entity.body_field;
  const fields = schema?.fields ?? [];

  const sections = useMemo(() => {
    const header: FieldDef[] = [];
    const body: FieldDef[] = [];
    const footer: FieldDef[] = [];
    // Fields are already in entity definition order from the schema
    for (const field of fields) {
      const section = field.section ?? "body";
      if (section === "hidden") continue;
      if (section === "header") header.push(field);
      else if (section === "footer") footer.push(field);
      else body.push(field);
    }
    return { header, body, footer };
  }, [fields]);

  const handleEdit = useCallback((fieldName: string) => {
    setEditingField(fieldName);
  }, []);

  const handleCommit = useCallback(
    (fieldName: string, value: unknown) => {
      updateField(entity.entity_type, entity.id, fieldName, value).catch(() => {});
      setEditingField(null);
    },
    [updateField, entity.entity_type, entity.id],
  );

  const handleCancel = useCallback(() => {
    setEditingField(null);
  }, []);

  const isEditable = (field: FieldDef) => field.type.kind !== "computed";

  if (fields.length === 0) {
    return <p className="text-sm text-muted-foreground">Loading schema...</p>;
  }

  const renderField = (field: FieldDef) => (
    <FieldRow
      key={field.name}
      field={field}
      value={entity.fields[field.name]}
      entity={entity}
      editing={editingField === field.name}
      editable={isEditable(field)}
      isBodyField={field.name === bodyFieldName}
      tags={tags}
      bodyFieldName={bodyFieldName}
      onEdit={() => handleEdit(field.name)}
      onCommit={(value) => handleCommit(field.name, value)}
      onCancel={handleCancel}
    />
  );

  return (
    <div data-testid="entity-inspector">
      {sections.header.length > 0 && (
        <div className="space-y-2" data-testid="inspector-header">
          {sections.header.map(renderField)}
        </div>
      )}
      {sections.header.length > 0 && sections.body.length > 0 && (
        <div className="my-3 h-px bg-border" />
      )}
      {sections.body.length > 0 && (
        <div className="space-y-3" data-testid="inspector-body">
          {sections.body.map(renderField)}
        </div>
      )}
      {sections.footer.length > 0 && (
        <>
          <div className="my-3 h-px bg-border" />
          <div className="space-y-3" data-testid="inspector-footer">
            {sections.footer.map(renderField)}
          </div>
        </>
      )}
    </div>
  );
}

interface FieldRowProps {
  field: FieldDef;
  value: unknown;
  entity: Entity;
  editing: boolean;
  editable: boolean;
  isBodyField?: boolean;
  tags: Entity[];
  bodyFieldName?: string;
  onEdit: () => void;
  onCommit: (value: unknown) => void;
  onCancel: () => void;
}

function FieldRow({
  field,
  value,
  entity,
  editing,
  editable,
  isBodyField,
  tags,
  bodyFieldName,
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
        entity={entity}
        editing={editing && editable}
        isBodyField={isBodyField}
        tags={tags}
        bodyFieldName={bodyFieldName}
        onEdit={onEdit}
        onCommit={onCommit}
        onCancel={onCancel}
      />
    </section>
  );
}

function FieldDispatch({
  field,
  value,
  entity,
  editing,
  isBodyField,
  tags,
  bodyFieldName,
  onEdit,
  onCommit,
  onCancel,
}: {
  field: FieldDef;
  value: unknown;
  entity: Entity;
  editing: boolean;
  isBodyField?: boolean;
  tags: Entity[];
  bodyFieldName?: string;
  onEdit: () => void;
  onCommit: (value: unknown) => void;
  onCancel: () => void;
}) {
  // Markdown fields — EditableMarkdown with optional tag decorations
  if (field.type.kind === "markdown") {
    const text = typeof value === "string" ? value : "";
    const multiline = !field.type.single_line;
    return (
      <EditableMarkdown
        value={text}
        onCommit={(v) => onCommit(v)}
        multiline={multiline}
        tags={isBodyField ? tags : undefined}
        className="text-sm leading-relaxed cursor-text"
        inputClassName="text-sm leading-relaxed bg-transparent w-full"
        placeholder={`Add ${field.name.replace(/_/g, " ")}...`}
      />
    );
  }

  // Computed: tags — render as pill list
  if (field.display === "badge-list" && field.type.kind === "computed") {
    const slugs = Array.isArray(value) ? (value as string[]) : [];
    if (slugs.length === 0) return <span className="text-sm text-muted-foreground italic">None</span>;
    return (
      <div className="flex flex-wrap gap-1">
        {slugs.map((slug) => (
          <TagPill key={slug} slug={slug} tags={tags} taskId={entity.id} />
        ))}
      </div>
    );
  }

  // Computed: progress — render as SubtaskProgress bar using the body field
  if (field.type.kind === "computed" && (field.type as Record<string, unknown>).derive === "parse-body-progress") {
    const bodyText = bodyFieldName ? getStr(entity, bodyFieldName) || undefined : undefined;
    return <SubtaskProgress description={bodyText} />;
  }

  // Color fields — palette + picker
  if (field.type.kind === "color") {
    const hex = typeof value === "string" ? value : "888888";
    return <ColorField value={hex} onCommit={(v) => onCommit(v)} />;
  }

  // Default fallback
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

/** 16-color palette matching Rust auto_color */
const COLOR_PALETTE = [
  "d73a4a", "e36209", "f9c513", "0e8a16",
  "006b75", "1d76db", "0075ca", "5319e7",
  "b60205", "d93f0b", "fbca04", "0e8a16",
  "006b75", "1d76db", "6f42c1", "e4e669",
];

function ColorField({ value, onCommit }: { value: string; onCommit: (v: string) => void }) {
  const [selected, setSelected] = useState(value);
  const [pickerOpen, setPickerOpen] = useState(false);
  const timerRef = useRef<ReturnType<typeof setTimeout>>(undefined);

  const saveDebounced = useCallback(
    (color: string) => {
      clearTimeout(timerRef.current);
      timerRef.current = setTimeout(() => onCommit(color), 150);
    },
    [onCommit],
  );

  return (
    <div className="flex items-start gap-2">
      <div className="grid grid-cols-8 gap-1 flex-1">
        {COLOR_PALETTE.map((color, i) => (
          <button
            key={`${color}-${i}`}
            type="button"
            className={`w-6 h-6 rounded-full border-2 transition-all ${
              selected === color
                ? "border-foreground scale-110"
                : "border-transparent hover:border-muted-foreground/50"
            }`}
            style={{ backgroundColor: `#${color}` }}
            onClick={() => { setSelected(color); onCommit(color); }}
          />
        ))}
      </div>
      <Popover open={pickerOpen} onOpenChange={setPickerOpen}>
        <PopoverTrigger asChild>
          <button
            type="button"
            className="shrink-0 w-8 h-8 rounded-md border border-input cursor-pointer"
            style={{ backgroundColor: `#${selected}` }}
          />
        </PopoverTrigger>
        <PopoverContent align="end" className="w-auto p-3">
          <HexColorPicker
            color={`#${selected}`}
            onChange={(hex) => {
              const c = hex.replace("#", "");
              setSelected(c);
              saveDebounced(c);
            }}
          />
          <div className="mt-2 flex items-center gap-2">
            <span className="text-xs text-muted-foreground">#</span>
            <input
              type="text"
              value={selected}
              onChange={(e) => {
                const v = e.target.value.replace(/[^0-9a-fA-F]/g, "").slice(0, 6);
                setSelected(v);
                if (v.length === 6) saveDebounced(v);
              }}
              className="flex-1 text-xs font-mono bg-transparent border border-input rounded px-1.5 py-0.5"
              maxLength={6}
            />
          </div>
        </PopoverContent>
      </Popover>
    </div>
  );
}

function fieldLabel(field: FieldDef): string {
  return field.name.replace(/_/g, " ");
}
