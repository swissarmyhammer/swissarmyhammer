/**
 * Generic mention pill for rendered markdown — resolves any mentionable
 * entity type from the entity store and renders with its color.
 *
 * For #tags, delegates to TagPill (which has full context menu / inspect support).
 * For other mention types (@actor, etc.), renders a simpler colored pill.
 */

import { useEntityStore } from "@/lib/entity-store-context";
import { getStr } from "@/types/kanban";

interface MentionPillProps {
  /** Entity type (e.g. "tag", "actor") */
  entityType: string;
  /** The slug (display field value) without prefix */
  slug: string;
  /** The prefix character (e.g. "#", "@") */
  prefix: string;
  className?: string;
}

export function MentionPill({ entityType, slug, prefix, className }: MentionPillProps) {
  const { getEntities } = useEntityStore();
  const entities = getEntities(entityType);

  // Find entity by matching slug against common display fields
  const entity = entities.find((e) => {
    // Try common display field names
    for (const field of ["tag_name", "name", "id"]) {
      if (getStr(e, field) === slug) return true;
    }
    return false;
  });

  const color = entity ? getStr(entity, "color", "888888") : "888888";

  return (
    <span
      className={`inline-flex items-center rounded-full px-1.5 py-px text-xs font-medium cursor-default ${className ?? ""}`}
      style={{
        backgroundColor: `color-mix(in srgb, #${color} 20%, transparent)`,
        color: `#${color}`,
        border: `1px solid color-mix(in srgb, #${color} 30%, transparent)`,
      }}
    >
      {prefix}{slug}
    </span>
  );
}
