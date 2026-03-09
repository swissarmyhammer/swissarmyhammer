import { useState, useCallback, useMemo, useRef } from "react";
import { HexColorPicker } from "react-colorful";
import { Popover, PopoverTrigger, PopoverContent } from "@/components/ui/popover";
import { ACTOR_COLORS } from "@/lib/actor-colors";
import { EditableMarkdown } from "@/components/editable-markdown";
import { SubtaskProgress } from "@/components/subtask-progress";
import {
  resolveDisplay,
  BadgeListDisplay,
  BadgeDisplay,
  ColorSwatchDisplay,
  DateDisplay,
  NumberDisplay,
  AvatarDisplay,
  TextDisplay,
} from "@/components/fields/displays";
import {
  resolveEditor,
  MarkdownEditor,
  SelectEditor,
  NumberEditor,
  DateEditor,
  MultiSelectEditor,
} from "@/components/fields/editors";
import { useSchema } from "@/lib/schema-context";
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
  const { updateField } = useFieldUpdate();
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

  const isEditable = (field: FieldDef) => {
    // Computed tag fields are editable via tag/untag commands
    if (field.type.kind === "computed" && (field.type as Record<string, unknown>).derive === "parse-body-tags") {
      return true;
    }
    return field.type.kind !== "computed";
  };

  if (fields.length === 0) {
    return <p className="text-sm text-muted-foreground">Loading schema...</p>;
  }

  const renderField = (field: FieldDef, showLabel = true) => (
    <FieldRow
      key={field.name}
      field={field}
      value={entity.fields[field.name]}
      entity={entity}
      editing={editingField === field.name}
      editable={isEditable(field)}
      bodyFieldName={bodyFieldName}
      showLabel={showLabel}
      onEdit={() => handleEdit(field.name)}
      onCommit={(value) => handleCommit(field.name, value)}
      onCancel={handleCancel}
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
  value: unknown;
  entity: Entity;
  editing: boolean;
  editable: boolean;
  bodyFieldName?: string;
  showLabel?: boolean;
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
  bodyFieldName,
  showLabel = true,
  onEdit,
  onCommit,
  onCancel,
}: FieldRowProps) {
  return (
    <section data-testid={`field-row-${field.name}`}>
      {showLabel && (
        <h3 className="text-xs font-medium text-muted-foreground uppercase tracking-wide mb-1">
          {fieldLabel(field)}
        </h3>
      )}
      <FieldDispatch
        field={field}
        value={value}
        entity={entity}
        editing={editing && editable}
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
  bodyFieldName,
  onEdit,
  onCommit,
  onCancel,
}: {
  field: FieldDef;
  value: unknown;
  entity: Entity;
  editing: boolean;
  bodyFieldName?: string;
  onEdit: () => void;
  onCommit: (value: unknown) => void;
  onCancel: () => void;
}) {
  // Markdown fields — EditableMarkdown handles its own display/edit toggle
  // Mentions (tags, actors, etc.) are read from context automatically.
  if (field.type.kind === "markdown") {
    const text = typeof value === "string" ? value : "";
    const multiline = !field.type.single_line;
    return (
      <EditableMarkdown
        value={text}
        onCommit={(v) => onCommit(v)}
        multiline={multiline}
        className="text-sm leading-relaxed cursor-text"
        inputClassName="text-sm leading-relaxed bg-transparent w-full"
        placeholder={`Add ${field.name.replace(/_/g, " ")}...`}
      />
    );
  }

  // Computed: progress — render as SubtaskProgress bar using the body field
  if (field.type.kind === "computed" && (field.type as Record<string, unknown>).derive === "parse-body-progress") {
    const bodyText = bodyFieldName ? getStr(entity, bodyFieldName) || undefined : undefined;
    return <SubtaskProgress description={bodyText} />;
  }

  // Color fields — palette + picker (always interactive)
  if (field.type.kind === "color") {
    const hex = typeof value === "string" ? value : "888888";
    return <ColorField value={hex} onCommit={(v) => onCommit(v)} />;
  }

  // Editing: dispatch to shared editor components
  if (editing) {
    const editor = resolveEditor(field);
    const editorProps = { value, onCommit, onCancel, mode: "full" as const };

    switch (editor) {
      case "select":
        return <SelectEditor {...editorProps} field={field} />;
      case "number":
        return <NumberEditor {...editorProps} />;
      case "date":
        return <DateEditor {...editorProps} />;
      case "multi-select":
        return <MultiSelectEditor {...editorProps} field={field} entity={entity} />;
      case "markdown":
      default:
        return (
          <MarkdownEditor
            {...editorProps}
            placeholder={`Add ${field.name.replace(/_/g, " ")}...`}
          />
        );
    }
  }

  // Read-only: use shared display components in full mode
  const display = resolveDisplay(field);
  const displayProps = { field, value, entity, mode: "full" as const };

  const rendered = (() => {
    switch (display) {
      case "badge-list":
        return <BadgeListDisplay {...displayProps} />;
      case "badge":
        return <BadgeDisplay {...displayProps} />;
      case "color-swatch":
        return <ColorSwatchDisplay {...displayProps} />;
      case "date":
        return <DateDisplay {...displayProps} />;
      case "number":
        return <NumberDisplay {...displayProps} />;
      case "avatar":
        return <AvatarDisplay {...displayProps} />;
      default:
        return <TextDisplay {...displayProps} />;
    }
  })();

  return (
    <div className="text-sm cursor-text min-h-[1.25rem]" onClick={onEdit}>
      {rendered}
    </div>
  );
}

/** Palette for the color-picker grid — uses the canonical actor palette. */
const COLOR_PALETTE = ACTOR_COLORS;

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
