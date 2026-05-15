/**
 * Avatar display — handles two value shapes:
 *
 * 1. **String** (data URI or URL): renders the image directly as a circle.
 *    Used for the actor entity's own `avatar` field in the inspector.
 * 2. **Array of actor IDs**: renders a row of overlapping Avatar components.
 *    Used for reference fields like `assignees` in grid and inspector views.
 *
 * Empty-state handling mirrors `BadgeListDisplay`/`BadgeDisplay`: the
 * field's YAML `placeholder` (e.g. `"Assign"` on `assignees`) is rendered
 * when set, falling back to `-` (compact) or `None` (full) when the field
 * has not opted in.
 *
 * In compact mode, the output is wrapped in {@link CompactCellWrapper} so
 * populated and empty variants render at the exact same pixel height —
 * required for the `DataTable` row virtualizer's fixed `ROW_HEIGHT`.
 * Avatars also shrink to `size="sm"` (20px) in compact mode so they fit
 * inside the wrapper's 24px height contract.
 */

import { Avatar } from "@/components/avatar";
import { CompactCellWrapper } from "./compact-cell-wrapper";
import type { DisplayProps } from "./text-display";

/** Props for {@link EmptyAvatar}. */
interface EmptyAvatarProps {
  /** Display mode — drives the styling and fallback text. */
  mode: "compact" | "full";
  /** Optional YAML-configured placeholder; falls back to mode-specific defaults. */
  placeholder?: string;
}

/**
 * Empty-state rendering — compact grid cells vs. full inspector rows.
 *
 * Honors the field's YAML `placeholder` when set, falling back to the
 * legacy `-` (compact) / `None` (full) text otherwise. Mirrors
 * {@link file://./badge-list-display.tsx EmptyBadgeList} so every
 * empty-state convention stays consistent across displays.
 */
function EmptyAvatar({ mode, placeholder }: EmptyAvatarProps) {
  if (mode === "compact") {
    return (
      <span className="text-muted-foreground/50">{placeholder ?? "-"}</span>
    );
  }
  return (
    <span className="text-sm text-muted-foreground italic">
      {placeholder ?? "None"}
    </span>
  );
}

export function AvatarDisplay({ field, value, mode }: DisplayProps) {
  // String value — render the image directly (actor's own avatar field).
  // This branch is only used in `mode="full"` (the inspector); the YAML
  // `actor.avatar` field has no `compact` representation, so no wrapper
  // is needed here.
  if (typeof value === "string" && value.length > 0) {
    return (
      <img
        src={value}
        alt="avatar"
        className="w-10 h-10 rounded-full object-cover"
      />
    );
  }

  // Array value — render overlapping Actor avatars (assignees reference field).
  const ids: string[] = Array.isArray(value)
    ? value.filter((v): v is string => typeof v === "string")
    : [];

  if (ids.length === 0) {
    const empty = <EmptyAvatar mode={mode} placeholder={field.placeholder} />;
    return mode === "compact" ? (
      <CompactCellWrapper>{empty}</CompactCellWrapper>
    ) : (
      empty
    );
  }

  // Avatar size shrinks to `sm` (20px) in compact mode so it fits inside
  // the CompactCellWrapper's 24px height. `md` (28px) is reserved for
  // full inspector rows where natural content height is allowed.
  const avatarSize = mode === "compact" ? "sm" : "md";
  const stack = (
    <div className="flex items-center">
      {ids.map((id, i) => (
        <Avatar
          key={id}
          actorId={id}
          size={avatarSize}
          className={i > 0 ? "-ml-1.5 ring-2 ring-background" : ""}
        />
      ))}
    </div>
  );
  return mode === "compact" ? (
    <CompactCellWrapper>{stack}</CompactCellWrapper>
  ) : (
    stack
  );
}
