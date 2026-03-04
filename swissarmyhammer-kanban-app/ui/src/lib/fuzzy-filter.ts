/**
 * Simple fuzzy string matching.
 *
 * Checks whether all characters of `query` appear in `target` in order
 * (case-insensitive) and produces a score indicating match quality.
 */

/** Result of a fuzzy match attempt. */
export interface FuzzyResult {
  /** Whether the query matched the target. */
  match: boolean;
  /**
   * Match quality score (lower is better).
   * Only meaningful when `match` is true.
   * Accounts for character spread, start position, and consecutive runs.
   */
  score: number;
}

/**
 * Test whether `query` fuzzy-matches `target`.
 *
 * All characters of `query` must appear in `target` in order (case-insensitive).
 * The score favors:
 * - Matches that start earlier in the target (lower start position)
 * - Characters that are closer together (smaller total gap)
 * - Consecutive character runs (bonus for adjacency)
 *
 * @param query  - The search string typed by the user.
 * @param target - The candidate string to match against.
 * @returns A `FuzzyResult` with `match` and `score`.
 */
export function fuzzyMatch(query: string, target: string): FuzzyResult {
  if (query.length === 0) {
    return { match: true, score: 0 };
  }

  const q = query.toLowerCase();
  const t = target.toLowerCase();

  let qi = 0;
  let firstMatchPos = -1;
  let totalGap = 0;
  let consecutiveBonus = 0;
  let prevPos = -2; // -2 so first match is never "consecutive"

  for (let ti = 0; ti < t.length && qi < q.length; ti++) {
    if (t[ti] === q[qi]) {
      if (firstMatchPos === -1) firstMatchPos = ti;

      // Gap between this match and the previous one (0 = adjacent)
      const gap = ti - prevPos - 1;
      totalGap += gap;

      // Bonus for consecutive characters (adjacent = gap 0)
      if (gap === 0) consecutiveBonus++;

      prevPos = ti;
      qi++;
    }
  }

  if (qi < q.length) {
    return { match: false, score: Infinity };
  }

  // Score: lower is better.
  // - firstMatchPos rewards matches near the start
  // - totalGap penalizes spread-out matches
  // - consecutiveBonus rewards adjacent character runs
  const score = firstMatchPos + totalGap - consecutiveBonus;
  return { match: true, score };
}
