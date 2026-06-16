/**
 * Shared runtime helpers for the comment-log field.
 *
 * The display and editor variants share one runtime contract for the
 * `comments` field value (see the Rust side: `comment/normalize.rs`):
 *
 * - **Stored members** always carry `{id, actor, text, timestamp}` with a
 *   server-minted ULID `id` (time-ordered, so id order == creation order),
 *   sorted by `id` ascending.
 * - **Wire-only shapes** exist transiently inside the editor between a
 *   local mutation and the server round-trip: a new member is `{text}`
 *   (the server assigns id/timestamp/author) and an explicit delete is
 *   the tombstone `{id, deleted: true}` — absence from the committed
 *   array means "preserve", never "delete".
 *
 * Centralizing the validation here keeps the two components in lock-step,
 * mirroring `attachment-utils.ts` for the attachment field family.
 */

/** A stored comment-log member as returned by the server. */
export interface CommentMember {
  /** Server-minted ULID — time-ordered, so id order is creation order. */
  id: string;
  /** Resolved author actor id. Immutable after creation. */
  actor: string;
  /** The comment body. The only member field the editor may change. */
  text: string;
  /** RFC 3339 creation timestamp. Immutable after creation. */
  timestamp: string;
}

/** Wire-only shape for a freshly composed member — server assigns the rest. */
export interface NewCommentMember {
  text: string;
}

/** Wire-only explicit-delete marker. Never stored, never rendered. */
export interface CommentTombstone {
  id: string;
  deleted: true;
}

/** Any element the editor may hold in its draft array. */
export type CommentWireMember =
  | CommentMember
  | NewCommentMember
  | CommentTombstone;

/** Check whether a wire member is the explicit-delete tombstone. */
export function isTombstone(v: CommentWireMember): v is CommentTombstone {
  return (v as CommentTombstone).deleted === true;
}

/**
 * Check whether a single value is a valid stored comment member.
 *
 * Valid members are objects with a string `id` that are not tombstones.
 * Anything else — `null`, numbers, objects without an `id`, wire-only
 * tombstones — is rejected so downstream renderers never see unexpected
 * shapes.
 */
export function isStoredMember(v: unknown): v is CommentMember {
  return (
    v != null &&
    typeof v === "object" &&
    typeof (v as Record<string, unknown>).id === "string" &&
    (v as Record<string, unknown>).deleted !== true
  );
}

/**
 * Normalize the field value into stored members sorted by `id` ascending.
 *
 * The server already stores the log in id order, but the sort is cheap
 * defence so the rendered thread is always chronological regardless of
 * what shape the value arrived in. Invalid elements and tombstones are
 * silently filtered out.
 */
export function normalizeComments(value: unknown): CommentMember[] {
  if (!Array.isArray(value)) return [];
  return value
    .filter(isStoredMember)
    .sort((a, b) => (a.id < b.id ? -1 : a.id > b.id ? 1 : 0));
}

/**
 * Rebase un-flushed local draft operations onto a fresh server value.
 *
 * The editor's emits are debounce-autosaved, so an external field-change
 * (e.g. a concurrent agent append) can deliver a fresh `value` while
 * local ops are still pending. Wholesale-resetting the draft to that
 * value would drop the pending ops — and a follow-up op inside the
 * debounce window would then re-emit WITHOUT them, losing them for good.
 * Instead, re-apply each local op that the fresh value has not
 * acknowledged yet:
 *
 * - **Tombstones** — kept (in place) while their id still exists in
 *   `fresh`; dropped once the server acknowledged the delete (the id is
 *   gone). Tombstoning is idempotent server-side, so re-emitting is safe.
 * - **Text edits** — a draft member whose text diverged from `baseline`
 *   keeps its local text while `fresh` still returns the baseline text;
 *   any other server-side text wins (the edit was acknowledged, or
 *   someone else changed it).
 * - **Pending `{text}` adds** — kept (appended) until the server mints
 *   them, detected as a `fresh` member absent from `baseline` carrying
 *   the same text.
 *
 * @param draft - The editor's current wire array (last emitted value).
 * @param baseline - The server value the draft was last rebased onto.
 * @param fresh - The newly arrived server value (stored members only).
 * @returns The next draft: `fresh` with un-acknowledged local ops re-applied.
 */
export function rebaseComments(
  draft: CommentWireMember[],
  baseline: CommentMember[],
  fresh: CommentMember[],
): CommentWireMember[] {
  const tombstoned = new Set(draft.filter(isTombstone).map((t) => t.id));
  const baselineById = new Map(baseline.map((m) => [m.id, m]));
  const draftById = new Map(draft.filter(isStoredMember).map((m) => [m.id, m]));

  const next: CommentWireMember[] = fresh.map((m) => {
    if (tombstoned.has(m.id)) return { id: m.id, deleted: true as const };
    const local = draftById.get(m.id);
    const base = baselineById.get(m.id);
    if (local && base && local.text !== base.text && m.text === base.text) {
      return { ...m, text: local.text };
    }
    return m;
  });

  const baselineIds = new Set(baseline.map((m) => m.id));
  const mintedTexts = new Set(
    fresh.filter((m) => !baselineIds.has(m.id)).map((m) => m.text),
  );
  for (const m of draft) {
    if (!isTombstone(m) && !("id" in m) && !mintedTexts.has(m.text)) {
      next.push(m);
    }
  }
  return next;
}
