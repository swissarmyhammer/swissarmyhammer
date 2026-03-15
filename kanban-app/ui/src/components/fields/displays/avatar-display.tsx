/**
 * Avatar display — handles two value shapes:
 *
 * 1. **String** (data URI or URL): renders the image directly as a circle.
 *    Used for the actor entity's own `avatar` field in the inspector.
 * 2. **Array of actor IDs**: renders a row of overlapping Avatar components.
 *    Used for reference fields like `assignees` in grid and inspector views.
 */

import { Avatar } from "@/components/avatar";

interface AvatarDisplayProps {
  value: unknown;
}

export function AvatarDisplay({ value }: AvatarDisplayProps) {
  // String value — render the image directly (actor's own avatar field)
  if (typeof value === "string" && value.length > 0) {
    return (
      <img
        src={value}
        alt="avatar"
        className="w-10 h-10 rounded-full object-cover"
      />
    );
  }

  // Array value — render overlapping Actor avatars (assignees reference field)
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
