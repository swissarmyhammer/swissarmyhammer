import { forwardRef, useCallback, useMemo } from "react";
import { GripVertical, Info } from "lucide-react";
import { EditableMarkdown } from "@/components/editable-markdown";
import { SubtaskProgress } from "@/components/subtask-progress";
import { BadgeListDisplay } from "@/components/fields/displays/badge-list-display";
import { FocusScope } from "@/components/focus-scope";
import { useFieldUpdate } from "@/lib/field-update-context";
import { useSchema } from "@/lib/schema-context";
import { useInspect } from "@/lib/inspect-context";
import { useEntityCommands } from "@/lib/entity-commands";
import { moniker } from "@/lib/moniker";
import type { Entity, FieldDef } from "@/types/kanban";
import { getStr } from "@/types/kanban";

interface EntityCardProps {
  entity: Entity;
  dragHandleProps?: Record<string, unknown>;
  style?: React.CSSProperties;
  draggable?: boolean;
  onDragStart?: (e: React.DragEvent) => void;
  onDragEnd?: (e: React.DragEvent) => void;
}

/**
 * Compact card view for an entity on the board.
 *
 * Renders header-section fields from the schema using the same
 * dispatch logic as EntityInspector — no hardcoded field names.
 */
export const EntityCard = forwardRef<HTMLDivElement, EntityCardProps>(
  function EntityCard(
    {
      entity,
      dragHandleProps,
      style,
      draggable,
      onDragStart,
      onDragEnd,
      ...rest
    },
    ref,
  ) {
    const { updateField } = useFieldUpdate();
    const { getSchema } = useSchema();
    const inspectEntity = useInspect();
    const schema = getSchema(entity.entity_type);
    const bodyFieldName = schema?.entity?.body_field;

    const entityMoniker = moniker(entity.entity_type, entity.id);

    const cardFields = useMemo(
      () => (schema?.fields ?? []).filter((f) => f.section === "header"),
      [schema],
    );

    const handleCommit = useCallback(
      (fieldName: string, value: unknown) => {
        updateField(entity.entity_type, entity.id, fieldName, value).catch(
          () => {},
        );
      },
      [updateField, entity.entity_type, entity.id],
    );

    const commands = useEntityCommands(entity.entity_type, entity.id, entity);

    return (
      <FocusScope moniker={entityMoniker} commands={commands}>
        <div
          ref={ref}
          style={style}
          data-entity-card={entity.id}
          draggable={draggable}
          onDragStart={onDragStart}
          onDragEnd={onDragEnd}
          className="rounded-md bg-card px-3 py-2 text-sm border border-border hover:ring-1 hover:ring-ring transition-shadow relative group flex items-start gap-2 overflow-hidden"
          {...rest}
        >
          <button
            type="button"
            className="shrink-0 mt-0.5 p-0 text-muted-foreground/50 hover:text-muted-foreground cursor-grab active:cursor-grabbing touch-none"
            onClick={(e) => e.stopPropagation()}
            {...dragHandleProps}
          >
            <GripVertical className="h-4 w-4" />
          </button>
          <div className="flex-1 min-w-0 break-words">
            {cardFields.map((field) => (
              <CardFieldDispatch
                key={field.name}
                field={field}
                value={entity.fields[field.name]}
                entity={entity}
                bodyFieldName={bodyFieldName}
                onCommit={(value) => handleCommit(field.name, value)}
              />
            ))}
          </div>
          <button
            type="button"
            className="shrink-0 mt-0.5 p-0.5 rounded text-muted-foreground/50 hover:text-muted-foreground hover:bg-muted transition-colors"
            onClick={(e) => {
              e.stopPropagation();
              inspectEntity(entityMoniker);
            }}
            title="Inspect"
          >
            <Info className="h-3.5 w-3.5" />
          </button>
        </div>
      </FocusScope>
    );
  },
);

/**
 * Compact field renderer for card view — no labels, minimal spacing.
 * Same dispatch logic as EntityInspector's FieldDispatch.
 */
function CardFieldDispatch({
  field,
  value,
  entity,
  bodyFieldName,
  onCommit,
}: {
  field: FieldDef;
  value: unknown;
  entity: Entity;
  bodyFieldName?: string;
  onCommit: (value: unknown) => void;
}) {
  // Markdown fields — inline editable
  if (field.type.kind === "markdown") {
    const text = typeof value === "string" ? value : "";
    return (
      <EditableMarkdown
        value={text}
        onCommit={(v) => onCommit(v)}
        className="leading-snug"
        inputClassName="leading-snug bg-transparent border-b border-ring w-full"
      />
    );
  }

  // Badge-list fields (tags, references) — use shared display component
  if (field.display === "badge-list") {
    const vals = Array.isArray(value) ? value : [];
    if (vals.length === 0) return null;
    return (
      <div className="mt-1.5">
        <BadgeListDisplay
          field={field}
          value={value}
          entity={entity}
          mode="compact"
        />
      </div>
    );
  }

  // Computed: progress — bar
  if (
    field.type.kind === "computed" &&
    (field.type as Record<string, unknown>).derive === "parse-body-progress"
  ) {
    const bodyText = bodyFieldName
      ? getStr(entity, bodyFieldName) || undefined
      : undefined;
    return <SubtaskProgress description={bodyText} className="mt-1.5" />;
  }

  return null;
}
