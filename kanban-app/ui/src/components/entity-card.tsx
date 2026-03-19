import { forwardRef, useCallback, useMemo } from "react";
import { GripVertical, Info } from "lucide-react";
import { EditableMarkdown } from "@/components/editable-markdown";
import { SubtaskProgress } from "@/components/subtask-progress";
import { TagPill } from "@/components/tag-pill";
import { FocusScope } from "@/components/focus-scope";
import { useFieldUpdate } from "@/lib/field-update-context";
import { useSchema } from "@/lib/schema-context";
import { useEntityStore } from "@/lib/entity-store-context";
import { useInspect } from "@/lib/inspect-context";
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
  function EntityCard({ entity, dragHandleProps, style, draggable, onDragStart, onDragEnd, ...rest }, ref) {
    const { updateField } = useFieldUpdate();
    const { getSchema } = useSchema();
    const { getEntities, getEntity } = useEntityStore();
    const inspectEntity = useInspect();
    const tags = getEntities("tag");
    const schema = getSchema(entity.entity_type);
    const bodyFieldName = schema?.entity?.body_field;

    const entityMoniker = moniker(entity.entity_type, entity.id);

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

    const commands = useMemo(() => [
      {
        id: "entity.inspect",
        name: `Inspect ${entity.entity_type}`,
        target: entityMoniker,
        contextMenu: true,
        execute: () => inspectEntity(entityMoniker),
      },
    ], [entity.entity_type, entityMoniker, inspectEntity]);

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
            <DependencyPills
              entity={entity}
              getEntity={getEntity}
              onInspect={inspectEntity}
            />
          </div>
          <button
            type="button"
            className="shrink-0 mt-0.5 p-0.5 rounded text-muted-foreground/50 hover:text-muted-foreground hover:bg-muted transition-colors"
            onClick={(e) => { e.stopPropagation(); inspectEntity(entityMoniker); }}
            title="Inspect"
          >
            <Info className="h-3.5 w-3.5" />
          </button>
        </div>
      </FocusScope>
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

/** Truncate a string to maxLen chars with ellipsis. */
function truncate(s: string, maxLen: number): string {
  return s.length > maxLen ? s.slice(0, maxLen) + "…" : s;
}

/**
 * Render dependency pills for blocked_by and blocks relationships.
 *
 * Shows compact clickable pills after tags in the card header.
 * blocked_by pills are orange (warning), blocks pills are muted.
 * Renders nothing if there are no dependencies.
 */
function DependencyPills({
  entity,
  getEntity,
  onInspect,
}: {
  entity: Entity;
  getEntity: (entityType: string, id: string) => Entity | undefined;
  onInspect: (moniker: string) => void;
}) {
  const blockedBy = Array.isArray(entity.fields.blocked_by)
    ? (entity.fields.blocked_by as string[])
    : [];
  const blocks = Array.isArray(entity.fields.blocks)
    ? (entity.fields.blocks as string[])
    : [];

  if (blockedBy.length === 0 && blocks.length === 0) return null;

  return (
    <div className="flex flex-wrap gap-1 mt-1.5">
      {blockedBy.map((id) => {
        const dep = getEntity("task", id);
        const title = dep ? getStr(dep, "title") || id : id;
        return (
          <button
            key={`blocked-${id}`}
            type="button"
            className="inline-flex items-center rounded-full px-1.5 py-px text-xs font-medium bg-amber-500/10 text-amber-500 border border-amber-500/30 cursor-pointer hover:bg-amber-500/20"
            onClick={(e) => { e.stopPropagation(); onInspect(moniker("task", id)); }}
            title={`Blocked by: ${title}`}
          >
            ⊳ {truncate(title, 20)}
          </button>
        );
      })}
      {blocks.map((id) => {
        const dep = getEntity("task", id);
        const title = dep ? getStr(dep, "title") || id : id;
        return (
          <button
            key={`blocks-${id}`}
            type="button"
            className="inline-flex items-center rounded-full px-1.5 py-px text-xs font-medium bg-muted text-muted-foreground border border-border cursor-pointer hover:bg-muted/80"
            onClick={(e) => { e.stopPropagation(); onInspect(moniker("task", id)); }}
            title={`Blocks: ${title}`}
          >
            ⊲ {truncate(title, 20)}
          </button>
        );
      })}
    </div>
  );
}
