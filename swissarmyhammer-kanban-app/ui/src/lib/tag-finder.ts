/**
 * Find known tag slugs in text by literal string matching.
 *
 * NO parsing logic here — the backend (tag_parser.rs) is the sole authority
 * on what constitutes a valid tag. This module just locates known `#slug`
 * strings in text for rendering/decoration purposes.
 */

export interface TagHit {
  /** Start position in text (of the `#`) */
  index: number;
  /** Length of full match including `#` */
  length: number;
  /** Tag name without `#` */
  slug: string;
}

/**
 * Find all occurrences of known `#slug` patterns in text.
 * Boundary rules: `#` must be preceded by a non-word char (or start of string),
 * and the slug must be followed by whitespace, `#`, or end of string.
 */
export function findTagsInText(
  text: string,
  slugs: Iterable<string>,
): TagHit[] {
  const hits: TagHit[] = [];
  for (const slug of slugs) {
    const needle = `#${slug}`;
    let pos = 0;
    while (pos <= text.length - needle.length) {
      const idx = text.indexOf(needle, pos);
      if (idx === -1) break;

      const beforeOk =
        idx === 0 || !/\w/.test(text[idx - 1]);
      const end = idx + needle.length;
      const afterOk =
        end >= text.length || /[\s#]/.test(text[end]);

      if (beforeOk && afterOk) {
        hits.push({ index: idx, length: needle.length, slug });
      }
      pos = idx + 1;
    }
  }
  hits.sort((a, b) => a.index - b.index);
  return hits;
}
