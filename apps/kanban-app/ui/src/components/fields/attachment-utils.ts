/**
 * Shared runtime helpers for attachment fields.
 *
 * The display and editor variants both have to tolerate the
 * commit -> entity-layer enrichment round-trip, during which the
 * `attachments` array briefly holds raw string paths alongside fully
 * enriched {@link AttachmentMeta} objects. Centralizing the validation
 * and basename helpers here keeps the two components in lock-step on
 * the runtime contract — see the review note on
 * `01KQSDSF1Y0T38F55VNH1952V8` for the parity gap this closes.
 */

import type { AttachmentMeta } from "@/components/fields/displays/attachment-display";

/**
 * Check whether a single value is a valid attachment element.
 *
 * Valid elements are either strings (file paths) or objects with a
 * string `id` property (an {@link AttachmentMeta}-shaped value).
 * Anything else — `null`, numbers, objects without an `id`, etc. — is
 * rejected, so downstream renderers never see unexpected shapes.
 */
export function isValidElement(v: unknown): v is AttachmentMeta | string {
  if (typeof v === "string") return true;
  if (
    v != null &&
    typeof v === "object" &&
    "id" in v &&
    typeof (v as Record<string, unknown>).id === "string"
  )
    return true;
  return false;
}

/**
 * Normalize the value prop into an array of attachments/paths.
 *
 * The value can be:
 * - An array of {@link AttachmentMeta} objects (existing attachments)
 * - An array containing a mix of {@link AttachmentMeta} and string
 *   paths (newly dropped/picked entries during the round-trip)
 * - A single valid element (rare, but tolerated)
 * - `null`/`undefined` (empty)
 *
 * Invalid elements are silently filtered out so callers can rely on a
 * clean `(AttachmentMeta | string)[]`.
 */
export function normalizeAttachments(
  value: unknown,
): Array<AttachmentMeta | string> {
  if (Array.isArray(value)) return value.filter(isValidElement);
  if (isValidElement(value)) return [value];
  return [];
}

/**
 * Extract the final path segment from a POSIX/Windows-style path so a
 * pending-drop string entry renders by basename instead of its full
 * temp path. Falls back to the original `path` when no separator is
 * present, when the path ends in a separator (e.g. `"/foo/bar/"`), or
 * when the path is empty — guaranteeing the user always sees *something*,
 * mirroring the editor's pending branch which falls back to the full
 * `att` text.
 */
export function basename(path: string): string {
  const slash = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
  if (slash >= 0 && slash < path.length - 1) return path.slice(slash + 1);
  return path;
}
