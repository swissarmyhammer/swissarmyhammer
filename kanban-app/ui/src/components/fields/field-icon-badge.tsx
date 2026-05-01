/**
 * `<FieldIconBadge>` — the tooltip-wrapped lucide badge that decorates a
 * field row.
 *
 * Lives next to `<Field>` so both the inspector callsite (`FieldRow`) and
 * `<Field withIcon />` itself can reach the same component without
 * cross-importing the inspector. The icon resolves outside this file —
 * callers pass the already-resolved `LucideIcon` plus the tooltip text.
 *
 * Visual: a 14px lucide glyph in a 20px-tall inline-flex span, muted
 * foreground colour. The wrapping `<Tooltip>` opens a left-aligned popover
 * with the field's description (or humanised name fallback). Identical to
 * the legacy `FieldIconTooltip` component the inspector used; only the
 * location and name changed.
 */
import {
  Tooltip,
  TooltipTrigger,
  TooltipContent,
} from "@/components/ui/tooltip";
import type { LucideIcon } from "lucide-react";

/** Props for `<FieldIconBadge>`. */
export interface FieldIconBadgeProps {
  /** Resolved lucide icon component to render. */
  Icon: LucideIcon;
  /** Tooltip body — the field description, or its humanised name fallback. */
  tip: string;
}

/**
 * Tooltip-wrapped field icon badge. Used by `<Field withIcon />` to render
 * the leftmost child inside the field's `<FocusZone>`.
 */
export function FieldIconBadge({ Icon, tip }: FieldIconBadgeProps) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <span className="h-5 inline-flex items-center shrink-0 text-muted-foreground">
          <Icon size={14} />
        </span>
      </TooltipTrigger>
      <TooltipContent side="left" align="start">
        {tip}
      </TooltipContent>
    </Tooltip>
  );
}
