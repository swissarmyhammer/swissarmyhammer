/**
 * Universal mention pill for all entity types — tags, actors, tasks, etc.
 *
 * Resolves entities from the entity store, renders colored pills with:
 * - Tooltip (if entity has a description field)
 * - Right-click context menu via FocusScope + CommandScope
 * - entity.inspect command (always)
 * - task.untag command (for tags on a task, when taskId is provided)
 * - Slugified matching for entities whose display field contains spaces (e.g. task titles)
 */

import { useMemo } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import {
  Tooltip,
  TooltipTrigger,
  TooltipContent,
} from "@/components/ui/tooltip";
import { FocusScope } from "@/components/focus-scope";
import { useEntityStore } from "@/lib/entity-store-context";
import { useEntityCommands } from "@/lib/entity-commands";
import { useSchema } from "@/lib/schema-context";
import { moniker } from "@/lib/moniker";
import { slugify } from "@/lib/slugify";
import type { CommandDef } from "@/lib/command-scope";
import type { ClaimPredicate } from "@/lib/entity-focus-context";
import { getStr } from "@/types/kanban";

interface MentionPillProps {
  /** Entity type (e.g. "tag", "actor", "task") */
  entityType: string;
  /** The slug (display field value or slugified form) without prefix */
  slug: string;
  /** The prefix character (e.g. "#", "@", "^") */
  prefix: string;
  /** If set, adds a "Remove Tag" context menu command (for tags on a task) */
  taskId?: string;
  className?: string;
  /** Predicates that let this pill's FocusScope claim focus on nav commands. */
  claimWhen?: ClaimPredicate[];
}

export function MentionPill({
  entityType,
  slug,
  prefix,
  taskId,
  className,
  claimWhen,
}: MentionPillProps) {
  const { getEntities } = useEntityStore();
  const { mentionableTypes } = useSchema();
  const entities = getEntities(entityType);

  // Use the schema-declared display field for this entity type
  const displayField =
    mentionableTypes.find((mt) => mt.entityType === entityType)?.displayField ??
    "name";

  // Find entity by matching slug against the display field (with slugified fallback)
  const entity = entities.find((e) => {
    const val = getStr(e, displayField);
    if (val && (val === slug || slugify(val) === slug)) return true;
    // Fall back to ID match
    return e.id === slug;
  });

  const color = entity ? getStr(entity, "color", "888888") : "888888";
  const description = entity
    ? getStr(entity, "description") || undefined
    : undefined;

  // Resolve display name for tooltip — show full name when slug is abbreviated
  const displayName = entity
    ? getStr(entity, displayField) || undefined
    : undefined;
  const tooltipText =
    displayName && displayName !== slug
      ? description
        ? `${displayName}\n\n${description}`
        : displayName
      : description;
  const entityId = entity?.id ?? slug;
  const scopeMoniker = moniker(entityType, entityId);

  // Build the local task.untag extra command — only for tags on a specific task
  const extraCommands = useMemo<CommandDef[] | undefined>(() => {
    if (entityType === "tag" && taskId) {
      return [
        {
          id: "task.untag",
          name: "Remove Tag",
          contextMenu: true,
          args: { id: taskId, tag: slug },
        },
      ];
    }
    return undefined;
  }, [entityType, taskId, slug]);

  const commands = useEntityCommands(
    entityType,
    entityId,
    entity ?? undefined,
    extraCommands,
  );

  return (
    <FocusScope moniker={scopeMoniker} commands={commands} className="inline mention-pill-focus" claimWhen={claimWhen}>
      <MentionPillInner
        slug={slug}
        prefix={prefix}
        color={color}
        tooltipText={tooltipText}
        richTooltip={!!description}
        className={className}
      />
    </FocusScope>
  );
}

/** Shorten a slug to at most 3 hyphen-separated words for compact display. */
function briefSlug(slug: string): string {
  const parts = slug.split("-");
  if (parts.length <= 3) return slug;
  return parts.slice(0, 3).join("-") + "…";
}

/**
 * Inner component rendered inside FocusScope so it can access
 * useContextMenu() from the correct CommandScope context.
 */
function MentionPillInner({
  slug,
  prefix,
  color,
  tooltipText,
  richTooltip,
  className,
}: {
  slug: string;
  prefix: string;
  color: string;
  tooltipText?: string;
  richTooltip?: boolean;
  className?: string;
}) {
  const pill = (
    <span
      className={`inline-flex items-center rounded-full px-1.5 py-px text-xs font-medium cursor-default ${className ?? ""}`}
      style={{
        backgroundColor: `color-mix(in srgb, #${color} 20%, transparent)`,
        color: `#${color}`,
        border: `1px solid color-mix(in srgb, #${color} 30%, transparent)`,
      }}
    >
      {prefix}
      {briefSlug(slug)}
    </span>
  );

  if (!tooltipText) return pill;

  return (
    <Tooltip>
      <TooltipTrigger asChild>{pill}</TooltipTrigger>
      <TooltipContent
        side="bottom"
        className={
          richTooltip ? "prose prose-sm dark:prose-invert max-w-xs" : "max-w-xs"
        }
      >
        {richTooltip ? (
          <ReactMarkdown remarkPlugins={[remarkGfm]}>
            {tooltipText}
          </ReactMarkdown>
        ) : (
          tooltipText
        )}
      </TooltipContent>
    </Tooltip>
  );
}
