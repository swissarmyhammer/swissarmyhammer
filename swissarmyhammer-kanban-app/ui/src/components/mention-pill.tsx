/**
 * Generic mention pill for rendered markdown — resolves any mentionable
 * entity type from the entity store and renders with its color.
 *
 * For #tags, delegates to TagPill (which has full context menu / inspect support).
 * For other mention types (@actor, etc.), renders a colored pill wrapped in
 * FocusScope with entity.inspect support (right-click context menu + double-click).
 */

import { useMemo, useRef } from "react";
import { FocusScope } from "@/components/focus-scope";
import { useEntityStore } from "@/lib/entity-store-context";
import { useInspect } from "@/lib/inspect-context";
import { moniker } from "@/lib/moniker";
import type { CommandDef } from "@/lib/command-scope";
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
  const inspectEntity = useInspect();
  const entities = getEntities(entityType);

  // Find entity by matching slug against common display fields
  const entity = entities.find((e) => {
    for (const field of ["tag_name", "name", "id"]) {
      if (getStr(e, field) === slug) return true;
    }
    return false;
  });

  const color = entity ? getStr(entity, "color", "888888") : "888888";
  const entityId = entity?.id ?? slug;
  const scopeMoniker = moniker(entityType, entityId);

  // Keep a ref so execute always uses the latest resolved moniker
  const monikerRef = useRef(scopeMoniker);
  monikerRef.current = scopeMoniker;

  const commands = useMemo<CommandDef[]>(() => [
    {
      id: "entity.inspect",
      name: `Inspect ${entityType}`,
      target: scopeMoniker,
      contextMenu: true,
      execute: () => inspectEntity(monikerRef.current),
    },
  ], [entityType, scopeMoniker, inspectEntity]);

  return (
    <FocusScope moniker={scopeMoniker} commands={commands} className="inline">
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
    </FocusScope>
  );
}
