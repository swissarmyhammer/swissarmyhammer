/**
 * Find known tag slugs in text by literal string matching.
 *
 * Thin wrapper around the generic mention-finder with prefix `#`.
 */

import { findMentionsInText, type MentionHit } from "@/lib/mention-finder";

export type TagHit = MentionHit;

/**
 * Find all occurrences of known `#slug` patterns in text.
 * Boundary rules: `#` must be preceded by a non-word char (or start of string),
 * and the slug must be followed by whitespace, `#`, or end of string.
 */
export function findTagsInText(
  text: string,
  slugs: Iterable<string>,
): TagHit[] {
  return findMentionsInText(text, "#", slugs);
}
