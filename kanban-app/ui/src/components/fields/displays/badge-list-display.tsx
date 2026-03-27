import { useMemo } from "react";
import { MentionPill } from "@/components/mention-pill";
import { useParentFocusScope } from "@/components/focus-scope";
import { useSchema } from "@/lib/schema-context";
import { useEntityStore } from "@/lib/entity-store-context";
import type { ClaimPredicate } from "@/lib/entity-focus-context";
import { moniker as buildMoniker } from "@/lib/moniker";
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
 *
 * Each pill is wrapped in a FocusScope from MentionPill, enabling
 * click-to-focus and context menu support per pill.
 */
export function BadgeListDisplay({ field, value, entity, mode }: DisplayProps) {
  const { mentionableTypes } = useSchema();
  const { getEntities } = useEntityStore();

  const values = Array.isArray(value) ? (value as string[]) : [];

  // Read target entity type from field type (set in YAML for both reference and computed fields)
  const targetEntityType = field.type.entity as string | undefined;
  const isComputedSlug = !!(field.type as Record<string, unknown>)
    .commit_display_names;

  // Look up mention config for the target entity type
  const mentionConfig = useMemo(
    () => mentionableTypes.find((mt) => mt.entityType === targetEntityType),
    [mentionableTypes, targetEntityType],
  );

  const prefix = mentionConfig?.prefix ?? "";
  const displayField = mentionConfig?.displayField;

  // For reference fields, values are entity IDs — resolve to slugified display names
  const targetEntities = useMemo(
    () => (targetEntityType ? getEntities(targetEntityType) : []),
    [targetEntityType, getEntities],
  );

  // Build ID → slugified display name map for reference fields
  const idToSlug = useMemo(() => {
    if (isComputedSlug || !displayField) return null;
    const map = new Map<string, string>();
    for (const e of targetEntities) {
      const raw = getStr(e, displayField);
      if (raw) map.set(e.id, slugify(raw));
    }
    return map;
  }, [isComputedSlug, displayField, targetEntities]);

  const parentMoniker = useParentFocusScope();

  // Compute the moniker each pill's FocusScope will use, mirroring MentionPill's
  // internal resolution so claimWhen predicates reference the correct strings.
  const pillMonikers = useMemo(() => {
    return values.map((val) => {
      if (isComputedSlug) {
        const tagEntity = targetEntities.find((e) => {
          const raw = getStr(e, displayField ?? "name");
          return raw && (raw === val || slugify(raw) === val);
        });
        return buildMoniker(targetEntityType ?? "tag", tagEntity?.id ?? val);
      } else {
        return buildMoniker(targetEntityType ?? "tag", val);
      }
    });
  }, [values, isComputedSlug, targetEntities, displayField, targetEntityType]);

  // Build claimWhen predicates so nav.left/nav.right moves focus between pills.
  const pillClaimPredicates = useMemo(() => {
    return pillMonikers.map((_, i) => {
      const predicates: ClaimPredicate[] = [];
      // nav.right: claim when the pill to my left (or parent field) is focused
      if (i === 0 && parentMoniker) {
        predicates.push({ command: "nav.right", when: (f) => f === parentMoniker });
      }
      if (i > 0) {
        predicates.push({ command: "nav.right", when: (f) => f === pillMonikers[i - 1] });
      }
      // nav.left: claim when the pill to my right is focused
      if (i < pillMonikers.length - 1) {
        predicates.push({ command: "nav.left", when: (f) => f === pillMonikers[i + 1] });
      }
      return predicates;
    });
  }, [pillMonikers, parentMoniker]);

  if (values.length === 0) {
    return mode === "compact" ? (
      <span className="text-muted-foreground/50">-</span>
    ) : (
      <span className="text-sm text-muted-foreground italic">None</span>
    );
  }

  return (
    <div className="flex flex-wrap gap-1">
      {values.map((val, i) => {
        // For computed tags, val is already the slug. For references, resolve ID to display slug.
        const slug = idToSlug ? (idToSlug.get(val) ?? val) : val;
        return (
          <MentionPill
            key={val}
            entityType={targetEntityType ?? "tag"}
            slug={slug}
            prefix={prefix}
            taskId={isComputedSlug ? entity.id : undefined}
            claimWhen={pillClaimPredicates[i]}
          />
        );
      })}
    </div>
  );
}
