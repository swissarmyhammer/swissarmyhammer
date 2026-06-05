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
  /**
   * Lookup slug for the metaMap, without the prefix. For literal mentions this
   * is the matched slug verbatim. For shape-matched task refs (`^`) it is the
   * canonical short id — a full 26-char ULID is normalized to its lowercased
   * last-7, so both `^<short>` and `^<full-ulid>` resolve through the same key.
   */
  slug: string;
}

/** The mention prefix whose references are matched by shape, not enumeration. */
const SHORT_REF_PREFIX = "^";

/**
 * Crockford-base32 character class (excludes I, L, O, U), case-insensitive.
 * ULIDs and the derived 7-char short ids are drawn from this alphabet.
 */
const CROCKFORD = "[0-9A-HJKMNP-TV-Za-hjkmnp-tv-z]";

/** Number of trailing ULID chars that form the canonical short id. */
const SHORT_ID_LEN = 7;

/** Length of a full ULID, in characters. */
const ULID_LEN = 26;

/**
 * Match `^` task references by shape: the caret followed by exactly a full
 * 26-char ULID OR exactly a 7-char short id. The alternation lists the
 * 26-char form first so a full ULID is consumed as one token rather than as a
 * 7-char prefix plus trailing characters (longest-first). Boundary guards
 * mirror the literal finder: the caret must not follow a word char, and the
 * run must not be flanked by `[\w-]` on either side.
 */
const SHORT_REF_RE = new RegExp(
  `(?<![\\w-])\\^(${CROCKFORD}{${ULID_LEN}}|${CROCKFORD}{${SHORT_ID_LEN}})(?![\\w-])`,
  "gu",
);

/**
 * Find `^` task references in text by shape and normalize each to its short id.
 *
 * A matched 26-char ULID is normalized to its lowercased last-7; a matched
 * 7-char short id is lowercased as-is. The returned `length` spans the full
 * matched token (caret + run), while `slug` is the normalized short id used to
 * resolve against the task metaMap. Unknown ids (shape matches, lookup misses)
 * are the caller's concern — this finder only locates well-shaped references.
 */
function findShortRefsInText(text: string): MentionHit[] {
  const hits: MentionHit[] = [];
  SHORT_REF_RE.lastIndex = 0;
  let m: RegExpExecArray | null;
  while ((m = SHORT_REF_RE.exec(text)) !== null) {
    const run = m[1];
    const slug = (
      run.length === ULID_LEN ? run.slice(-SHORT_ID_LEN) : run
    ).toLowerCase();
    hits.push({ index: m.index, length: m[0].length, slug });
  }
  return hits;
}

/**
 * Find all occurrences of known `prefix+slug` patterns in text.
 *
 * The `^` prefix is matched by SHAPE — a caret followed by a full ULID or a
 * 7-char short id — via {@link findShortRefsInText}; the `slugs` argument is
 * unused for it. Every other prefix is matched by literal enumeration of
 * `slugs`.
 *
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
  if (prefix === SHORT_REF_PREFIX) {
    return findShortRefsInText(text);
  }
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
