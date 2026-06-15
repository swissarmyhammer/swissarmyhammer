/**
 * Comment-log display — renders the `comments` field as a chronological
 * thread, ordered by member `id` ascending (server-minted ULIDs are
 * time-ordered, so id order is creation order).
 *
 * Each member shows the resolved author (avatar + display name via
 * {@link useActorDisplay} — the same resolution the assignees field
 * uses), the relative timestamp (shared {@link formatDateForDisplay}
 * helper, raw value exposed as the native `title` tooltip), and the
 * comment text. Read-only — all mutation lives in the comment-log
 * editor.
 *
 * Compact mode collapses to an icon + member count inside
 * {@link CompactCellWrapper} so the fixed-height grid row contract
 * holds, mirroring `AttachmentListDisplay`.
 */

import { MessageSquare } from "lucide-react";
import { Avatar, useActorDisplay } from "@/components/avatar";
import { formatDateForDisplay } from "@/lib/format-date";
import {
  normalizeComments,
  type CommentMember,
} from "@/components/fields/comment-utils";
import { CompactCellWrapper } from "./compact-cell-wrapper";
import type { FieldDef } from "@/types/kanban";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/** Props for the CommentLogDisplay component. */
export interface CommentLogDisplayProps {
  /** Field definition — drives the empty-state placeholder convention. */
  field?: FieldDef;
  value: unknown;
  mode: "compact" | "full";
}

/** Props for a single rendered comment. */
export interface CommentItemProps {
  member: CommentMember;
}

// ---------------------------------------------------------------------------
// Components
// ---------------------------------------------------------------------------

/**
 * One comment in the thread: author avatar + resolved name, relative
 * timestamp (raw value on `title`), and the comment text. Shared by the
 * read-only display and the editor (which adds its controls alongside).
 */
export function CommentItem({ member }: CommentItemProps) {
  const { name } = useActorDisplay(member.actor);
  return (
    <div data-comment-id={member.id} className="flex flex-col gap-1 min-w-0">
      <div className="flex items-center gap-2 min-w-0">
        <Avatar actorId={member.actor} size="sm" />
        <span className="text-sm font-medium truncate">{name}</span>
        <span
          className="text-xs text-muted-foreground shrink-0 tabular-nums"
          title={member.timestamp}
        >
          {formatDateForDisplay(member.timestamp)}
        </span>
      </div>
      <div className="text-sm whitespace-pre-wrap break-words pl-7">
        {member.text}
      </div>
    </div>
  );
}

/** Renders the comment log as a read-only chronological thread. */
export function CommentLogDisplay({
  field,
  value,
  mode,
}: CommentLogDisplayProps) {
  const members = normalizeComments(value);

  // Compact mode collapses to an inline icon + count so the fixed-height
  // row virtualizer contract (`data-table.tsx::ROW_HEIGHT`) holds. Empty
  // cells honor `field.placeholder` (falling back to `-`) per the
  // convention shared with the attachment/avatar/badge displays.
  if (mode === "compact") {
    return (
      <CompactCellWrapper>
        {members.length > 0 ? (
          <span className="flex items-center gap-1.5 text-sm">
            <MessageSquare className="h-3.5 w-3.5 shrink-0 text-muted-foreground" />
            <span className="text-muted-foreground tabular-nums">
              {members.length}
            </span>
          </span>
        ) : (
          <span className="text-muted-foreground/50">
            {field?.placeholder ?? "-"}
          </span>
        )}
      </CompactCellWrapper>
    );
  }

  if (members.length === 0) {
    return (
      <span className="text-sm text-muted-foreground italic">
        {field?.placeholder ?? "None"}
      </span>
    );
  }

  return (
    <div className="flex flex-col gap-3">
      {members.map((member) => (
        <CommentItem key={member.id} member={member} />
      ))}
    </div>
  );
}
