import { invoke } from "@tauri-apps/api/core";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { Tooltip, TooltipTrigger, TooltipContent } from "@/components/ui/tooltip";
import type { Entity } from "@/types/kanban";

interface TagPillProps {
  slug: string;
  tags: Entity[];
  taskId?: string;
  className?: string;
}

/**
 * Single shared tag pill component used everywhere:
 * - Inline in rendered markdown (remark plugin)
 * - Tag list on task cards
 * - Tag list in the detail panel header
 *
 * Accepts tag entities (entity_type: "tag") whose fields contain tag_name,
 * color, and description. Resolves the display tag by matching slug against
 * the tag_name field.
 *
 * Right-click opens a native context menu via Tauri.
 * Hover shows a markdown tooltip with the tag description.
 */
export function TagPill({ slug, tags, taskId, className }: TagPillProps) {
  const tag = tags.find((t) => (t.fields.tag_name as string) === slug);
  const color = (tag?.fields.color as string) ?? "888888";
  const description = (tag?.fields.description as string) || undefined;

  const pill = (
    <span
      className={`inline-flex items-center rounded-full px-1.5 py-px text-xs font-medium cursor-default ${className ?? ""}`}
      style={{
        backgroundColor: `color-mix(in srgb, #${color} 20%, transparent)`,
        color: `#${color}`,
        border: `1px solid color-mix(in srgb, #${color} 30%, transparent)`,
      }}
      onContextMenu={(e) => {
        e.preventDefault();
        e.stopPropagation();
        invoke("show_tag_context_menu", { tagId: slug, taskId: taskId ?? null }).catch(console.error);
      }}
    >
      #{slug}
    </span>
  );

  if (!description) return pill;

  return (
    <Tooltip>
      <TooltipTrigger asChild>{pill}</TooltipTrigger>
      <TooltipContent side="bottom" className="prose prose-sm dark:prose-invert max-w-xs">
        <ReactMarkdown remarkPlugins={[remarkGfm]}>{description}</ReactMarkdown>
      </TooltipContent>
    </Tooltip>
  );
}
