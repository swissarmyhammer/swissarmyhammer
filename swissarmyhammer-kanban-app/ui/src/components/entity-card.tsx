import { forwardRef, useCallback, useMemo } from "react";
import { GripVertical, Info } from "lucide-react";
import { EditableMarkdown } from "@/components/editable-markdown";
import { SubtaskProgress } from "@/components/subtask-progress";
import { TagPill } from "@/components/tag-pill";
import { useFieldUpdate } from "@/lib/field-update-context";
import { useSchema } from "@/lib/schema-context";
import { useEntityStore } from "@/lib/entity-store-context";
import type { Entity, FieldDef } from "@/types/kanban";
import { getStr } from "@/types/kanban";

interface EntityCardProps {
  entity: Entity;
  isBlocked?: boolean;
  onInspect?: (entityId: string) => void;
  dragHandleProps?: Record<string, unknown>;
  style?: React.CSSProperties;
}

/**
 * Compact card view for an entity on the board.
 *
 * Renders header-section fields from the schema using the same
 * dispatch logic as EntityInspector — no hardcoded field names.
 */
export const EntityCard = forwardRef<HTMLDivElement, EntityCardProps>(
  function EntityCard({ entity, isBlocked, onInspect, dragHandleProps, style, ...rest }, ref) {
    const { updateField } = useFieldUpdate();
    const { getSchema } = useSchema();
    const { getEntities } = useEntityStore();
    const tags = getEntities("tag");
    const schema = getSchema(entity.entity_type);
    const bodyFieldName = schema?.entity?.body_field;

    const headerFields = useMemo(
      () => (schema?.fields ?? []).filter((f) => f.section === "header"),
      [schema],
    );

    const handleCommit = useCallback(
      (fieldName: string, value: unknown) => {
        updateField(entity.entity_type, entity.id, fieldName, value).catch(() => {});
      },
      [updateField, entity.entity_type, entity.id],
    );

    return (
      <div
        ref={ref}
        style={style}
        className={`rounded-md bg-card px-3 py-2 text-sm border border-border hover:ring-1 hover:ring-ring transition-shadow relative group flex items-start gap-2 ${
          isBlocked ? "opacity-50" : ""
        }`}
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
        {onInspect && (
          <button
            type="button"
            className="absolute top-1.5 right-1.5 p-0.5 rounded text-muted-foreground/50 hover:text-muted-foreground hover:bg-muted transition-colors"
            onClick={(e) => { e.stopPropagation(); onInspect(entity.id); }}
            title="Inspect"
          >
            <Info className="h-3.5 w-3.5" />
          </button>
        )}
        <div className="flex-1 min-w-0">
          {headerFields.map((field) => (
            <CardFieldDispatch
              key={field.name}
              field={field}
              value={entity.fields[field.name]}
              entity={entity}
              tags={tags}
              bodyFieldName={bodyFieldName}
              onCommit={(value) => handleCommit(field.name, value)}
            />
          ))}
        </div>
      </div>
    );
  }
);

/**
 * Compact field renderer for card view — no labels, minimal spacing.
 * Same dispatch logic as EntityInspector's FieldDispatch.
 */
function CardFieldDispatch({
  field,
  value,
  entity,
  tags,
  bodyFieldName,
  onCommit,
}: {
  field: FieldDef;
  value: unknown;
  entity: Entity;
  tags: Entity[];
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

  // Computed: tags — pill list
  if (field.display === "badge-list" && field.type.kind === "computed") {
    const slugs = Array.isArray(value) ? (value as string[]) : [];
    if (slugs.length === 0) return null;
    return (
      <div className="flex flex-wrap gap-1 mt-1.5">
        {slugs.map((slug) => (
          <TagPill key={slug} slug={slug} tags={tags} taskId={entity.id} />
        ))}
      </div>
    );
  }

  // Computed: progress — bar
  if (field.type.kind === "computed" && (field.type as Record<string, unknown>).derive === "parse-body-progress") {
    const bodyText = bodyFieldName ? getStr(entity, bodyFieldName) || undefined : undefined;
    return <SubtaskProgress description={bodyText} className="mt-1.5" />;
  }

  return null;
}
