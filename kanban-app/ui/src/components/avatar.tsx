/**
 * Avatar component — renders an actor's profile image or colored-initials circle.
 *
 * Resolves actor from EntityStore by ID. Falls back to initials + deterministic
 * color if no avatar image is available.
 *
 * Wrapped in FocusScope so right-click and double-click open the inspector.
 */

import { FocusScope } from "@/components/focus-scope";
import {
  Tooltip,
  TooltipTrigger,
  TooltipContent,
} from "@/components/ui/tooltip";
import { useEntityStore } from "@/lib/entity-store-context";
import { useSchema } from "@/lib/schema-context";
import { moniker } from "@/lib/moniker";
import { deriveActorColor } from "@/lib/actor-colors";
import { getStr } from "@/types/kanban";

const SIZES = {
  sm: "w-5 h-5 min-w-5 min-h-5 text-[9px]",
  md: "w-7 h-7 min-w-7 min-h-7 text-xs",
  lg: "w-9 h-9 min-w-9 min-h-9 text-sm",
} as const;

interface AvatarProps {
  actorId: string;
  size?: "sm" | "md" | "lg";
  className?: string;
}

/** Extract initials from a name (first letter of first two words). */
function initials(name: string): string {
  const words = name.trim().split(/\s+/);
  if (words.length >= 2) {
    return (words[0][0] + words[1][0]).toUpperCase();
  }
  return (name[0] ?? "?").toUpperCase();
}

export function Avatar({ actorId, size = "md", className }: AvatarProps) {
  const { getEntity } = useEntityStore();
  const { mentionableTypes } = useSchema();
  const actor = getEntity("actor", actorId);

  // Resolve the display name field from the actor schema (mention_display_field)
  const nameField =
    mentionableTypes.find((mt) => mt.entityType === "actor")?.displayField ??
    "name";
  const name = actor ? getStr(actor, nameField) || actorId : actorId;
  const color = actor
    ? getStr(actor, "color") || deriveActorColor(actorId)
    : deriveActorColor(actorId);
  const avatar = actor ? getStr(actor, "avatar") : undefined;

  const sizeClass = SIZES[size];
  const scopeMoniker = actor?.moniker ?? moniker("actor", actorId);

  const element = avatar ? (
    <img
      src={avatar}
      alt={name}
      aria-label={name}
      className={`${sizeClass} rounded-full object-cover shrink-0 ${className ?? ""}`}
    />
  ) : (
    <span
      aria-label={name}
      className={`${sizeClass} rounded-full shrink-0 inline-flex items-center justify-center font-medium leading-none ${className ?? ""}`}
      style={{
        backgroundColor: `#${color}`,
        color: "#fff",
      }}
    >
      {initials(name)}
    </span>
  );

  const inner = (
    <Tooltip>
      <TooltipTrigger asChild>{element}</TooltipTrigger>
      <TooltipContent side="top">{name}</TooltipContent>
    </Tooltip>
  );

  return (
    <FocusScope moniker={scopeMoniker} className="inline-block">
      {inner}
    </FocusScope>
  );
}
