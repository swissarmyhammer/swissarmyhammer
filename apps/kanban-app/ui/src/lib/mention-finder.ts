/**
 * Find known mention slugs in text by literal string matching.
 *
 * Generic version parameterized by prefix character (e.g. `#` for tags,
 * `@` for actors). The backend is the sole authority on what constitutes
 * a valid mention — this module just locates known `prefix+slug` strings
 * in text for rendering/decoration purposes.
 */

export interface MentionHit {
  /** Start position in text (of the prefix char) */
  index: number;
  /** Length of full match including prefix */
  length: number;
  /** Slug without the prefix */
  slug: string;
}

/**
 * Find all occurrences of known `prefix+slug` patterns in text.
 * Boundary rules: prefix must be preceded by a non-word char (or start of
 * string), and the slug must be followed by a non-slug char (anything other
 * than alphanumeric, hyphen, or underscore) or end of string. Uses Unicode-
 * aware matching so CJK punctuation, em-dashes, emoji, etc. all count as
 * boundaries.
 */
export function findMentionsInText(
  text: string,
  prefix: string,
  slugs: Iterable<string>,
): MentionHit[] {
  const hits: MentionHit[] = [];
  for (const slug of slugs) {
    const needle = `${prefix}${slug}`;
    let pos = 0;
    while (pos <= text.length - needle.length) {
      const idx = text.indexOf(needle, pos);
      if (idx === -1) break;

      const beforeOk = idx === 0 || !/\w/.test(text[idx - 1]);
      const end = idx + needle.length;
      const afterOk = end >= text.length || !/[\w-]/u.test(text[end]);

      if (beforeOk && afterOk) {
        hits.push({ index: idx, length: needle.length, slug });
      }
      pos = idx + 1;
    }
  }
  hits.sort((a, b) => a.index - b.index);
  return hits;
}
