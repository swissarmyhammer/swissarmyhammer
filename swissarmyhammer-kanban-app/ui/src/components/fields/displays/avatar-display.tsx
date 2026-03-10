/**
 * Avatar display for reference fields targeting actors (e.g. assignees).
 *
 * Renders a row of overlapping Avatar components from an array of actor IDs.
 * Same rendering in both grid (compact) and inspector (full) modes.
 */

import { Avatar } from "@/components/avatar";

interface AvatarDisplayProps {
  value: unknown;
}

export function AvatarDisplay({ value }: AvatarDisplayProps) {
  const ids: string[] = Array.isArray(value)
    ? value.filter((v): v is string => typeof v === "string")
    : [];

  if (ids.length === 0) {
    return <span className="text-muted-foreground/50">-</span>;
  }

  return (
    <div className="flex items-center">
      {ids.map((id, i) => (
        <Avatar
          key={id}
          actorId={id}
          size="md"
          className={i > 0 ? "-ml-1.5 ring-2 ring-background" : ""}
        />
      ))}
    </div>
  );
}
