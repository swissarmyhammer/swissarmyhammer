/**
 * Comment-log editor — pure UI for full CRUD on the `comments` array.
 *
 * The editor never persists anything itself: every mutation builds the
 * next wire array and emits it via `onChange`; `Field` persists it
 * through `updateField` → the generic `entity.update_field` command,
 * whose comment-log normalization branch owns ALL server-side logic
 * (minting member ids/timestamps, resolving the author). The editor
 * never sends id/timestamp/author for new members.
 *
 * Wire semantics (matching `comment/normalize.rs`):
 *
 * - **Add** — append `{text}`; the server assigns the rest.
 * - **Edit** — re-emit the member with its `id` retained and only
 *   `text` changed; actor/timestamp are immutable server-side.
 * - **Delete** — replace the member IN PLACE with the tombstone
 *   `{id, deleted: true}`. Never delete by omission: the server merge
 *   treats absence as "preserve" so concurrent agent appends survive a
 *   stale inspector snapshot.
 *
 * The draft array mirrors the last emitted value so consecutive
 * operations inside the autosave debounce window compose instead of
 * clobbering each other. When the store round-trip delivers a fresh
 * `value`, the draft is REBASED onto it (see `rebaseComments`) rather
 * than wholesale-reset: an external field-change (a concurrent agent
 * append) arriving while a local op is still in the debounce window
 * must not drop that op from the draft — a follow-up op would re-emit
 * without it and lose it permanently.
 */

import { useCallback, useEffect, useRef, useState } from "react";
import { Pencil, Trash2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import { CommentItem } from "@/components/fields/displays/comment-log-display";
import {
  isTombstone,
  normalizeComments,
  rebaseComments,
  type CommentMember,
  type CommentWireMember,
} from "@/components/fields/comment-utils";
import type { EditorProps } from ".";

/** Props for one member row inside the editor. */
interface MemberRowProps {
  member: CommentMember;
  /** Commit a text-only edit for this member. */
  onEditCommit: (id: string, text: string) => void;
  /** Replace this member with a tombstone. */
  onDelete: (id: string) => void;
}

/**
 * One stored member inside the editor: the shared {@link CommentItem}
 * read view with edit/delete controls, swapping to an inline textarea
 * while the member's text is being edited.
 */
function MemberRow({ member, onEditCommit, onDelete }: MemberRowProps) {
  const [editing, setEditing] = useState(false);
  const [text, setText] = useState(member.text);

  const startEdit = useCallback(() => {
    setText(member.text);
    setEditing(true);
  }, [member.text]);

  const commitEdit = useCallback(() => {
    setEditing(false);
    // An empty edit is a cancel, consistent with the add path's empty
    // no-op — a blank comment body is never persisted.
    if (!text.trim()) return;
    if (text !== member.text) onEditCommit(member.id, text);
  }, [text, member.id, member.text, onEditCommit]);

  if (editing) {
    return (
      <div data-comment-id={member.id} className="flex flex-col gap-1.5">
        <Textarea
          value={text}
          onChange={(e) => setText(e.target.value)}
          className="text-sm"
          aria-label="Comment text"
        />
        <div className="flex justify-end gap-1.5">
          <Button variant="ghost" size="sm" onClick={() => setEditing(false)}>
            Cancel
          </Button>
          <Button variant="outline" size="sm" onClick={commitEdit}>
            Save
          </Button>
        </div>
      </div>
    );
  }

  return (
    <div className="flex items-start gap-1 group">
      <div className="flex-1 min-w-0">
        <CommentItem member={member} />
      </div>
      <Button
        variant="ghost"
        size="icon"
        className="h-5 w-5 shrink-0 opacity-50 hover:opacity-100"
        aria-label="Edit comment"
        onClick={startEdit}
      >
        <Pencil size={12} />
      </Button>
      <Button
        variant="ghost"
        size="icon"
        className="h-5 w-5 shrink-0 opacity-50 hover:opacity-100"
        aria-label="Delete comment"
        onClick={() => onDelete(member.id)}
      >
        <Trash2 size={12} />
      </Button>
    </div>
  );
}

/**
 * Renders a freshly composed member that has not completed the server
 * round-trip yet — it carries only `text` (no id/author/timestamp), so
 * it shows as muted/pending until the normalized value comes back.
 * Mirrors `PendingAttachmentItem` in the attachment family.
 */
function PendingCommentItem({ text }: { text: string }) {
  return (
    <div
      className="text-sm whitespace-pre-wrap break-words opacity-60 italic"
      aria-busy="true"
    >
      {text}
      <span className="sr-only">pending</span>
    </div>
  );
}

/**
 * Editor for comment-log fields: the existing thread with per-member
 * edit/delete controls and a compose box for new comments. Pure UI —
 * see the file header for the wire contract.
 */
export function CommentLogEditor({ value, onChange }: EditorProps) {
  // Draft mirrors the last array this editor emitted so consecutive
  // operations within the autosave debounce window compose (two quick
  // deletes must carry BOTH tombstones). When a fresh value arrives,
  // REBASE the draft onto it instead of wholesale-resetting: the fresh
  // value may be an external change (agent append) landing while a
  // local op is still pending in the autosave debounce, and resetting
  // would drop that op — a follow-up op in the window would then emit
  // without it, losing it permanently. `baseline` tracks the server
  // value the draft was last rebased onto so rebaseComments can tell
  // local edits apart from server-acknowledged ones.
  const [draft, setDraft] = useState<CommentWireMember[]>(() =>
    normalizeComments(value),
  );
  const baselineRef = useRef<CommentMember[]>(normalizeComments(value));
  useEffect(() => {
    const fresh = normalizeComments(value);
    setDraft((prev) => rebaseComments(prev, baselineRef.current, fresh));
    baselineRef.current = fresh;
  }, [value]);

  const [composeText, setComposeText] = useState("");
  const composeRef = useRef<HTMLTextAreaElement>(null);

  // The compose box is the editor's primary affordance — focus it on
  // mount so entering edit mode drops the user straight into typing.
  useEffect(() => {
    composeRef.current?.focus();
  }, []);

  const emit = useCallback(
    (next: CommentWireMember[]) => {
      setDraft(next);
      onChange?.(next);
    },
    [onChange],
  );

  const handleAdd = useCallback(() => {
    const text = composeText.trim();
    if (!text) return;
    emit([...draft, { text }]);
    setComposeText("");
  }, [composeText, draft, emit]);

  const handleEditCommit = useCallback(
    (id: string, text: string) => {
      emit(
        draft.map((m) =>
          !isTombstone(m) && "id" in m && m.id === id ? { ...m, text } : m,
        ),
      );
    },
    [draft, emit],
  );

  const handleDelete = useCallback(
    (id: string) => {
      // Explicit tombstone IN PLACE — never filter the member out
      // (absence means "preserve" on the server).
      emit(
        draft.map((m) =>
          !isTombstone(m) && "id" in m && m.id === id
            ? { id, deleted: true as const }
            : m,
        ),
      );
    },
    [draft, emit],
  );

  return (
    <div className="flex flex-col gap-3 w-full">
      {draft.map((member, index) => {
        if (isTombstone(member)) return null;
        if (!("id" in member)) {
          return <PendingCommentItem key={`pending:${index}`} text={member.text} />;
        }
        return (
          <MemberRow
            key={member.id}
            member={member}
            onEditCommit={handleEditCommit}
            onDelete={handleDelete}
          />
        );
      })}

      <div className="flex flex-col gap-1.5">
        <Textarea
          ref={composeRef}
          value={composeText}
          onChange={(e) => setComposeText(e.target.value)}
          onKeyDown={(e) => {
            // Enter submits; Shift+Enter inserts a newline. Never treat
            // Enter as submit mid-IME-composition — it only confirms the
            // composition (precedent: prompt-input.tsx).
            if (e.nativeEvent.isComposing) return;
            if (e.key === "Enter" && !e.shiftKey) {
              e.preventDefault();
              handleAdd();
            }
          }}
          placeholder="Add a comment…"
          className="text-sm"
        />
        <div className="flex justify-end">
          <Button variant="outline" size="sm" onClick={handleAdd}>
            Comment
          </Button>
        </div>
      </div>
    </div>
  );
}
