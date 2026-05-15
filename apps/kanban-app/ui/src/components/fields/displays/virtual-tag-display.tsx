import {
  Tooltip,
  TooltipTrigger,
  TooltipContent,
} from "@/components/ui/tooltip";
import { useBoardData } from "@/components/window-container";
import { CompactCellWrapper } from "./compact-cell-wrapper";
import type { VirtualTagMeta } from "@/types/kanban";

/** Props for VirtualTagDisplay. */
export interface VirtualTagDisplayProps {
  /** Array of virtual tag slugs (e.g. ["READY", "BLOCKING"]). */
  value: unknown;
  /** Presentation mode — `compact` (grid cells) or `full` (inspector rows). */
  mode?: "compact" | "full";
}

/**
 * Display component for the virtual_tags field.
 *
 * Renders computed virtual tags (READY, BLOCKED, BLOCKING) as colored pill
 * badges with tooltips. Colors and descriptions come from the backend
 * VirtualTagRegistry via `useBoardData().virtualTagMeta`.
 *
 * Renders an empty wrapper in compact mode (preserving the row's fixed
 * `ROW_HEIGHT` virtualizer contract) and `null` in full mode when the
 * value is empty or undefined.
 */
export function VirtualTagDisplay({ value, mode }: VirtualTagDisplayProps) {
  const boardData = useBoardData();
  const vtMeta = boardData?.virtualTagMeta ?? [];
  const tags = Array.isArray(value) ? (value as string[]) : [];

  if (tags.length === 0) {
    return mode === "compact" ? (
      <CompactCellWrapper>{null}</CompactCellWrapper>
    ) : null;
  }

  const pills = (
    <div className="flex flex-wrap gap-1">
      {tags.map((slug) => {
        const meta = vtMeta.find((m: VirtualTagMeta) => m.slug === slug);
        if (!meta) return null;

        return (
          <Tooltip key={slug}>
            <TooltipTrigger asChild>
              <span
                className="inline-flex items-center rounded-full px-1.5 py-px text-xs font-medium cursor-default"
                style={{
                  backgroundColor: `color-mix(in srgb, #${meta.color} 20%, transparent)`,
                  color: `#${meta.color}`,
                  border: `1px solid color-mix(in srgb, #${meta.color} 30%, transparent)`,
                }}
              >
                #{slug}
              </span>
            </TooltipTrigger>
            <TooltipContent side="bottom">{meta.description}</TooltipContent>
          </Tooltip>
        );
      })}
    </div>
  );

  return mode === "compact" ? (
    <CompactCellWrapper>{pills}</CompactCellWrapper>
  ) : (
    pills
  );
}
