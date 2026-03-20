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

import { useCallback, useMemo, useRef } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import {
  Tooltip,
  TooltipTrigger,
  TooltipContent,
} from "@/components/ui/tooltip";
import { FocusScope } from "@/components/focus-scope";
import { useContextMenu } from "@/lib/context-menu";
import { useEntityFocus } from "@/lib/entity-focus-context";
import { useEntityStore } from "@/lib/entity-store-context";
import { useInspect } from "@/lib/inspect-context";
import { moniker } from "@/lib/moniker";
import { slugify } from "@/lib/slugify";
import type { CommandDef } from "@/lib/command-scope";
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
}

export function MentionPill({
  entityType,
  slug,
  prefix,
  taskId,
  className,
}: MentionPillProps) {
  const { getEntities } = useEntityStore();
  const inspectEntity = useInspect();
  const entities = getEntities(entityType);

  // Find entity by matching slug against common display fields,
  // falling back to slugified comparison for fields with spaces (e.g. task titles)
  const entity = entities.find((e) => {
    for (const field of ["tag_name", "name", "title", "id"]) {
      const val = getStr(e, field);
      if (!val) continue;
      if (val === slug || slugify(val) === slug) return true;
    }
    return false;
  });

  const color = entity ? getStr(entity, "color", "888888") : "888888";
  const description = entity
    ? getStr(entity, "description") || undefined
    : undefined;

  // Resolve display name for tooltip — show full name when slug is abbreviated
  const displayName = entity
    ? getStr(entity, "title") ||
      getStr(entity, "name") ||
      getStr(entity, "tag_name") ||
      undefined
    : undefined;
  const tooltipText =
    displayName && displayName !== slug
      ? description
        ? `${displayName}\n\n${description}`
        : displayName
      : description;
  const entityId = entity?.id ?? slug;
  const scopeMoniker = moniker(entityType, entityId);

  // Keep a ref so execute always uses the latest resolved moniker
  const monikerRef = useRef(scopeMoniker);
  monikerRef.current = scopeMoniker;

  const commands = useMemo<CommandDef[]>(() => {
    const cmds: CommandDef[] = [];

    cmds.push({
      id: "entity.inspect",
      name: `Inspect ${entityType}`,
      target: scopeMoniker,
      contextMenu: true,
      execute: () => inspectEntity(monikerRef.current),
    });

    // Remove command — only for tags on a specific task
    if (entityType === "tag" && taskId) {
      cmds.push({
        id: "task.untag",
        name: "Remove Tag",
        contextMenu: true,
        args: { id: taskId, tag: slug },
      });
    }

    return cmds;
  }, [entityType, scopeMoniker, taskId, slug, inspectEntity]);

  return (
    <FocusScope moniker={scopeMoniker} commands={commands} className="inline">
      <MentionPillInner
        slug={slug}
        prefix={prefix}
        color={color}
        tooltipText={tooltipText}
        scopeMoniker={scopeMoniker}
        className={className}
      />
    </FocusScope>
  );
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
  scopeMoniker,
  className,
}: {
  slug: string;
  prefix: string;
  color: string;
  tooltipText?: string;
  scopeMoniker: string;
  className?: string;
}) {
  const contextMenuHandler = useContextMenu();
  const { setFocus } = useEntityFocus();

  const handleContextMenu = useCallback(
    (e: React.MouseEvent) => {
      e.preventDefault();
      e.stopPropagation();
      setFocus(scopeMoniker);
      contextMenuHandler(e);
    },
    [scopeMoniker, setFocus, contextMenuHandler],
  );

  const pill = (
    <span
      className={`inline-flex items-center rounded-full px-1.5 py-px text-xs font-medium cursor-default ${className ?? ""}`}
      style={{
        backgroundColor: `color-mix(in srgb, #${color} 20%, transparent)`,
        color: `#${color}`,
        border: `1px solid color-mix(in srgb, #${color} 30%, transparent)`,
      }}
      onContextMenu={handleContextMenu}
    >
      {prefix}
      {slug}
    </span>
  );

  if (!tooltipText) return pill;

  return (
    <Tooltip>
      <TooltipTrigger asChild>{pill}</TooltipTrigger>
      <TooltipContent
        side="bottom"
        className="prose prose-sm dark:prose-invert max-w-xs"
      >
        <ReactMarkdown remarkPlugins={[remarkGfm]}>{tooltipText}</ReactMarkdown>
      </TooltipContent>
    </Tooltip>
  );
}
