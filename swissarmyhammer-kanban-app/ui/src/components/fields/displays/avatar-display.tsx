/**
 * Avatar display for reference fields targeting actors (e.g. assignees).
 *
 * Renders a row of overlapping Avatar components from an array of actor IDs.
 * Compact mode shows up to 3 with "+N" overflow. Full mode shows all.
 */

import { Avatar } from "@/components/avatar";
import type { DisplayProps } from "./text-display";

const MAX_COMPACT = 3;

export function AvatarDisplay({ value, mode }: DisplayProps) {
  const ids: string[] = Array.isArray(value)
    ? value.filter((v): v is string => typeof v === "string")
    : [];

  if (ids.length === 0) {
    return mode === "compact"
      ? <span className="text-muted-foreground/50">-</span>
      : <span className="text-muted-foreground italic text-sm">No assignees</span>;
  }

  const isCompact = mode === "compact";
  const shown = isCompact ? ids.slice(0, MAX_COMPACT) : ids;
  const overflow = isCompact ? ids.length - MAX_COMPACT : 0;
  const size = isCompact ? "sm" : "md";

  return (
    <div className="flex items-center">
      {shown.map((id, i) => (
        <Avatar
          key={id}
          actorId={id}
          size={size}
          className={i > 0 ? "-ml-1.5 ring-2 ring-background" : ""}
        />
      ))}
      {overflow > 0 && (
        <span className="ml-1 text-xs text-muted-foreground">+{overflow}</span>
      )}
    </div>
  );
}
