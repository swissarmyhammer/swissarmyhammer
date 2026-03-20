import { useMemo } from "react";
import { MentionPill } from "@/components/mention-pill";
import { useSchema } from "@/lib/schema-context";
import { useEntityStore } from "@/lib/entity-store-context";
import { slugify } from "@/lib/slugify";
import { getStr } from "@/types/kanban";
import type { DisplayProps } from "./text-display";

/**
 * Badge list display — renders mention pills for any badge-list field.
 *
 * Works for both:
 * - Computed tag fields (values are tag slugs like "bugfix")
 * - Reference fields like depends_on (values are entity IDs)
 *
 * Resolves the target entity type and mention prefix from the field definition
 * and schema, so it works generically for tags, tasks, or any mentionable type.
 */
export function BadgeListDisplay({ field, value, entity, mode }: DisplayProps) {
  const { mentionableTypes } = useSchema();
  const { getEntities } = useEntityStore();

  const values = Array.isArray(value) ? (value as string[]) : [];

  // Determine target entity type from the field definition
  const isComputedTags =
    field.type.kind === "computed" &&
    (field.type as Record<string, unknown>).derive === "parse-body-tags";
  const targetEntityType = isComputedTags
    ? "tag"
    : (field.type.entity as string | undefined);

  // Look up mention config for the target entity type
  const mentionConfig = useMemo(
    () => mentionableTypes.find((mt) => mt.entityType === targetEntityType),
    [mentionableTypes, targetEntityType],
  );

  const prefix = mentionConfig?.prefix ?? (isComputedTags ? "#" : "");
  const displayField = mentionConfig?.displayField;

  // For reference fields, values are entity IDs — resolve to slugified display names
  const targetEntities = useMemo(
    () => (targetEntityType ? getEntities(targetEntityType) : []),
    [targetEntityType, getEntities],
  );

  // Build ID → slugified display name map for reference fields
  const idToSlug = useMemo(() => {
    if (isComputedTags || !displayField) return null;
    const map = new Map<string, string>();
    for (const e of targetEntities) {
      const raw = getStr(e, displayField);
      if (raw) map.set(e.id, slugify(raw));
    }
    return map;
  }, [isComputedTags, displayField, targetEntities]);

  if (values.length === 0) {
    return mode === "compact" ? (
      <span className="text-muted-foreground/50">-</span>
    ) : (
      <span className="text-sm text-muted-foreground italic">None</span>
    );
  }

  return (
    <div className="flex flex-wrap gap-1">
      {values.map((val) => {
        // For computed tags, val is already the slug. For references, resolve ID to display slug.
        const slug = idToSlug ? (idToSlug.get(val) ?? val) : val;
        return (
          <MentionPill
            key={val}
            entityType={targetEntityType ?? "tag"}
            slug={slug}
            prefix={prefix}
            taskId={isComputedTags ? entity.id : undefined}
          />
        );
      })}
    </div>
  );
}
