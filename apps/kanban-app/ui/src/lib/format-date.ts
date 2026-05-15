/**
 * Shared date-formatting helpers used by the date field displays.
 *
 * Two entry points serve distinct call-sites:
 *
 * - {@link formatDateForDisplay} — renders a full human sentence
 *   (`"yesterday"`, `"3 hours ago"`, `"Apr 12, 2026"`). Used by
 *   {@link ../components/fields/displays/date-display.tsx DateDisplay} where
 *   the formatted string is the only thing shown.
 * - {@link formatRelativeMagnitude} — renders just the magnitude (`"3 days"`,
 *   `"2 weeks"`, `"just now"`). Used by
 *   {@link ../components/fields/displays/status-date-display.tsx StatusDateDisplay}
 *   to compose directional sentences like `"Completed 2 days ago"` or
 *   `"Scheduled in 3 weeks"` where the caller owns the prefix and direction.
 *
 * Both functions accept the backend's two wire formats — RFC 3339 datetimes
 * (system-derived: `created`, `updated`, `started`, `completed`) and bare
 * `YYYY-MM-DD` calendar dates (user-set: `due`, `scheduled`). Bare dates are
 * parsed as midnight UTC so they sort and compare consistently with the
 * datetime timestamps that sit alongside them.
 *
 * Both functions accept an optional `now` for deterministic tests. If omitted
 * they read the clock via `DateTime.now()`. Unparseable input is returned
 * verbatim — callers never have to special-case parse failures.
 */

import { DateTime } from "luxon";

/**
 * Match a bare `YYYY-MM-DD` calendar date with no time component. Strictly
 * checks the shape; the actual validity of the calendar value (e.g.
 * `2026-99-99`) is left to luxon's parser.
 */
const BARE_DATE_RE = /^\d{4}-\d{2}-\d{2}$/;

/**
 * Parse either a bare `YYYY-MM-DD` calendar date (treated as midnight UTC)
 * or a full ISO 8601 datetime. Returns `null` when the string can't be
 * parsed — callers use this null to fall back to the raw input.
 */
function parseInput(value: string): DateTime | null {
  if (BARE_DATE_RE.test(value)) {
    const dt = DateTime.fromISO(value, { zone: "utc" });
    return dt.isValid ? dt : null;
  }
  const dt = DateTime.fromISO(value);
  return dt.isValid ? dt : null;
}

/**
 * Full-sentence human-friendly rendering of a stored date string.
 *
 * The output follows a three-tier format ladder:
 *
 * 1. **Sub-day relative** (|Δ| < 24h, datetime inputs only) —
 *    `"3 hours ago"`, `"45 minutes ago"`, `"in 2 hours"` via
 *    {@link DateTime.toRelative}.
 * 2. **Calendar bucket** (24h ≤ |Δ| ≤ ~30 days) —
 *    `"yesterday"`, `"today"`, `"tomorrow"`, `"2 days ago"`, `"last week"`,
 *    `"last month"`, `"in 3 days"` via {@link DateTime.toRelativeCalendar}.
 *    Calendar-aware English (e.g. "yesterday" instead of "1 day ago") that
 *    native `Intl.RelativeTimeFormat` cannot produce.
 * 3. **Localized date** (|Δ| > ~30 days) — `"Apr 12, 2026"` if the year
 *    differs from `now`, or just `"Apr 12"` when the year matches (year
 *    omitted to save column width).
 *
 * Bare `YYYY-MM-DD` inputs never take the sub-day branch; "today" / "tomorrow"
 * is always preferred over "in 3 hours".
 *
 * @param value - RFC 3339 datetime or bare `YYYY-MM-DD` date.
 * @param now - Reference instant for all relative computations. Defaults to
 *              `DateTime.now()`; tests pin it to get deterministic output.
 * @returns A human-readable string, or `value` unchanged on parse failure.
 */
export function formatDateForDisplay(
  value: string,
  now: DateTime = DateTime.now(),
): string {
  const dt = parseInput(value);
  if (dt === null) return value;

  const dateOnly = BARE_DATE_RE.test(value);
  const diffDays = Math.abs(dt.diff(now, "days").days);

  // Sub-day relative — only for datetime inputs; bare dates always skip ahead
  // to the calendar bucket so "today"/"tomorrow" wins over "in 3 hours".
  if (!dateOnly && diffDays < 1) {
    return dt.toRelative({ base: now }) ?? value;
  }

  // Calendar bucket — ~30 days on either side. Past this window the calendar
  // phrases start to feel vague ("3 months ago") and a concrete date reads
  // better.
  if (diffDays <= 30) {
    return dt.toRelativeCalendar({ base: now }) ?? value;
  }

  // Far past / future — prefer the compact same-year shape to save width.
  if (dt.year === now.year) {
    return dt.toLocaleString({ month: "short", day: "numeric" });
  }
  return dt.toLocaleString(DateTime.DATE_MED);
}

// --- formatRelativeMagnitude ------------------------------------------------
//
// The bucket constants and rounding rules below match the hand-rolled helper
// previously embedded in `status-date-display.tsx`. They are intentionally
// expressed in raw milliseconds (rather than luxon `Duration` units) so the
// output is bit-for-bit identical to the pre-refactor behaviour — the
// existing `status-date-display.test.tsx` / `status-date-empty.test.ts`
// suites assert on those strings.

const SECOND = 1000;
const MINUTE = 60 * SECOND;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;
const WEEK = 7 * DAY;
const MONTH = 30 * DAY;
const YEAR = 365 * DAY;

/**
 * Magnitude-only relative phrase for a timestamp.
 *
 * Produces direction-agnostic phrases the caller can embed inside a longer
 * sentence: the caller decides whether to prepend `"in "` or append `" ago"`.
 *
 * Bucket boundaries (absolute delta):
 *
 * | |Δ|        | Output                 |
 * | ---------- | ---------------------- |
 * | < 1 minute | `"just now"`           |
 * | < 1 hour   | `"N minute(s)"`        |
 * | < 1 day    | `"N hour(s)"`          |
 * | < 1 week   | `"N day(s)"`           |
 * | < 1 month  | `"N week(s)"`          |
 * | < 1 year   | `"N month(s)"`         |
 * | ≥ 1 year   | `"N year(s)"`          |
 *
 * Month and year buckets use the fixed-length approximations (30 days / 365
 * days) so the output is stable across calendars.
 *
 * @param timestamp - RFC 3339 datetime or bare `YYYY-MM-DD` date.
 * @param now - Reference instant. Defaults to `DateTime.now()`.
 * @returns Magnitude phrase, or `timestamp` unchanged on parse failure.
 */
export function formatRelativeMagnitude(
  timestamp: string,
  now: DateTime = DateTime.now(),
): string {
  const dt = parseInput(timestamp);
  if (dt === null) return timestamp;

  const abs = Math.abs(dt.toMillis() - now.toMillis());

  if (abs < MINUTE) return "just now";
  if (abs < HOUR) {
    const n = Math.round(abs / MINUTE);
    return `${n} minute${n === 1 ? "" : "s"}`;
  }
  if (abs < DAY) {
    const n = Math.round(abs / HOUR);
    return `${n} hour${n === 1 ? "" : "s"}`;
  }
  if (abs < WEEK) {
    const n = Math.round(abs / DAY);
    return `${n} day${n === 1 ? "" : "s"}`;
  }
  if (abs < MONTH) {
    const n = Math.round(abs / WEEK);
    return `${n} week${n === 1 ? "" : "s"}`;
  }
  if (abs < YEAR) {
    const n = Math.round(abs / MONTH);
    return `${n} month${n === 1 ? "" : "s"}`;
  }
  const n = Math.round(abs / YEAR);
  return `${n} year${n === 1 ? "" : "s"}`;
}
